use crate::core::retry::CircuitBreaker;
use crate::presentation::mcp::McpServer;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

pub struct HttpTransport {
    server: Arc<McpServer>,
    circuit: CircuitBreaker,
}

impl HttpTransport {
    pub fn new(server: Arc<McpServer>) -> Self {
        Self {
            server,
            circuit: CircuitBreaker::new(10, 60),
        }
    }

    pub fn start(&self, port: u16) {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).expect("Failed to bind HTTP server");
        eprintln!("[Synapsis MCP] HTTP/SSE server listening on {}", addr);

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
                    std::thread::spawn(move || {
                        handle_connection(stream, &server);
                    });
                }
                Err(e) => eprintln!("[HTTP] Connection error: {}", e),
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream, server: &McpServer) {
    let cloned = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(cloned);

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
                let resp = "HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
            let mut body = vec![0u8; content_length];
            let _ = reader.read_exact(&mut body);
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
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(resp.as_bytes());
        }
    }
}
