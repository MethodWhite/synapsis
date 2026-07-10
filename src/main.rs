//! Synapsis - Unified MCP Server
//!
//! Starts the MCP server with HTTP/SSE transport for multi-agent coordination.
//! All agent communication happens via standard MCP protocol - no raw TCP.

use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" => {
                eprintln!("synapsis v{}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                eprintln!("synapsis v{}", env!("CARGO_PKG_VERSION"));
                eprintln!("Usage: synapsis [--version | --help]");
                eprintln!("       synapsis license <status|verify|sign>");
                eprintln!("       synapsis [--tls-cert <path> --tls-key <path>]");
                eprintln!("Without arguments, starts the HTTP/SSE MCP server on port 7438.");
                eprintln!("Use --tls-cert and --tls-key to enable HTTPS.");
                eprintln!("Env: SYNAPSIS_PORT, SYNAPSIS_TLS_CERT, SYNAPSIS_TLS_KEY");
                eprintln!("     SYNAPSIS_LICENSE (path to license file)");
                std::process::exit(0);
            }
            "license" => {
                let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
                match sub {
                    "status" => {
                        println!("{}", synapsis::core::license::current_license_status());
                        std::process::exit(0);
                    }
                    "verify" => {
                        match synapsis::core::license::load_license() {
                            Some(lic) => {
                                println!("License: VALID");
                                println!("Customer: {}", lic.data.customer);
                                println!("Type: {}", lic.data.license_type);
                                println!("Issued: {}", lic.data.issued_at);
                                println!("Expires: {}", lic.data.expires_at);
                                println!("Features: {}", lic.data.features.join(", "));
                                std::process::exit(0);
                            }
                            None => {
                                eprintln!("License: NOT FOUND or INVALID");
                                eprintln!("{}", synapsis::core::license::current_license_status());
                                std::process::exit(1);
                            }
                        }
                    }
                    "sign" => {
                        let license_path = args.get(3).expect("Usage: synapsis license sign <license.json>");
                        let data_str = std::fs::read_to_string(license_path)
                            .expect("Failed to read license file");
                        let data: synapsis::core::license::LicenseData = serde_json::from_str(&data_str)
                            .expect("Invalid license JSON");
                        eprint!("Enter private key: ");
                        let mut privkey = String::new();
                        std::io::stdin().read_line(&mut privkey).ok();
                        let privkey = privkey.trim();
                        match synapsis::core::license::sign_license(data, privkey) {
                            Ok(signed) => {
                                let out = serde_json::to_string_pretty(&signed).expect("JSON");
                                let out_path = format!("{}.signed", license_path);
                                std::fs::write(&out_path, &out).expect("Write");
                                println!("Signed license written to: {}", out_path);
                                std::process::exit(0);
                            }
                            Err(e) => {
                                eprintln!("Signing failed: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                    _ => {
                        eprintln!("Usage: synapsis license <status|verify|sign>");
                        std::process::exit(1);
                    }
                }
            }
            "x402" => {
                let sub = args.get(2).map(|s| s.as_str()).unwrap_or("");
                match sub {
                    "discover" => {
                        let engine = synapsis::core::x402::X402Engine::new(
                            "0x0000000000000000000000000000000000000000",
                            "https://mainnet.base.org",
                        );
                        let discovery = engine.get_x402_discovery();
                        println!("{}", serde_json::to_string_pretty(&discovery).unwrap());
                        std::process::exit(0);
                    }
                    "features" => {
                        let features = synapsis::core::x402::all_premium_features();
                        println!("{:<20} {:<50} {:>10}", "Feature", "Description", "Price");
                        println!("{}", "-".repeat(82));
                        for f in &features {
                            println!(
                                "{:<20} {:<50} {:>8.3} USDC",
                                f.name, f.description, f.price_usdc
                            );
                        }
                        std::process::exit(0);
                    }
                    _ => {
                        eprintln!("Usage: synapsis x402 <discover|features>");
                        std::process::exit(1);
                    }
                }
            }
            _ => {}
        }
    }

    let port: u16 = std::env::var("SYNAPSIS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7438);

    let tls_cert = std::env::var("SYNAPSIS_TLS_CERT").ok().or_else(|| {
        let mut val = None;
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--tls-cert" {
                val = args.get(i + 1).cloned();
            }
            i += 1;
        }
        val
    });

    let tls_key = std::env::var("SYNAPSIS_TLS_KEY").ok().or_else(|| {
        let mut val = None;
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--tls-key" {
                val = args.get(i + 1).cloned();
            }
            i += 1;
        }
        val
    });

    let tls_config = match (tls_cert, tls_key) {
        (Some(cert_path), Some(key_path)) => {
            match synapsis::presentation::http::load_tls_config(&cert_path, &key_path) {
                Ok(cfg) => {
                    eprintln!("[Synapsis] TLS configured (cert: {})", cert_path);
                    Some(cfg)
                }
                Err(e) => {
                    eprintln!("[Synapsis] Failed to load TLS config: {}", e);
                    std::process::exit(1);
                }
            }
        }
        (Some(_cert_path), None) => {
            eprintln!("[Synapsis] SYNAPSIS_TLS_CERT set without key — generating self-signed cert");
            match synapsis::presentation::http::generate_self_signed_cert() {
                Ok((cert_der, key_der)) => {
                    match rustls::ServerConfig::builder()
                        .with_no_client_auth()
                        .with_single_cert(
                            vec![rustls::pki_types::CertificateDer::from(cert_der)],
                            rustls::pki_types::PrivateKeyDer::try_from(key_der)
                                .expect("Invalid private key"),
                        ) {
                        Ok(cfg) => {
                            eprintln!("[Synapsis] Self-signed TLS configured");
                            Some(cfg)
                        }
                        Err(e) => {
                            eprintln!("[Synapsis] Failed to build self-signed TLS: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[Synapsis] Failed to generate self-signed cert: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ => None,
    };

    let proto = if tls_config.is_some() {
        "HTTPS"
    } else {
        "HTTP"
    };

    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Synapsis v{} - Multi-Agent MCP Server            ║",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!(
        "║  Transport: {}/SSE (port {})                      ║",
        proto, port
    );
    eprintln!("║  Multi-Agent: enabled                                  ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝");

    let license_status = synapsis::core::license::current_license_status();
    if license_status.starts_with("License: NOT FOUND") {
        eprintln!("[Synapsis] {}", license_status);
    }

    let db = Arc::new(synapsis::infrastructure::database::Database::new());
    let orchestrator = Arc::new(synapsis::core::orchestrator::Orchestrator::new());
    let server = Arc::new(synapsis::presentation::mcp::McpServer::new(
        db,
        orchestrator,
    ));
    server.init();

    let transport = match tls_config {
        Some(cfg) => synapsis::presentation::http::HttpTransport::with_tls(server, cfg),
        None => synapsis::presentation::http::HttpTransport::new(server),
    };
    transport.start(port);
}
