//! Synapsis CLI - Command Line Interface
//!
//! CLI completa con subcomandos para todas las operaciones.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

/// CLI Configuration
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub data_dir: String,
    pub port: u16,
    pub log_level: LogLevel,
    pub color: bool,
    pub json: bool,
    pub quiet: bool,
    pub verbose: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            port: 7438,
            log_level: LogLevel::Info,
            color: true,
            json: false,
            quiet: false,
            verbose: false,
        }
    }
}

fn default_data_dir() -> String {
    std::env::var("SYNAPSIS_DATA_DIR")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.synapsis", h)))
        .unwrap_or_else(|_| ".synapsis".to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" | "err" => Self::Error,
            "warn" | "warning" => Self::Warn,
            "info" => Self::Info,
            "debug" | "dbg" => Self::Debug,
            "trace" | "verbose" => Self::Trace,
            _ => Self::Info,
        }
    }
}

/// CLI Commands
#[derive(Debug, Clone)]
pub enum Command {
    Serve(ServeOpts),
    Mcp(McpOpts),
    Save(SaveOpts),
    Search(SearchOpts),
    Stats(StatsOpts),
    Session(SessionOpts),
    Export(ExportOpts),
    Import(ImportOpts),
    Sync(SyncOpts),
    Config(ConfigOpts),
    Help(HelpOpts),
}

