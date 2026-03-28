//! Synapsis Comm - Inter-agent communication CLI
//!
//! Allows sending broadcast messages and checking events from command line

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "broadcast" => cmd_broadcast(&args[2..]),
        "poll" => cmd_poll(&args[2..]),
        "agents" => cmd_agents(),
        "help" | "--help" | "-h" => print_usage(),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
        }
    }
}

fn print_usage() {
    println!("Synapsis Comm - Inter-agent Communication CLI");
    println!();
    println!("Usage:");
    println!("  synapsis comm broadcast <message> [--channel <channel>] [--project <project>] [--priority <0-2>]");
    println!("  synapsis comm poll [--since <timestamp>] [--channel <channel>]");
    println!("  synapsis comm agents [--project <project>]");
    println!();
    println!("Commands:");
    println!("  broadcast  Send a message to all agents in the channel");
    println!("  poll       Poll for new events");
    println!("  agents     List active agents");
    println!();
    println!("Examples:");
    println!("  synapsis comm broadcast \"Starting deployment\" --channel deploy --priority 1");
    println!("  synapsis comm poll --since 0 --channel global");
    println!("  synapsis comm agents --project default");
}

fn cmd_broadcast(args: &[String]) {
    if args.is_empty() {
        eprintln!("Error: Message required");
        println!("Usage: synapsis comm broadcast <message> [--channel <channel>]");
        return;
    }

    let mut message = String::new();
    let mut channel = "global".to_string();
    let mut project = None;
    let mut priority = 0;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--channel" | "-c" => {
                if i + 1 < args.len() {
                    channel = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --channel requires a value");
                    return;
                }
            }
            "--project" | "-p" => {
                if i + 1 < args.len() {
                    project = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --project requires a value");
                    return;
                }
            }
            "--priority" | "-P" => {
                if i + 1 < args.len() {
                    priority = args[i + 1].parse().unwrap_or(0);
                    i += 2;
                } else {
                    eprintln!("Error: --priority requires a value");
                    return;
                }
            }
            _ => {
                if !message.is_empty() {
                    message.push(' ');
                }
                message.push_str(&args[i]);
                i += 1;
            }
        }
    }

    if message.is_empty() {
        eprintln!("Error: Message required");
        return;
    }

    // Get session ID from environment or generate one
    let session_id =
        env::var("SYNAPSIS_SESSION_ID").unwrap_or_else(|_| format!("cli-{}", process::id()));

    // Use synapsis MCP to send broadcast
    // For now, we'll write directly to the database
    broadcast_message(
        &session_id,
        &message,
        &channel,
        project.as_deref(),
        priority,
    );
}

fn broadcast_message(
    session_id: &str,
    message: &str,
    channel: &str,
    project: Option<&str>,
    priority: i32,
) {
    use synapsis_core::infrastructure::database::Database;

    let db = Database::new();
    let _now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    match db.broadcast_event(
        "cli_message",
        session_id,
        project,
        channel,
        &format!(
            r#"{{"type":"cli_message","content":"{}"}}"#,
            message.replace('"', "\\\"")
        ),
        priority,
    ) {
        Ok(event_id) => {
            println!(
                "✓ Broadcast sent (event_id: {}, channel: '{}')",
                event_id, channel
            );
            println!("  Message: {}", message);
            if let Some(proj) = project {
                println!("  Project: {}", proj);
            }
            println!("  Priority: {}", priority);
        }
        Err(e) => {
            eprintln!("✗ Error sending broadcast: {}", e);
        }
    }
}

fn cmd_poll(args: &[String]) {
    let mut since = 0i64;
    let mut channel = None;
    let mut project = None;
    let mut limit = 20;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--since" | "-s" => {
                if i + 1 < args.len() {
                    since = args[i + 1].parse().unwrap_or(0);
                    i += 2;
                }
            }
            "--channel" | "-c" => {
                if i + 1 < args.len() {
                    channel = Some(args[i + 1].clone());
                    i += 2;
                }
            }
            "--project" | "-p" => {
                if i + 1 < args.len() {
                    project = Some(args[i + 1].clone());
                    i += 2;
                }
            }
            "--limit" | "-l" => {
                if i + 1 < args.len() {
                    limit = args[i + 1].parse().unwrap_or(20);
                    i += 2;
                }
            }
            _ => i += 1,
        }
    }

    use synapsis_core::infrastructure::database::Database;

    let db = Database::new();

    match db.poll_events(since, channel.as_deref(), project.as_deref(), limit) {
        Ok(events) => {
            if events.is_empty() {
                println!("No new events");
            } else {
                println!("Events ({}):", events.len());
                for event in events {
                    let event_type = event["event_type"].as_str().unwrap_or("unknown");
                    let from = event["from"].as_str().unwrap_or("unknown");
                    let content = event["content"].as_str().unwrap_or("");
                    let timestamp = event["timestamp"].as_i64().unwrap_or(0);
                    let channel = event["channel"].as_str().unwrap_or("global");

                    println!();
                    println!(
                        "  [{}] {} @ {} -> channel:{}",
                        event_type,
                        from,
                        format_timestamp(timestamp),
                        channel
                    );
                    println!("    {}", content);
                }
            }
        }
        Err(e) => {
            eprintln!("Error polling events: {}", e);
        }
    }
}

fn cmd_agents() {
    use synapsis_core::infrastructure::database::Database;

    let db = Database::new();

    match db.get_active_agents(None) {
        Ok(agents) => {
            if agents.is_empty() {
                println!("No active agents");
            } else {
                println!("Active agents ({}):", agents.len());
                for agent in agents {
                    let session_id = agent["session_id"].as_str().unwrap_or("unknown");
                    let agent_type = agent["agent_type"].as_str().unwrap_or("unknown");
                    let instance = agent["instance"].as_str().unwrap_or("unknown");
                    let project = agent["project"].as_str().unwrap_or("default");
                    let last_heartbeat = agent["last_heartbeat"].as_i64().unwrap_or(0);
                    let current_task = agent["current_task"].as_str().unwrap_or("idle");

                    println!();
                    println!("  {} ({})", session_id, agent_type);
                    println!("    Instance: {} | Project: {}", instance, project);
                    println!("    Task: {}", current_task);
                    println!("    Last heartbeat: {}", format_timestamp(last_heartbeat));
                }
            }
        }
        Err(e) => {
            eprintln!("Error getting agents: {}", e);
        }
    }
}

fn format_timestamp(ts: i64) -> String {
    use std::time::UNIX_EPOCH;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let diff = now - ts;

    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

// Helper to get process ID
mod process {
    pub fn id() -> u32 {
        std::process::id()
    }
}
