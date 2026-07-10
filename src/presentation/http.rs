use crate::core::retry::CircuitBreaker;
use crate::presentation::mcp::McpServer;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;

pub struct HttpTransport {
    server: Arc<McpServer>,
    circuit: CircuitBreaker,
    tls_config: Option<Arc<rustls::ServerConfig>>,
}

impl HttpTransport {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self {
            server,
            circuit: CircuitBreaker::new(10, 60),
            tls_config: None,
        }
    }

    pub fn with_tls(server: Arc<McpServer>, tls_config: rustls::ServerConfig) -> Self {
        Self {
            server,
            circuit: CircuitBreaker::new(10, 60),
            tls_config: Some(Arc::new(tls_config)),
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
                        let resp =
                            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n";
                        let mut stream = stream;
                        let _ = stream.write_all(resp.as_bytes());
                        continue;
                    }
                    let server = self.server.clone();
                    let tls_config = self.tls_config.clone();
                    std::thread::spawn(move || {
                        if let Some(tls_config) = tls_config {
                            match rustls::ServerConnection::new(tls_config) {
                                Ok(conn) => {
                                    let tls_stream =
                                        rustls::StreamOwned::new(conn, stream);
                                    handle_connection(tls_stream, &server);
                                }
                                Err(e) => {
                                    eprintln!("[HTTPS] TLS handshake error: {}", e);
                                }
                            }
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
    let mut reader = BufReader::new(&mut stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1];
    let mut content_length: usize = 0;

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

    match (method, path) {
        ("GET", "/sse") => {
            drop(reader);
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
            if content_length > 10_000_000 {
                drop(reader);
                let resp = "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
            let mut body = vec![0u8; content_length];
            let _ = reader.read_exact(&mut body);
            drop(reader);
            let body_str = String::from_utf8_lossy(&body);
            let response = server.handle_message(&body_str).unwrap_or_default();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                response.len(),
                response
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
        _ => {
            drop(reader);
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
        }
    }
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

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(config)
}

pub fn generate_self_signed_cert() -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let key_pair = rcgen::KeyPair::generate()?;
    let cert_params =
        rcgen::CertificateParams::new(vec!["synapsis.local".to_string(), "127.0.0.1".to_string()])?;
    let cert = cert_params.self_signed(&key_pair)?;
    Ok((cert.der().to_vec(), key_pair.serialize_der()))
}
