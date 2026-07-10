//! Synapsis Unified MCP Server
//!
//! Single server that supports:
//! - stdio transport (local MCP clients)
//! - HTTP/SSE transport (multi-agent remote)
//! - QUIC transport (encrypted, cross-platform)
//!
//! No raw TCP - only MCP standard and QUIC.

use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = 7438;
    let mut quic_port: u16 = 7439;
    let mut http_mode = false;
    let mut quic_mode = false;
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--http" | "-h" => http_mode = true,
            "--quic" | "-q" => quic_mode = true,
            "--port" | "-p" => {
                if let Some(p) = args.get(i + 1) {
                    port = p.parse().unwrap_or(7438);
                    i += 1;
                }
            }
            "--quic-port" => {
                if let Some(p) = args.get(i + 1) {
                    quic_port = p.parse().unwrap_or(7439);
                    i += 1;
                }
            }
            "--tls-cert" => {
                if let Some(p) = args.get(i + 1) {
                    tls_cert = Some(p.clone());
                    i += 1;
                }
            }
            "--tls-key" => {
                if let Some(p) = args.get(i + 1) {
                    tls_key = Some(p.clone());
                    i += 1;
                }
            }
            "--help" => {
                println!("Synapsis MCP Server");
                println!("Usage:");
                println!("  synapsis-server                       Start MCP server (stdio)");
                println!("  synapsis-server --http                Start MCP server with HTTP/SSE");
                println!(
                    "  synapsis-server --http --port PORT    Custom HTTP port (default: 7438)"
                );
                println!("  synapsis-server --quic                Start MCP server with QUIC");
                println!(
                    "  synapsis-server --quic --quic-port PORT Custom QUIC port (default: 7439)"
                );
                println!("");
                println!("TLS options (with --http):");
                println!("  --tls-cert <path>                    TLS certificate file");
                println!("  --tls-key <path>                     TLS private key file");
                println!("  If --tls-cert is set without --tls-key, a self-signed cert is used.");
                println!("");
                println!("Env vars:");
                println!("  SYNAPSIS_PORT");
                println!("  SYNAPSIS_TLS_CERT");
                println!("  SYNAPSIS_TLS_KEY");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    // Check env vars (lower priority than CLI)
    if tls_cert.is_none() {
        tls_cert = std::env::var("SYNAPSIS_TLS_CERT").ok();
    }
    if tls_key.is_none() {
        tls_key = std::env::var("SYNAPSIS_TLS_KEY").ok();
    }

    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Synapsis v{} - Unified MCP Server               ║",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!("╠══════════════════════════════════════════════════════════╣");

    let state = synapsis::infrastructure::shared_state::SharedState::new();
    state.init();
    let server = Arc::new(synapsis::presentation::mcp::McpServer::new(
        state.db.clone(),
        Arc::new(synapsis::core::orchestrator::Orchestrator::new()),
    ));
    server.init();

    // Run task cleanup on startup
    if let Ok(report) =
        synapsis::core::task_cleanup::TaskCleanupManager::new(state.db.clone()).run_cleanup()
    {
        if report.total_removed() > 0 {
            eprintln!(
                "[Synapsis] Startup cleanup: removed {} stale tasks",
                report.total_removed()
            );
        }
    }

    if http_mode {
        let tls_config = match (tls_cert, tls_key) {
            (Some(ref cert_path), Some(ref key_path)) => {
                match synapsis::presentation::http::load_tls_config(cert_path, key_path) {
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
            (Some(_), None) => {
                eprintln!("[Synapsis] TLS cert set without key — generating self-signed cert");
                match synapsis::presentation::http::generate_self_signed_cert() {
                    Ok((cert_der, key_der)) => {
                        match rustls::ServerConfig::builder()
                            .with_no_client_auth()
                            .with_single_cert(
                                vec![rustls::pki_types::CertificateDer::from(cert_der)],
                                rustls::pki_types::PrivateKeyDer::try_from(key_der)
                                    .expect("Invalid private key"),
                            )
                        {
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
        eprintln!(
            "║  Transport: {}/SSE (port {})                      ║",
            proto, port
        );
        eprintln!("╚══════════════════════════════════════════════════════════╝");
        let transport = match tls_config {
            Some(cfg) => synapsis::presentation::http::HttpTransport::with_tls(server, cfg),
            None => synapsis::presentation::http::HttpTransport::new(server),
        };
        transport.start(port);
    } else if quic_mode {
        eprintln!(
            "║  Transport: QUIC (port {})                     ║",
            quic_port
        );
        eprintln!("╚══════════════════════════════════════════════════════════╝");

        // Start mDNS discovery for local network peers
        if std::env::var("SYNAPSIS_NO_DISCOVERY").is_err() {
            if let Ok(discovery) = synapsis::core::discovery_net::NetworkDiscovery::new() {
                let _ = discovery.start_scan();
                eprintln!("[Synapsis] mDNS discovery started");
            }
        }

        let transport = synapsis::presentation::quic::QuicTransport::new(server);
        transport.start(quic_port);
    } else {
        eprintln!("║  Transport: stdio                                    ║");
        eprintln!("╚══════════════════════════════════════════════════════════╝");
        if let Err(e) = server.run() {
            eprintln!("MCP Server error: {}", e);
            std::process::exit(1);
        }
    }
}
