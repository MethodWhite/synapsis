//! Synapsis MCP Auto-Configurator
//!
//! Detects installed AI development platforms and generates MCP configuration
//! files so they can connect to Synapsis's MCP server.
//!
//! Usage:
//!   synapsis-autoconfig [--apply] [--watch]
//!   synapsis-autoconfig --help

use std::env;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    let is_help = args.iter().any(|a| a == "--help" || a == "-h");
    let apply = args.iter().any(|a| a == "--apply");
    let watch = args.iter().any(|a| a == "--watch");

    if is_help {
        print_help(&args[0]);
        return;
    }

    if watch {
        watch_loop(apply);
    } else {
        run_once(apply);
    }
}

fn print_help(bin: &str) {
    println!("Synapsis MCP Auto-Configurator");
    println!();
    println!("Detects installed AI development platforms and generates MCP");
    println!("configuration files so they can connect to Synapsis's MCP server.");
    println!();
    println!("USAGE:");
    println!("  {bin} [FLAGS]");
    println!();
    println!("FLAGS:");
    println!("  --apply     Actually write config files (default: dry-run only)");
    println!("  --watch     Continuously monitor for new platforms");
    println!("  --help      Show this help message");
    println!();
    println!("EXAMPLES:");
    println!("  {bin}                    Dry-run: show what would be configured");
    println!("  {bin} --apply            Write configs for all detected platforms");
    println!("  {bin} --apply --watch    Monitor and auto-configure new platforms");
}

fn run_once(apply: bool) {
    let report = synapsis::core::mcp_autoconfig::detect_and_generate_configs();
    print_report(&report, apply);

    if !report.generated.is_empty() && apply {
        match synapsis::core::mcp_autoconfig::write_configs(&report, false) {
            Ok(()) => println!("\n✓ Configuration complete."),
            Err(e) => eprintln!("\n✗ Error writing configs: {e}"),
        }
    } else if !report.generated.is_empty() {
        println!("\nUse --apply to write these configs.");
    }
}

fn watch_loop(apply: bool) {
    println!("🔍 Watching for new AI platforms... (Ctrl+C to stop)");
    let mut previous_names: Vec<String> = Vec::new();

    loop {
        let report = synapsis::core::mcp_autoconfig::detect_and_generate_configs();
        let current_names: Vec<String> = report
            .generated
            .iter()
            .map(|e| e.platform_name.clone())
            .collect();

        let new_platforms: Vec<&String> = current_names
            .iter()
            .filter(|n| !previous_names.contains(n))
            .collect();

        for name in &new_platforms {
            println!("  ⚡ New platform detected: {name}");
        }

        if !new_platforms.is_empty() && apply
            && let Err(e) = synapsis::core::mcp_autoconfig::write_configs(&report, false) {
                eprintln!("  ✗ Error writing config: {e}");
            }

        previous_names = current_names;
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn print_report(report: &synapsis::core::mcp_autoconfig::AutoConfigReport, apply: bool) {
    let mode = if apply { "APPLY" } else { "DRY-RUN" };
    println!("╔══════════════════════════════════════════════╗");
    println!("║  Synapsis MCP Auto-Configurator [{mode:>7}]  ║");
    println!("╚══════════════════════════════════════════════╝");
    println!();

    if report.generated.is_empty() {
        println!("  No AI platforms detected.");
    } else {
        println!("  Platforms to configure:");
        for entry in &report.generated {
            println!("    • {} -> {}", entry.platform_name, entry.config_path);
        }
    }

    if !report.skipped.is_empty() {
        println!();
        println!("  Skipped (no config path):");
        for s in &report.skipped {
            println!("    • {s}");
        }
    }
}
