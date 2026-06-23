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
                eprintln!("Without arguments, starts the HTTP/SSE MCP server on port 7438.");
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", args[1]);
                std::process::exit(1);
            }
        }
    }

    let port: u16 = std::env::var("SYNAPSIS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7438);

    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Synapsis v{} - Multi-Agent MCP Server            ║",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!(
        "║  Transport: HTTP/SSE (port {})                       ║",
        port
    );
    eprintln!("║  Multi-Agent: enabled                                  ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝");

    let db = Arc::new(synapsis::infrastructure::database::Database::new());
    let orchestrator = Arc::new(synapsis::core::orchestrator::Orchestrator::new());
    let server = Arc::new(synapsis::presentation::mcp::McpServer::new(
        db,
        orchestrator,
    ));
    server.init();

    let transport = synapsis::presentation::http::HttpTransport::new(server);
    transport.start(port);
}
