#![recursion_limit = "512"]

//! Synapsis MCP Bridge
//!
//! stdio transport for local MCP clients (Claude Code, Qwen, OpenCode, etc.)
//! Connects to the shared Synapsis state.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--version" | "-v" => {
                eprintln!("synapsis-mcp v{}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                eprintln!("synapsis-mcp v{}", env!("CARGO_PKG_VERSION"));
                eprintln!("Usage: synapsis-mcp [--version | --help]");
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", args[1]);
                std::process::exit(1);
            }
        }
    }

    let quiet = std::env::var("SYNAPSIS_QUIET").is_ok() || std::env::var("QUIET").is_ok();

    let state = synapsis::infrastructure::shared_state::SharedState::new();
    state.init();
    let server = synapsis::presentation::mcp::McpServer::new(
        state.db.clone(),
        std::sync::Arc::new(synapsis::core::orchestrator::Orchestrator::new()),
    );
    server.init();

    if !quiet {
        eprintln!(
            "[synapsis-mcp] v{} ready (stdio)",
            env!("CARGO_PKG_VERSION")
        );
    }

    if let Err(e) = server.run() {
        eprintln!("[synapsis-mcp] Fatal: {}", e);
        std::process::exit(1);
    }
}
