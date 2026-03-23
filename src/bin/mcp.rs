//! Synapsis MCP Bridge - connects MCP clients to TCP server
//!
//! This bridge allows IDEs/CLIs that support MCP to connect to the
//! Synapsis TCP server, enabling shared state between all agents.

use std::io::{BufRead, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

struct Bridge {
    server_url: String,
    connected: bool,
}

impl Bridge {
    fn new(server_url: &str) -> Self {
        Self {
            server_url: server_url.to_string(),
            connected: false,
        }
    }

    fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let stream = TcpStream::connect(&self.server_url)?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
        std::io::BufReader::new(stream).lines().next();
        self.connected = true;
        Ok(())
    }

    fn forward(&self, request: &str) -> Result<String, Box<dyn std::error::Error>> {
        if !self.connected {
            return Err("Not connected to TCP server".into());
        }

        let mut stream = TcpStream::connect(&self.server_url)?;
        stream.write_all(request.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;

        let reader = std::io::BufReader::new(stream);
        let mut response = String::new();
        if let Some(Ok(line)) = reader.lines().next() {
            response = line;
        }

        Ok(response)
    }
}

fn start_tcp_server() -> Result<Child, Box<dyn std::error::Error>> {
    let child = Command::new(std::env::current_exe()?)
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    std::thread::sleep(std::time::Duration::from_millis(500));
    Ok(child)
}

fn run_local_mcp() {
    let db = std::sync::Arc::new(synapsis::infrastructure::database::Database::new());
    let orchestrator = std::sync::Arc::new(synapsis::core::orchestrator::Orchestrator::new());
    let server = synapsis::presentation::mcp::McpServer::new(db, orchestrator);
    server.init();

    if let Err(e) = server.run() {
        eprintln!("MCP Server error: {}", e);
        std::process::exit(1);
    }
}

fn run_bridge_mode(server_url: &str, auto_start: bool) {
    let _tcp_server: Option<Child> = if auto_start {
        match start_tcp_server() {
            Ok(child) => {
                println!("[Bridge] Started TCP server");
                Some(child)
            }
            Err(e) => {
                eprintln!("[Bridge] Warning: Could not start TCP server: {}", e);
                None
            }
        }
    } else {
        None
    };

    let mut bridge = Bridge::new(server_url);

    match bridge.connect() {
        Ok(_) => {
            println!("[Bridge] Connected to TCP server at {}", server_url);
        }
        Err(e) => {
            eprintln!("[Bridge] Warning: Could not connect to TCP server: {}", e);
            eprintln!("[Bridge] Falling back to local mode");
            run_local_mcp();
            return;
        }
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let mut writer = std::io::BufWriter::new(stdout);

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        let response = match bridge.forward(line) {
            Ok(resp) => resp,
            Err(e) => serde_json::json!({
                "jsonrpc": "2.0",
                "error": format!("Bridge error: {}", e),
                "id": null
            })
            .to_string(),
        };

        writeln!(writer, "{}", response).ok();
        writer.flush().ok();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut bridge_mode = false;
    let mut auto_start_server = true;
    let mut server_url = "127.0.0.1:7438".to_string();

    for (i, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("Synapsis MCP Bridge");
                println!();
                println!("Usage:");
                println!("  synapsis mcp              Start MCP server (local mode)");
                println!("  synapsis mcp --bridge     Connect MCP to TCP server");
                println!("  synapsis mcp --url HOST   Custom TCP server URL");
                println!("  synapsis mcp --no-server  Don't auto-start TCP server");
                return;
            }
            "--bridge" | "-b" => bridge_mode = true,
            "--url" => {
                if let Some(url) = args.get(i + 1) {
                    server_url = url.clone();
                }
            }
            "--no-server" => auto_start_server = false,
            _ => {}
        }
    }

    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Synapsis v{} - MCP Bridge                          ║",
        env!("CARGO_PKG_VERSION")
    );
    if bridge_mode {
        eprintln!(
            "║  Connecting to TCP server: {}                       ║",
            server_url
        );
    } else {
        eprintln!("║  MCP Memory Server (Local Mode)                     ║");
    }
    eprintln!("╚══════════════════════════════════════════════════════════╝");
    eprintln!();

    if bridge_mode {
        run_bridge_mode(&server_url, auto_start_server);
    } else {
        run_local_mcp();
    }
}