#[derive(Debug, Clone)]
pub struct ServeOpts {
    pub port: u16,
    pub host: String,
    pub workers: u32,
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct McpOpts {
    pub tools: Option<String>,
    pub profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SaveOpts {
    pub title: String,
    pub content: String,
    pub obs_type: Option<String>,
    pub project: Option<String>,
    pub scope: Option<String>,
    pub topic_key: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub query: String,
    pub obs_type: Option<String>,
    pub project: Option<String>,
    pub scope: Option<String>,
    pub limit: Option<u32>,
    pub format: OutputFormat,
}

#[derive(Debug, Clone)]
pub enum OutputFormat {
    Text,
    Json,
    Table,
}

#[derive(Debug, Clone)]
pub struct StatsOpts {
    pub project: Option<String>,
    pub detailed: bool,
    pub format: OutputFormat,
}

#[derive(Debug, Clone)]
pub enum SessionOpts {
    Start {
        project: String,
        directory: Option<String>,
    },
    End {
        session_id: String,
        summary: Option<String>,
    },
    List {
        project: Option<String>,
        limit: Option<u32>,
    },
    Active,
}

#[derive(Debug, Clone)]
pub struct ExportOpts {
    pub path: Option<String>,
    pub format: ExportFormat,
    pub include_deleted: bool,
}

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Json,
    Jsonl,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct ImportOpts {
    pub path: String,
    pub format: ExportFormat,
    pub merge: bool,
}

#[derive(Debug, Clone)]
pub struct SyncOpts {
    pub direction: SyncDirection,
    pub project: Option<String>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub enum SyncDirection {
    Push,
    Pull,
    Both,
}

#[derive(Debug, Clone)]
pub struct ConfigOpts {
    pub action: ConfigAction,
}

#[derive(Debug, Clone)]
pub enum ConfigAction {
    Show,
    Set { key: String, value: String },
    Get { key: String },
    Init,
}

#[derive(Debug, Clone)]
pub struct HelpOpts {
    pub command: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// ARGUMENT PARSER
// ═══════════════════════════════════════════════════════════════════════════

/// Simple argument parser
pub struct ArgParser {
    args: Vec<String>,
    pos: usize,
}

impl ArgParser {
    pub fn new(args: &[String]) -> Self {
        Self {
            args: args.to_vec(),
            pos: 0,
        }
    }

    pub fn parse() -> Result<(CliConfig, Command), CliError> {
        let mut parser = Self::new(&std::env::args().collect::<Vec<_>>());

        // Skip program name
        parser.pos = 1;

        let config = parser.parse_global_opts()?;
        let command = parser.parse_command()?;

        Ok((config, command))
    }

    fn parse_global_opts(&mut self) -> Result<CliConfig, CliError> {
        let mut config = CliConfig::default();

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--data-dir" | "-d" => {
                    self.advance();
                    config.data_dir = self.expect_value("--data-dir")?;
                }
                "--port" | "-p" => {
                    self.advance();
                    config.port = self
                        .expect_value("--port")?
                        .parse()
                        .map_err(|_| CliError::InvalidValue("--port".to_string()))?;
                }
                "--log-level" | "-l" => {
                    self.advance();
                    config.log_level = LogLevel::from_str(&self.expect_value("--log-level")?);
                }
                "--json" => {
                    config.json = true;
                    self.advance();
                }
                "--quiet" | "-q" => {
                    config.quiet = true;
                    self.advance();
                }
                "--verbose" | "-v" => {
                    config.verbose = true;
                    self.advance();
                }
                "--no-color" => {
                    config.color = false;
                    self.advance();
                }
                "--help" | "-h" => {
                    return Ok((config, Command::Help(HelpOpts { command: None })));
                }
                _ => break,
            }
        }

        Ok(config)
    }

    fn parse_command(&mut self) -> Result<Command, CliError> {
        let cmd = self.expect_value("command")?;

        match cmd.as_str() {
            "serve" | "server" => Ok(Command::Serve(self.parse_serve_opts()?)),
            "mcp" => Ok(Command::Mcp(self.parse_mcp_opts()?)),
            "save" => Ok(Command::Save(self.parse_save_opts()?)),
            "search" => Ok(Command::Search(self.parse_search_opts()?)),
            "stats" => Ok(Command::Stats(self.parse_stats_opts()?)),
            "session" => Ok(Command::Session(self.parse_session_opts()?)),
            "export" => Ok(Command::Export(self.parse_export_opts()?)),
            "import" => Ok(Command::Import(self.parse_import_opts()?)),
            "sync" => Ok(Command::Sync(self.parse_sync_opts()?)),
            "config" => Ok(Command::Config(self.parse_config_opts()?)),
            "help" => Ok(Command::Help(HelpOpts { command: None })),
            _ => Err(CliError::UnknownCommand(cmd)),
        }
    }

    fn parse_serve_opts(&mut self) -> Result<ServeOpts, CliError> {
        let mut opts = ServeOpts {
            port: 7438,
            host: "127.0.0.1".to_string(),
            workers: 1,
            tls: None,
        };

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--port" | "-p" => {
                    self.advance();
                    opts.port = self
                        .expect_value("--port")?
                        .parse()
                        .map_err(|_| CliError::InvalidValue("--port".to_string()))?;
                }
                "--host" | "-h" => {
                    self.advance();
                    opts.host = self.expect_value("--host")?;
                }
                "--workers" | "-w" => {
                    self.advance();
                    opts.workers = self
                        .expect_value("--workers")?
                        .parse()
                        .map_err(|_| CliError::InvalidValue("--workers".to_string()))?;
                }
                "--tls" => {
                    self.advance();
                    opts.tls = Some(TlsConfig {
                        cert: self.expect_value("--tls")?,
                        key: self.expect_value("--tls-key")?,
                    });
                }
                _ => break,
            }
        }

        Ok(opts)
    }

    fn parse_mcp_opts(&mut self) -> Result<McpOpts, CliError> {
        let mut opts = McpOpts {
            tools: None,
            profile: None,
        };

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--tools" | "-t" => {
                    self.advance();
                    opts.tools = Some(self.expect_value("--tools")?);
                }
                "--profile" => {
                    self.advance();
                    opts.profile = Some(self.expect_value("--profile")?);
                }
                _ => break,
            }
        }

        Ok(opts)
    }

    fn parse_save_opts(&mut self) -> Result<SaveOpts, CliError> {
        let title = self.expect_value("title")?;
        let mut opts = SaveOpts {
            title,
            content: String::new(),
            obs_type: None,
            project: None,
            scope: None,
            topic_key: None,
            session_id: None,
        };

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--content" | "-c" => {
                    self.advance();
                    opts.content = self.expect_value("--content")?;
                }
                "--type" | "-t" => {
                    self.advance();
                    opts.obs_type = Some(self.expect_value("--type")?);
                }
                "--project" => {
                    self.advance();
                    opts.project = Some(self.expect_value("--project")?);
                }
                "--scope" => {
                    self.advance();
                    opts.scope = Some(self.expect_value("--scope")?);
                }
                "--topic" => {
                    self.advance();
                    opts.topic_key = Some(self.expect_value("--topic")?);
                }
                "--session" | "-s" => {
                    self.advance();
                    opts.session_id = Some(self.expect_value("--session")?);
                }
                _ => break,
            }
        }

        if opts.content.is_empty() {
            opts.content = read_stdin_or_prompt()?;
        }

        Ok(opts)
    }

    fn parse_search_opts(&mut self) -> Result<SearchOpts, CliError> {
        let query = self.expect_value("query")?;
        let mut opts = SearchOpts {
            query,
            obs_type: None,
            project: None,
            scope: None,
            limit: None,
            format: OutputFormat::Text,
        };

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--type" | "-t" => {
                    self.advance();
                    opts.obs_type = Some(self.expect_value("--type")?);
                }
                "--project" => {
                    self.advance();
                    opts.project = Some(self.expect_value("--project")?);
                }
                "--scope" => {
                    self.advance();
                    opts.scope = Some(self.expect_value("--scope")?);
                }
                "--limit" | "-n" => {
                    self.advance();
                    opts.limit = Some(
                        self.expect_value("--limit")?
                            .parse()
                            .map_err(|_| CliError::InvalidValue("--limit".to_string()))?,
                    );
                }
                "--json" => opts.format = OutputFormat::Json,
                "--table" => opts.format = OutputFormat::Table,
                _ => break,
            }
        }

        Ok(opts)
    }

    fn parse_stats_opts(&mut self) -> Result<StatsOpts, CliError> {
        let mut opts = StatsOpts {
            project: None,
            detailed: false,
            format: OutputFormat::Text,
        };

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--project" => {
                    self.advance();
                    opts.project = Some(self.expect_value("--project")?);
                }
                "--detailed" | "-d" => opts.detailed = true,
                "--json" => opts.format = OutputFormat::Json,
                _ => break,
            }
        }

        Ok(opts)
    }

    fn parse_session_opts(&mut self) -> Result<SessionOpts, CliError> {
        let action = self.expect_value("action")?;

        match action.as_str() {
            "start" => Ok(SessionOpts::Start {
                project: self.expect_value("project")?,
                directory: self.next().filter(|a| !a.starts_with('-')),
            }),
            "end" => Ok(SessionOpts::End {
                session_id: self.expect_value("session_id")?,
                summary: self.next().filter(|a| !a.starts_with('-')),
            }),
            "list" | "ls" => Ok(SessionOpts::List {
                project: self.peek().filter(|a| !a.starts_with('-')).map(|s| {
                    self.advance();
                    s.clone();
                    s.clone();
                    s.clone()
                }),
                limit: self.next().and_then(|s| s.parse().ok()),
            }),
            "active" => Ok(SessionOpts::Active),
            _ => Err(CliError::InvalidValue(format!(
                "Unknown session action: {}",
                action
            ))),
        }
    }

    fn parse_export_opts(&mut self) -> Result<ExportOpts, CliError> {
        let mut opts = ExportOpts {
            path: None,
            format: ExportFormat::Json,
            include_deleted: false,
        };

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--path" => {
                    self.advance();
                    opts.path = Some(self.expect_value("--path")?);
                }
                "--format" | "-f" => {
                    self.advance();
                    opts.format = match self.expect_value("--format")?.as_str() {
                        "json" => ExportFormat::Json,
                        "jsonl" => ExportFormat::Jsonl,
                        "md" | "markdown" => ExportFormat::Markdown,
                        _ => return Err(CliError::InvalidValue("--format".to_string())),
                    };
                }
                "--include-deleted" => opts.include_deleted = true,
                _ => break,
            }
        }

        Ok(opts)
    }

    fn parse_import_opts(&mut self) -> Result<ImportOpts, CliError> {
        let path = self.expect_value("path")?;
        let mut opts = ImportOpts {
            path,
            format: ExportFormat::Json,
            merge: true,
        };

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "--format" | "-f" => {
                    self.advance();
                    opts.format = match self.expect_value("--format")?.as_str() {
                        "json" => ExportFormat::Json,
                        "jsonl" => ExportFormat::Jsonl,
                        "md" | "markdown" => ExportFormat::Markdown,
                        _ => return Err(CliError::InvalidValue("--format".to_string())),
                    };
                }
                "--no-merge" => opts.merge = false,
                _ => break,
            }
        }

        Ok(opts)
    }

    fn parse_sync_opts(&mut self) -> Result<SyncOpts, CliError> {
        let mut direction = SyncDirection::Both;

        while let Some(arg) = self.peek() {
            match arg.as_str() {
                "push" => {
                    direction = SyncDirection::Push;
                    self.advance();
                }
                "pull" => {
                    direction = SyncDirection::Pull;
                    self.advance();
                }
                "--project" => {
                    self.advance();
                    self.next(); // consume value
                }
                "--force" | "-f" => {
                    self.advance();
                }
                "--dry-run" | "-n" => {
                    self.advance();
                }
                _ => break,
            }
        }

        Ok(SyncOpts {
            direction,
            project: None,
            force: false,
            dry_run: false,
        })
    }

    fn parse_config_opts(&mut self) -> Result<ConfigOpts, CliError> {
        let action = self.next().unwrap_or_default();

        match action.as_str() {
            "show" | "" => Ok(ConfigOpts {
                action: ConfigAction::Show,
            }),
            "set" => {
                let key = self.expect_value("key")?;
                let value = self.expect_value("value")?;
                Ok(ConfigOpts {
                    action: ConfigAction::Set { key, value },
                })
            }
            "get" => Ok(ConfigOpts {
                action: ConfigAction::Get {
                    key: self.expect_value("key")?,
                },
            }),
            "init" => Ok(ConfigOpts {
                action: ConfigAction::Init,
            }),
            _ => Err(CliError::InvalidValue(format!(
                "Unknown config action: {}",
                action
            ))),
        }
    }

    fn peek(&self) -> Option<&String> {
        self.args.get(self.pos)
    }

    fn advance(&mut self) -> Option<String> {
        if self.pos < self.args.len() {
            let arg = self.args[self.pos].clone();
            self.pos += 1;
            Some(arg)
        } else {
            None
        }
    }

    fn next(&mut self) -> Option<String> {
        self.advance()
    }

    fn expect_value(&mut self, option: &str) -> Result<String, CliError> {
        self.advance()
            .ok_or_else(|| CliError::MissingValue(option.to_string()))
    }
}

