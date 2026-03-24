//! Synapsis CLI - Robust Command Line Interface

use synapsis_core::domain::entities::{Observation, SearchParams};
use synapsis_core::domain::ports::StoragePort;
use synapsis_core::domain::types::{ObservationId, ObservationType, SessionId};
use synapsis_core::infrastructure::database::Database;
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct CLI {
    db: Database,
    verbose: bool,
}

impl CLI {
    pub fn new() -> Self {
        let db = Database::new();
        db.init().expect("Failed to initialize database");
        Self { db, verbose: false }
    }

    pub fn with_verbose(mut self) -> Self {
        self.verbose = true;
        self
    }

    pub fn run(&self) -> i32 {
        let args: Vec<String> = env::args().collect();

        if args.len() == 1 {
            self.print_banner();
            self.print_help();
            return 0;
        }

        let mut cmd_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();

        while let Some(arg) = cmd_args.first() {
            match *arg {
                "-h" | "--help" => {
                    self.print_help();
                    return 0;
                }
                "-V" | "--version" => {
                    println!("synapsis {}", VERSION);
                    return 0;
                }
                "-v" | "--verbose" => {
                    cmd_args.remove(0);
                    continue;
                }
                _ => break,
            }
        }

        if cmd_args.is_empty() {
            eprintln!("Error: No command specified");
            self.print_help();
            return 1;
        }

        let result = match cmd_args[0] {
            "add" => self.cmd_add(&cmd_args[1..]),
            "search" | "s" => self.cmd_search(&cmd_args[1..]),
            "timeline" | "tl" | "t" => self.cmd_timeline(&cmd_args[1..]),
            "sessions" | "ls" => self.cmd_sessions(&cmd_args[1..]),
            "stats" => self.cmd_stats(),
            "get" => self.cmd_get(&cmd_args[1..]),
            "delete" | "rm" => self.cmd_delete(&cmd_args[1..]),
            "export" => self.cmd_export(&cmd_args[1..]),
            "import" => self.cmd_import(&cmd_args[1..]),
            "init" => self.cmd_init(),
            "serve" | "server" => self.cmd_serve(),
            "help" => {
                self.print_help();
                Ok(())
            }
            _ => {
                eprintln!("Error: Unknown command '{}'", cmd_args[0]);
                eprintln!("Run 'synapsis help' for usage");
                Err(1)
            }
        };

        match result {
            Ok(_) => {
                if self.verbose {
                    eprintln!("[OK] Command completed successfully");
                }
                0
            }
            Err(code) => code,
        }
    }

