//! x402 Payment Server — HTTP 402 Payment Required microservice
//!
//! Serves:
//!   GET  /.well-known/x402  → discovery document
//!   GET  /features           → premium features list
//!   POST /verify             → verify a payment tx_hash
//!
//! Usage: synapsis-x402 [--port <PORT>] [--wallet <ADDR>] [--rpc <URL>]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

fn handle_client(mut stream: TcpStream, engine: &synapsis::core::x402::X402Engine) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]);

    let (status, body, content_type) = if request.starts_with("GET /.well-known/x402") {
        let discovery = engine.get_x402_discovery();
        (200, serde_json::to_string_pretty(&discovery).unwrap_or_default(), "application/json")
    } else if request.starts_with("GET /features") {
        let features = synapsis::core::x402::all_premium_features();
        (200, serde_json::to_string_pretty(&features).unwrap_or_default(), "application/json")
    } else if request.starts_with("POST /verify") {
        if let Some(body_start) = request.find("\r\n\r\n") {
            let body_str = request[body_start + 4..].trim();
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(body_str) {
                let tx_hash = payload.get("tx_hash").and_then(|v| v.as_str()).unwrap_or("");
                let feature = payload.get("feature").and_then(|v| v.as_str()).unwrap_or("");
                if tx_hash.is_empty() || feature.is_empty() {
                    (400, r#"{"error":"missing tx_hash or feature"}"#.into(), "application/json")
                } else {
                    match tokio::runtime::Runtime::new() {
                        Ok(rt) => match rt.block_on(engine.verify_payment(tx_hash, feature)) {
                            Ok(true) => (200, r#"{"verified":true}"#.into(), "application/json"),
                            Ok(false) => (200, r#"{"verified":false}"#.into(), "application/json"),
                            Err(e) => (500, format!(r#"{{"error":"{}"}}"#, e), "application/json"),
                        },
                        Err(e) => (500, format!(r#"{{"error":"{}"}}"#, e), "application/json"),
                    }
                }
            } else {
                (400, r#"{"error":"invalid JSON"}"#.into(), "application/json")
            }
        } else {
            (400, r#"{"error":"empty body"}"#.into(), "application/json")
        }
    } else {
        (404, r#"{"error":"not found"}"#.into(), "application/json")
    };

    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {length}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
        status = status,
        reason = if status == 200 { "OK" } else if status == 400 { "Bad Request" } else if status == 404 { "Not Found" } else { "Internal Server Error" },
        content_type = content_type,
        length = body.len(),
        body = body,
    );
    let _ = stream.write_all(response.as_bytes());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = 4020;
    let mut wallet = String::from("0x0000000000000000000000000000000000000000");
    let mut rpc = String::from("https://mainnet.base.org");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                if let Some(p) = args.get(i + 1) {
                    port = p.parse().unwrap_or(4020);
                    i += 1;
                }
            }
            "--wallet" | "-w" => {
                if let Some(w) = args.get(i + 1) {
                    wallet = w.clone();
                    i += 1;
                }
            }
            "--rpc" | "-r" => {
                if let Some(u) = args.get(i + 1) {
                    rpc = u.clone();
                    i += 1;
                }
            }
            "--help" | "-h" => {
                eprintln!("x402 Payment Server");
                eprintln!("Usage: synapsis-x402 [--port <PORT>] [--wallet <ADDR>] [--rpc <URL>]");
                eprintln!("Default port: 4020");
                eprintln!("Default RPC: https://mainnet.base.org");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    let engine = Arc::new(synapsis::core::x402::X402Engine::new(&wallet, &rpc));
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).expect("Failed to bind");

    eprintln!("╔══════════════════════════════════════════════╗");
    eprintln!("║  x402 Payment Server                        ║");
    eprintln!("║  Port: {}                                  ║", port);
    eprintln!("║  Wallet: {}  ║", &wallet[..wallet.len().min(36)]);
    eprintln!("║  RPC: {}    ║", &rpc[..rpc.len().min(34)]);
    eprintln!("║                                              ║");
    eprintln!("║  GET  /.well-known/x402  — discovery         ║");
    eprintln!("║  GET  /features           — premium features ║");
    eprintln!("║  POST /verify             — verify payment   ║");
    eprintln!("╚══════════════════════════════════════════════╝");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let engine = engine.clone();
                std::thread::spawn(move || handle_client(stream, &engine));
            }
            Err(e) => eprintln!("Connection failed: {}", e),
        }
    }
}