fn read_stdin_or_prompt() -> Result<String, CliError> {
    // Try to read from stdin
    if !std::io::stdin().is_tty() {
        let mut input = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
            .map_err(|_| CliError::IoError)?;
        return Ok(input.trim().to_string());
    }

    // For non-interactive use, return empty (will prompt later)
    Ok(String::new())
}

// ═══════════════════════════════════════════════════════════════════════════
// CLI ERROR
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum CliError {
    UnknownCommand(String),
    MissingValue(String),
    InvalidValue(String),
    IoError,
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand(cmd) => write!(f, "Unknown command: {}", cmd),
            Self::MissingValue(opt) => write!(f, "Missing value for option: {}", opt),
            Self::InvalidValue(opt) => write!(f, "Invalid value for option: {}", opt),
            Self::IoError => write!(f, "I/O error"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OUTPUT FORMATTING
// ═══════════════════════════════════════════════════════════════════════════

pub struct Output {
    pub json: bool,
    pub color: bool,
    pub quiet: bool,
}

impl Output {
    pub fn new(config: &CliConfig) -> Self {
        Self {
            json: config.json,
            color: config.color && std::io::stdout().is_tty(),
            quiet: config.quiet,
        }
    }

    pub fn print<T: fmt::Display>(&self, msg: T) {
        if !self.quiet {
            println!("{}", msg);
        }
    }

    pub fn print_json<T: serde::Serialize>(&self, value: &T) {
        if !self.quiet {
            let json = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string());
            println!("{}", json);
        }
    }

    pub fn error<T: fmt::Display>(&self, msg: T) {
        eprintln!("Error: {}", msg);
    }

    pub fn success<T: fmt::Display>(&self, msg: T) {
        if !self.quiet {
            if self.color {
                println!("\x1b[32m✓ {}\x1b[0m", msg);
            } else {
                println!("✓ {}", msg);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HELP
// ═══════════════════════════════════════════════════════════════════════════

pub fn print_help(command: Option<&str>) {
    match command {
        Some("serve") => print_serve_help(),
        Some("mcp") => print_mcp_help(),
        Some("save") => print_save_help(),
        Some("search") => print_search_help(),
        Some("stats") => print_stats_help(),
        Some("session") => print_session_help(),
        Some("export") => print_export_help(),
        Some("import") => print_import_help(),
        Some("sync") => print_sync_help(),
        Some("config") => print_config_help(),
        _ => print_general_help(),
    }
}

fn print_general_help() {
    println!(
        r#"
Synapsis - Persistent Memory for AI Agents

USAGE:
    synapsis <COMMAND> [OPTIONS]

COMMANDS:
    serve          Start HTTP server (default port 7438)
    mcp            Start MCP server (stdio mode)
    save           Save an observation to memory
    search         Search memories
    stats          Show statistics
    session        Manage sessions (start/end/list)
    export         Export data
    import         Import data
    sync           Synchronize with remote
    config         Manage configuration
    help           Show this help

GLOBAL OPTIONS:
    --data-dir     Data directory (default: ~/.synapsis)
    --port         HTTP server port (default: 7438)
    --log-level    Log level: error|warn|info|debug|trace
    --json         Output as JSON
    --quiet        Suppress output
    --verbose      Enable verbose output
    --no-color     Disable colored output
    --help, -h     Show help

EXAMPLES:
    synapsis serve --port 7438
    synapsis mcp --profile agent
    synapsis save "Bug fix" --content "Fixed N+1 query"
    synapsis search "authentication" --limit 20
    synapsis stats --detailed --json
    synapsis session start my-project
    synapsis export --path backup.json
    synapsis sync push --force

For more information, see https://github.com/methodwhite/synapsis
"#
    );
}

fn print_serve_help() {
    println!(
        r#"
synapsis serve - Start HTTP server

USAGE:
    synapsis serve [OPTIONS]

OPTIONS:
    --port, -p     Port to listen on (default: 7438)
    --host         Host to bind to (default: 127.0.0.1)
    --workers      Number of workers (default: 1)
    --tls          Enable TLS with certificate path
    --tls-key      TLS private key path

EXAMPLES:
    synapsis serve
    synapsis serve --port 8080 --host 0.0.0.0
"#
    );
}

fn print_mcp_help() {
    println!(
        r#"
synapsis mcp - Start MCP server

USAGE:
    synapsis mcp [OPTIONS]

OPTIONS:
    --tools        Comma-separated list of tools to enable
    --profile      Use a tool profile: agent|admin|all

PROFILES:
    agent          Core tools for AI agents (11 tools)
    admin          Admin tools for TUI/dashboards (3 tools)
    all            All tools (default)

TOOLS:
    mem_save, mem_search, mem_context, mem_session_summary,
    mem_session_start, mem_session_end, mem_update, mem_delete,
    mem_get_observation, mem_suggest_topic_key, mem_capture_passive,
    mem_save_prompt, mem_stats, mem_timeline

EXAMPLES:
    synapsis mcp
    synapsis mcp --profile agent
    synapsis mcp --tools mem_save,mem_search,mem_context
"#
    );
}

fn print_save_help() {
    println!(
        r#"
synapsis save - Save observation to memory

USAGE:
    synapsis save <title> [OPTIONS]

ARGUMENTS:
    title          Short, searchable title

OPTIONS:
    --content, -c  Content/body of the observation
    --type, -t     Type: manual|bugfix|decision|architecture|pattern|config
    --project      Project name
    --scope        Scope: project|personal
    --topic        Topic key for upserts
    --session, -s  Session ID

EXAMPLES:
    synapsis save "Fixed N+1 query" --content "..." --type bugfix
    synapsis save "JWT auth" --type decision --project myapp
"#
    );
}

fn print_search_help() {
    println!(
        r#"
synapsis search - Search memories

USAGE:
    synapsis search <query> [OPTIONS]

ARGUMENTS:
    query          Search query

OPTIONS:
    --type, -t     Filter by type
    --project      Filter by project
    --scope        Filter by scope
    --limit, -n    Max results (default: 20)
    --json         Output as JSON
    --table        Output as table

EXAMPLES:
    synapsis search "authentication bug"
    synapsis search "config" --type bugfix --limit 50
    synapsis search "api" --json
"#
    );
}

fn print_stats_help() {
    println!(
        r#"
synapsis stats - Show statistics

USAGE:
    synapsis stats [OPTIONS]

OPTIONS:
    --project      Filter by project
    --detailed, -d Show detailed statistics
    --json         Output as JSON

EXAMPLES:
    synapsis stats
    synapsis stats --detailed
    synapsis stats --project myapp --json
"#
    );
}

fn print_session_help() {
    println!(
        r#"
synapsis session - Manage sessions

USAGE:
    synapsis session <action> [OPTIONS]

ACTIONS:
    start <project>     Start a new session
    end <session_id>    End a session
    list                List sessions
    active              Show active sessions

OPTIONS:
    --directory, -d     Working directory

EXAMPLES:
    synapsis session start my-project
    synapsis session end session-123
    synapsis session list --limit 10
    synapsis session active
"#
    );
}

fn print_export_help() {
    println!(
        r#"
synapsis export - Export data

USAGE:
    synapsis export [OPTIONS]

OPTIONS:
    --path             Output file path
    --format, -f       Format: json|jsonl|markdown
    --include-deleted   Include deleted observations

EXAMPLES:
    synapsis export --path backup.json
    synapsis export --format markdown --path export.md
"#
    );
}

fn print_import_help() {
    println!(
        r#"
synapsis import - Import data

USAGE:
    synapsis import <path> [OPTIONS]

ARGUMENTS:
    path               Input file path

OPTIONS:
    --format, -f       Format: json|jsonl|markdown
    --no-merge         Replace instead of merge

EXAMPLES:
    synapsis import backup.json
    synapsis import --format jsonl data.jsonl
"#
    );
}

fn print_sync_help() {
    println!(
        r#"
synapsis sync - Synchronize with remote

USAGE:
    synapsis sync [direction] [OPTIONS]

DIRECTIONS:
    push              Push local changes to remote
    pull              Pull remote changes to local
    (none)            Push and pull

OPTIONS:
    --project          Sync specific project
    --force, -f        Force sync (ignore conflicts)
    --dry-run, -n      Show what would be synced

EXAMPLES:
    synapsis sync
    synapsis sync push
    synapsis sync pull --force
"#
    );
}

fn print_config_help() {
    println!(
        r#"
synapsis config - Manage configuration

USAGE:
    synapsis config <action> [key] [value]

ACTIONS:
    show               Show current configuration
    get <key>          Get configuration value
    set <key> <value>  Set configuration value
    init               Initialize configuration

EXAMPLES:
    synapsis config show
    synapsis config get data_dir
    synapsis config set port 8080
    synapsis config init
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(LogLevel::from_str("error"), LogLevel::Error);
        assert_eq!(LogLevel::from_str("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str("INFO"), LogLevel::Info);
    }
}
pub mod session;
