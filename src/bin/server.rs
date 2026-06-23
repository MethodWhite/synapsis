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

    for (i, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "--http" | "-h" => http_mode = true,
            "--quic" | "-q" => quic_mode = true,
            "--port" | "-p" => {
                if let Some(p) = args.get(i + 1) {
                    port = p.parse().unwrap_or(7438);
                }
            }
            "--quic-port" => {
                if let Some(p) = args.get(i + 1) {
                    quic_port = p.parse().unwrap_or(7439);
                }
            }
            "--help" => {
                println!("Synapsis MCP Server");
                println!("Usage:");
                println!("  synapsis-server                       Start MCP server (stdio)");
                println!("  synapsis-server --http                Start MCP server with HTTP/SSE");
                println!("  synapsis-server --http --port PORT    Custom HTTP port (default: 7438)");
                println!("  synapsis-server --quic                Start MCP server with QUIC");
                println!("  synapsis-server --quic --quic-port PORT Custom QUIC port (default: 7439)");
                return;
            }
            _ => {}
        }
    }

    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Synapsis v{} - Unified MCP Server               ║",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!("╠══════════════════════════════════════════════════════════╣");

    let db = Arc::new(synapsis::infrastructure::database::Database::new());
    let orchestrator = Arc::new(synapsis::core::orchestrator::Orchestrator::new());
    let server = Arc::new(synapsis::presentation::mcp::McpServer::new(
        db,
        orchestrator,
    ));
    server.init();

    if http_mode {
        eprintln!(
            "║  Transport: HTTP/SSE (port {})                      ║",
            port
        );
        eprintln!("╚══════════════════════════════════════════════════════════╝");
        let transport = synapsis::presentation::http::HttpTransport::new(server);
        transport.start(port);
    } else if quic_mode {
        eprintln!(
            "║  Transport: QUIC (port {})                     ║",
            quic_port
        );
        eprintln!("╚══════════════════════════════════════════════════════════╝");
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