    fn cmd_add(&self, args: &[&str]) -> Result<(), i32> {
        if args.is_empty() {
            eprintln!("Error: 'add' requires arguments");
            eprintln!("Usage: synapsis add [options] <title> <content>");
            eprintln!("Options:");
            eprintln!("  --type <type>    Observation type (default: manual)");
            eprintln!("  --project <proj> Project name");
            eprintln!("  --session <id>   Session ID");
            return Err(1);
        }

        let mut title = String::new();
        let mut content = String::new();
        let mut obs_type = ObservationType::Manual;
        let mut project: Option<String> = None;
        let mut session_id = "cli".to_string();

        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "--type" | "-t" if i + 1 < args.len() => {
                    obs_type = match args[i + 1].to_lowercase().as_str() {
                        "tool" | "tooluse" => ObservationType::ToolUse,
                        "file" | "filechange" => ObservationType::FileChange,
                        "command" | "cmd" => ObservationType::Command,
                        "search" => ObservationType::Search,
                        "decision" => ObservationType::Decision,
                        "arch" | "architecture" => ObservationType::Architecture,
                        "bugfix" | "bug" => ObservationType::Bugfix,
                        "pattern" => ObservationType::Pattern,
                        "config" => ObservationType::Config,
                        "discovery" => ObservationType::Discovery,
                        "learning" => ObservationType::Learning,
                        _ => ObservationType::Manual,
                    };
                    i += 2;
                }
                "--project" | "-p" if i + 1 < args.len() => {
                    project = Some(args[i + 1].to_string());
                    i += 2;
                }
                "--session" if i + 1 < args.len() => {
                    session_id = args[i + 1].to_string();
                    i += 2;
                }
                _ => {
                    if title.is_empty() {
                        title = args[i].to_string();
                    } else if content.is_empty() {
                        content = args[i].to_string();
                    } else {
                        content.push(' ');
                        content.push_str(args[i]);
                    }
                    i += 1;
                }
            }
        }

        if title.is_empty() {
            eprintln!("Error: Title is required");
            return Err(1);
        }

        if content.is_empty() {
            eprintln!("Error: Content is required");
            return Err(1);
        }

        let mut obs = Observation::new(SessionId::new(session_id), obs_type, title.clone(), content);
        obs.project = project;

        match self.db.save_observation(&obs) {
            Ok(id) => {
                println!("Added observation {}", id.0);
                println!("Title: {}", title);
                if self.verbose {
                    println!("Type: {:?}", obs_type);
                }
                Ok(())
            }
            Err(e) => {
                eprintln!("Error adding observation: {}", e);
                Err(1)
            }
        }
    }

    fn cmd_search(&self, args: &[&str]) -> Result<(), i32> {
        if args.is_empty() {
            eprintln!("Error: 'search' requires a query");
            eprintln!("Usage: synapsis search [options] <query>");
            eprintln!("Options:");
            eprintln!("  --type <type>    Filter by type");
            eprintln!("  --project <proj> Filter by project");
            eprintln!("  --limit <n>     Max results (default: 20)");
            return Err(1);
        }

        let mut query = String::new();
        let mut obs_type: Option<ObservationType> = None;
        let mut project: Option<String> = None;
        let mut limit = 20i32;

        let mut i = 0;
        while i < args.len() {
            match args[i] {
                "--type" if i + 1 < args.len() => {
                    obs_type = Some(match args[i + 1].to_lowercase().as_str() {
                        "tool" => ObservationType::ToolUse,
                        "file" => ObservationType::FileChange,
                        "command" => ObservationType::Command,
                        "search" => ObservationType::Search,
                        "decision" => ObservationType::Decision,
                        _ => ObservationType::Manual,
                    });
                    i += 2;
                }
                "--project" | "-p" if i + 1 < args.len() => {
                    project = Some(args[i + 1].to_string());
                    i += 2;
                }
                "--limit" | "-l" if i + 1 < args.len() => {
                    if let Ok(n) = args[i + 1].parse() {
                        limit = n;
                    }
                    i += 2;
                }
                _ => {
                    if !query.is_empty() {
                        query.push(' ');
                    }
                    query.push_str(args[i]);
                    i += 1;
                }
            }
        }

        let params = SearchParams {
            query,
            obs_type,
            project,
            scope: None,
            limit,
        };

        let results = self.db.search_observations(&params).map_err(|e| {
            eprintln!("Search error: {}", e);
            1
        })?;

        if results.is_empty() {
            println!("No results found");
            return Ok(());
        }

        println!("Found {} result(s):", results.len());
        println!("{}", "─".repeat(50));

        for (i, result) in results.iter().enumerate() {
            let obs = &result.observation;
            let truncated = if obs.content.len() > 80 {
                format!("{}...", &obs.content[..80])
            } else {
                obs.content.clone()
            };
            println!("{}. [{}] {}", i + 1, obs.observation_type, truncated);
            println!("   Title: {}", obs.title);
            println!();
        }

        Ok(())
    }

    fn cmd_timeline(&self, args: &[&str]) -> Result<(), i32> {
        let limit = args
            .first()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20)
            .min(1000) as usize;

        let entries = self.db.get_timeline(limit as i32).map_err(|e| {
            eprintln!("Error: {}", e);
            1
        })?;

        if entries.is_empty() {
            println!("No observations yet. Run 'synapsis add' to create one.");
            return Ok(());
        }

        println!("Timeline (last {} observations):", entries.len());
        println!("{}", "─".repeat(60));

        for (i, entry) in entries.iter().enumerate() {
            let obs = &entry.observation;
            let date = format_timestamp(obs.created_at.0);
            let truncated = if obs.title.len() > 40 {
                format!("{}...", &obs.title[..40])
            } else {
                obs.title.clone()
            };
            println!(
                "{:3}. {} | {:15} | {}",
                i + 1,
                date,
                format!("{:?}", obs.observation_type),
                truncated
            );
        }

        if self.verbose {
            println!("{}", "─".repeat(60));
            println!("Total: {} observations", entries.len());
        }

        Ok(())
    }

    fn cmd_sessions(&self, _args: &[&str]) -> Result<(), i32> {
        use synapsis_core::domain::ports::SessionPort;
        
        let sessions = self.db.list_sessions().map_err(|e| {
            eprintln!("Error: {}", e);
            1
        })?;

        if sessions.is_empty() {
            println!("No sessions found");
            return Ok(());
        }

        println!("Sessions:");
        println!("{}", "─".repeat(60));

        for session in &sessions {
            let status = if session.ended_at.is_some() {
                "ended"
            } else {
                "active"
            };
            let date = format_timestamp(session.started_at.0);
            println!(
                "{} | {} | {} | {} obs",
                session.id.as_str(),
                date,
                status,
                session.observation_count
            );
        }

        Ok(())
    }

    fn cmd_stats(&self) -> Result<(), i32> {
        let stats = self.db.stats().unwrap_or(serde_json::json!({}));

        println!("Synapsis Statistics");
        println!("{}", "═".repeat(40));
        println!("Total observations: {}", stats["total_observations"].as_i64().unwrap_or(0));
        println!("Total sessions:     {}", stats["total_sessions"].as_i64().unwrap_or(0));
        println!("Active sessions:     {}", stats["active_sessions"].as_i64().unwrap_or(0));
        println!("Deleted items:      {}", stats["deleted_observations"].as_i64().unwrap_or(0));

        if let Some(projects) = stats["projects"].as_array() {
            let project_list: Vec<String> = projects.iter().filter_map(|p| p.as_str()).map(|s| s.to_string()).collect();
            if !project_list.is_empty() {
                println!("Projects:           {}", project_list.join(", "));
            }
        }

        Ok(())
    }

    fn cmd_get(&self, args: &[&str]) -> Result<(), i32> {
        if args.is_empty() {
            eprintln!("Usage: synapsis get <id>");
            return Err(1);
        }

        let id: i64 = match args[0].parse() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("Error: '{}' is not a valid ID", args[0]);
                return Err(1);
            }
        };

        let obs = self
            .db
            .get_observation(ObservationId::new(id))
            .map_err(|e| {
                eprintln!("Error: {}", e);
                1
            })?;

        match obs {
            Some(o) => {
                println!("{}", "═".repeat(60));
                println!("ID:         {}", o.id.0);
                println!("Type:       {:?}", o.observation_type);
                println!("Title:      {}", o.title);
                println!("Session:    {}", o.session_id.as_str());
                println!("Created:    {}", format_timestamp(o.created_at.0));
                println!("{}", "─".repeat(60));
                println!("Content:");
                println!("{}", o.content);
                println!("{}", "─".repeat(60));
                Ok(())
            }
            None => {
                eprintln!("Error: Observation {} not found", id);
                Err(1)
            }
        }
    }

    fn cmd_delete(&self, args: &[&str]) -> Result<(), i32> {
        if args.is_empty() {
            eprintln!("Usage: synapsis delete <id>");
            return Err(1);
        }
        eprintln!("Delete not yet implemented");
        eprintln!("(Observations are soft-deleted in full implementation)");
        Ok(())
    }

    fn cmd_export(&self, args: &[&str]) -> Result<(), i32> {
        let path = args.first().copied().unwrap_or("synapsis-export.json");

        let timeline = self.db.get_timeline(10000).map_err(|e| {
            eprintln!("Database error: {}", e);
            1
        })?;

        let data = serde_json::to_string_pretty(&timeline).map_err(|e| {
            eprintln!("Serialization error: {}", e);
            1
        })?;

        std::fs::write(path, data).map_err(|e| {
            eprintln!("Write error: {}", e);
            1
        })?;

        println!("Exported to {}", path);
        Ok(())
    }

    fn cmd_import(&self, _args: &[&str]) -> Result<(), i32> {
        eprintln!("Import not yet implemented");
        Ok(())
    }

    fn cmd_init(&self) -> Result<(), i32> {
        println!("Initializing Synapsis...");
        println!("Data directory: ~/.local/share/synapsis/");
        println!("Database initialized successfully");
        Ok(())
    }

    fn cmd_serve(&self) -> Result<(), i32> {
        eprintln!("MCP server not yet implemented");
        eprintln!("Run 'synapsis-mcp' binary when available");
        Ok(())
    }

    fn print_banner(&self) {
        println!();
        println!("  ╔══════════════════════════════════════╗");
        println!("  ║     Synapsis v{} - Memory Engine     ║", VERSION);
        println!("  ╚══════════════════════════════════════╝");
        println!();
    }

    fn print_help(&self) {
        println!("Usage: synapsis <command> [options]");
        println!();
        println!("Commands:");
        println!("  add <title> <content>     Add observation");
        println!("  search <query>             Search memories");
        println!("  timeline [limit]           Show recent observations");
        println!("  sessions                   List sessions");
        println!("  stats                      Show statistics");
        println!("  get <id>                   Get observation by ID");
        println!("  delete <id>                Delete observation");
        println!("  export [file]              Export to JSON");
        println!("  import <file>              Import from JSON");
        println!("  init                       Initialize database");
        println!("  serve                      Start MCP server");
        println!();
        println!("Options:");
        println!("  -h, --help                 Show this help");
        println!("  -V, --version              Show version");
        println!("  -v, --verbose              Verbose output");
        println!();
        println!("Add options:");
        println!("  --type <type>              Observation type");
        println!("  --project <name>           Project name");
        println!("  --session <id>             Session ID");
        println!();
        println!("Examples:");
        println!("  synapsis add \"Title\" \"My content\"");
        println!("  synapsis search \"rust code\"");
        println!("  synapsis timeline 50");
        println!("  synapsis -v add \"Bug\" \"Fixed issue #123\"");
    }
}

impl Default for CLI {
    fn default() -> Self {
        Self::new()
    }
}

fn format_timestamp(ts: i64) -> String {
    let secs = ts;
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let mins = (rem % 3600) / 60;
    format!("{:3}d {:02}h {:02}m", days, hours, mins)
}
