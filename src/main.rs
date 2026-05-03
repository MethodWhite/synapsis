//! Synapsis Main Entry Point
//!
//! Unified CLI that handles MCP server, memory operations, and self-updates.

use std::sync::Arc;
use synapsis_core::infrastructure::database::Database;
use synapsis_core::core::orchestrator::Orchestrator;
use synapsis::presentation::mcp::McpServer;
use synapsis::app_core::updater::AutoUpdater;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    
    // Parse global config and command
    let mut parser = synapsis::cli::ArgParser::new(&args);
    let (config, command) = match parser.parse_all() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error: {}", e);
            synapsis::cli::print_help(None);
            std::process::exit(1);
        }
    };

    // Shared components
    let db = Arc::new(Database::new());
    let orchestrator = Arc::new(Orchestrator::new());

    match command {
        synapsis::cli::Command::Update => {
            let updater = AutoUpdater::new(db.clone());
            println!("🚀 Checking for updates...");
            match updater.perform_update() {
                Ok(_) => println!("✅ Synapsis is up to date."),
                Err(e) => eprintln!("❌ Update failed: {}", e),
            }
        }
        synapsis::cli::Command::Mcp(_opts) => {
            let server = McpServer::new(db.clone(), orchestrator.clone());
            server.init();
            
            eprintln!("╔══════════════════════════════════════════════════════════╗");
            eprintln!(
                "║  Synapsis v{} - MCP Server                           ║",
                env!("CARGO_PKG_VERSION")
            );
            eprintln!("║  Mode: Pure stdio (Standard)                        ║");
            eprintln!("╚══════════════════════════════════════════════════════════╝");
            eprintln!();

            server.run()?;
        }
        synapsis::cli::Command::Help(opts) => {
            synapsis::cli::print_help(opts.command.as_deref());
        }
        _ => {
            // Placeholder for other commands (save, search, etc.)
            // Most of these are already handled in synapsis-core/src/api/cli
            eprintln!("Command not yet fully integrated in unified binary. Use synapsis-mcp for now.");
        }
    }

    Ok(())
}
