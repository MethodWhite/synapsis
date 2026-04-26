//! Synapsis MCP Server - primary interface for AI agent coordination
//!
//! This is the standard MCP implementation that uses stdio for communication.
//! All TCP and Bridge modes have been removed to adhere to the MCP standard.

fn run_local_mcp() {
    let db = std::sync::Arc::new(synapsis_core::infrastructure::database::Database::new());
    let orchestrator = std::sync::Arc::new(synapsis_core::core::orchestrator::Orchestrator::new());
    let server = synapsis::presentation::mcp::McpServer::new(db, orchestrator);
    server.init();

    if let Err(e) = server.run() {
        eprintln!("MCP Server error: {}", e);
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Synapsis MCP Server");
        println!();
        println!("Usage:");
        println!("  synapsis mcp              Start MCP server (local stdio mode)");
        return;
    }

    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Synapsis v{} - MCP Server                           ║",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!("║  MCP Memory Server (Local Mode - Standard)          ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝");
    eprintln!();

    run_local_mcp();
}
