use crate::core::retry::CircuitBreaker;
use crate::core::x402::X402Engine;
use crate::presentation::mcp::McpServer;
use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

pub struct HttpTransport {
    server: Arc<McpServer>,
    circuit: CircuitBreaker,
    tls_config: Option<Arc<rustls::ServerConfig>>,
    x402: Option<Arc<X402Engine>>,
}

impl HttpTransport {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self {
            server,
            circuit: CircuitBreaker::new(10, 60),
            tls_config: None,
            x402: None,
        }
    }

    pub fn with_tls(server: Arc<McpServer>, tls_config: rustls::ServerConfig) -> Self {
        Self {
            server,
            circuit: CircuitBreaker::new(10, 60),
            tls_config: Some(Arc::new(tls_config)),
            x402: None,
        }
    }

    pub fn with_x402(server: Arc<McpServer>, x402: Arc<X402Engine>) -> Self {
        Self {
            server,
            circuit: CircuitBreaker::new(10, 60),
            tls_config: None,
            x402: Some(x402),
        }
    }

    pub fn with_tls_x402(
        server: Arc<McpServer>,
        tls_config: rustls::ServerConfig,
        x402: Arc<X402Engine>,
    ) -> Self {
        Self {
            server,
            circuit: CircuitBreaker::new(10, 60),
            tls_config: Some(Arc::new(tls_config)),
            x402: Some(x402),
        }
    }

    pub fn start(&self, port: u16) {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).expect("Failed to bind HTTP server");
        let proto = if self.tls_config.is_some() {
            "HTTPS"
        } else {
            "HTTP"
        };
        eprintln!("[Synapsis MCP] {}/SSE server listening on {}", proto, addr);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    if !self.circuit.is_closed() {
                        eprintln!("[HTTP] Circuit open - rejecting connection");
                        let resp = "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n";
                        let mut stream = stream;
                        let _ = stream.write_all(resp.as_bytes());
                        continue;
                    }
                    let server = self.server.clone();
                    let tls_config = self.tls_config.clone();
                    let x402 = self.x402.clone();
                    std::thread::spawn(move || {
                        if let Some(tls_config) = tls_config {
                            match rustls::ServerConnection::new(tls_config) {
                                Ok(conn) => {
                                    let tls_stream = rustls::StreamOwned::new(conn, stream);
                                    if let Some(x402) = x402 {
                                        handle_connection_x402(tls_stream, &server, &x402);
                                    } else {
                                        handle_connection(tls_stream, &server);
                                    }
                                }
                                Err(e) => eprintln!("[HTTPS] TLS handshake error: {}", e),
                            }
                        } else if let Some(x402) = x402 {
                            handle_connection_x402(stream, &server, &x402);
                        } else {
                            handle_connection(stream, &server);
                        }
                    });
                }
                Err(e) => eprintln!("[HTTP] Connection error: {}", e),
            }
        }
    }
}

fn handle_connection(mut stream: impl Read + Write, server: &McpServer) {
    let mut request = parse_http_request(&mut stream);
    if request.path == "/.well-known/x402" {
        let disc = serde_json::json!({"error": "x402 not configured", "documentation": "Set SYNAPSIS_X402_WALLET"});
        respond(
            &mut stream,
            404,
            &serde_json::to_string(&disc).unwrap_or_default(),
        );
        return;
    }
    handle_mcp_request(&mut stream, &mut request, server);
}

fn handle_connection_x402(mut stream: impl Read + Write, server: &McpServer, x402: &X402Engine) {
    let mut request = parse_http_request(&mut stream);
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/.well-known/x402") => {
            let disc = x402.get_x402_discovery();
            respond(
                &mut stream,
                200,
                &serde_json::to_string_pretty(&disc).unwrap_or_default(),
            );
        }
        ("POST", "/x402/verify") => {
            let body = request.body.clone().unwrap_or_default();
            let v: serde_json::Value =
                serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
            let tx_hash = v["tx_hash"].as_str().unwrap_or("");
            let feature = v["feature"].as_str().unwrap_or("");
            if tx_hash.is_empty() || feature.is_empty() {
                respond(
                    &mut stream,
                    400,
                    r#"{"error":"tx_hash and feature required"}"#,
                );
                return;
            }
            let rt = tokio::runtime::Runtime::new().unwrap();
            match rt.block_on(x402.verify_payment(tx_hash, feature)) {
                Ok(true) => respond(&mut stream, 200, r#"{"status":"verified"}"#),
                _ => respond(
                    &mut stream,
                    402,
                    r#"{"error":"payment required","network":"base","currency":"USDC"}"#,
                ),
            }
        }
        _ => handle_mcp_request(&mut stream, &mut request, server),
    }
}

struct HttpRequest {
    method: String,
    path: String,
    body: Option<String>,
    content_length: usize,
}

fn parse_http_request(stream: &mut (impl Read + Write)) -> HttpRequest {
    let mut reader = BufReader::new((&mut *stream) as &mut dyn Read);
    let mut request_line = String::new();
    let mut method = String::new();
    let mut path = String::new();
    let mut content_length: usize = 0;

    if reader.read_line(&mut request_line).is_ok() {
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() >= 2 {
            method = parts[0].to_string();
            path = parts[1].to_string();
        }
    }

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        let line = line.trim();
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_lowercase();
            let value = line[pos + 1..].trim().to_string();
            if key == "content-length" {
                content_length = value.parse().unwrap_or(0).min(10_000_000);
            }
        }
    }

    let body = if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        reader.read_exact(&mut buf).ok();
        Some(String::from_utf8_lossy(&buf).to_string())
    } else {
        None
    };

    HttpRequest {
        method,
        path,
        body,
        content_length,
    }
}

fn handle_mcp_request(stream: &mut (impl Read + Write), req: &HttpRequest, server: &McpServer) {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/sse") => {
            let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
            loop {
                let _ = stream.write_all(b"data: {\"type\":\"keepalive\"}\n\n");
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
        }
        ("POST", "/") | ("POST", "/message") => {
            if req.content_length > 10_000_000 {
                respond(stream, 413, "");
                return;
            }
            let body_str = req.body.as_deref().unwrap_or("");
            let response = server.handle_message(body_str).unwrap_or_default();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                response.len(),
                response
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
        _ => respond(stream, 404, ""),
    }
}

fn respond(stream: &mut impl Write, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        402 => "Payment Required",
        404 => "Not Found",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Unknown",
    };
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
        status,
        reason,
        body.len(),
        body
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

pub fn load_tls_config(cert_path: &str, key_path: &str) -> anyhow::Result<rustls::ServerConfig> {
    use rustls::pki_types::pem::PemObject;
    let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls::pki_types::CertificateDer::pem_file_iter(cert_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", cert_path, e))?
            .collect::<Result<Vec<_>, _>>()?;
    if certs.is_empty() {
        anyhow::bail!("No certificates found in {}", cert_path);
    }
    let key = rustls::pki_types::PrivateKeyDer::from_pem_file(key_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", key_path, e))?;
    Ok(rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?)
}

pub fn generate_self_signed_cert() -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let key_pair = rcgen::KeyPair::generate()?;
    let cert_params =
        rcgen::CertificateParams::new(vec!["synapsis.local".to_string(), "127.0.0.1".to_string()])?;
    let cert = cert_params.self_signed(&key_pair)?;
    Ok((cert.der().to_vec(), key_pair.serialize_der()))
}
