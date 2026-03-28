//! Synapsis MCP Server - primary interface for AI agent coordination
//!
//! This bridge allows IDEs/CLIs that support MCP to connect to the
//! Synapsis TCP server, enabling shared state between all agents.

use std::io::{BufRead, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use synapsis::presentation::mcp::secure_tcp;
use synapsis::presentation::mcp::tcp;

struct Bridge {
    server_url: String,
    connected: bool,
    secure: bool,
    secure_client: Option<secure_tcp::SecureTcpClient>,
}

impl Bridge {
    fn new(server_url: &str, secure: bool) -> Self {
        Self {
            server_url: server_url.to_string(),
            connected: false,
            secure,
            secure_client: None,
        }
    }

    fn connect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.secure {
            let client = secure_tcp::SecureTcpClient::connect(&self.server_url)
                .map_err(|e| format!("Secure connection failed: {}", e))?;
            self.secure_client = Some(client);
            self.connected = true;
        } else {
            let stream = TcpStream::connect(&self.server_url)?;
            stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
            std::io::BufReader::new(stream).lines().next();
            self.connected = true;
        }
        Ok(())
    }

    fn forward(&mut self, request: &str) -> Result<String, Box<dyn std::error::Error>> {
        if !self.connected {
            return Err("Not connected to TCP server".into());
        }

        if self.secure {
            let client = self
                .secure_client
                .as_mut()
                .ok_or("Secure client not initialized")?;
            client
                .send(request)
                .map_err(|e| format!("Secure send failed: {}", e).into())
        } else {
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
}

fn start_tcp_server(secure: bool, addr: &str) -> Result<Child, Box<dyn std::error::Error>> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("--tcp");
    if secure {
        cmd.arg("--secure");
    } else {
        cmd.arg("--insecure");
    }
    cmd.arg("--tcp-addr").arg(addr);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());

    let child = cmd.spawn()?;
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

fn run_tcp_server(addr: &str, secure: bool) {
    let db = std::sync::Arc::new(synapsis::infrastructure::database::Database::new());
    let orchestrator = std::sync::Arc::new(synapsis::core::orchestrator::Orchestrator::new());
    let server = std::sync::Arc::new(synapsis::presentation::mcp::McpServer::new(
        db,
        orchestrator,
    ));
    server.init();

    if secure {
        eprintln!("[MCP Secure TCP] Starting secure server on {}", addr);
        if let Err(e) = secure_tcp::start_secure_tcp_server(server, addr) {
            eprintln!("Secure TCP Server error: {}", e);
            std::process::exit(1);
        }
    } else {
        eprintln!("[MCP TCP] Starting insecure server on {}", addr);
        if let Err(e) = tcp::start_tcp_server(server, addr) {
            eprintln!("TCP Server error: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_bridge_mode(server_url: &str, auto_start: bool, secure: bool) {
    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!("║  DEPRECATION WARNING: Bridge mode is being phased out    ║");
    eprintln!("║  For local use, run without --bridge flag                ║");
    eprintln!("║  For multi-client TCP, use --tcp flag instead            ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝");

    let _tcp_server: Option<Child> = if auto_start {
        match start_tcp_server(secure, server_url) {
            Ok(child) => {
                println!(
                    "[Bridge] Started TCP server at {} (secure: {})",
                    server_url, secure
                );
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

    let mut bridge = Bridge::new(server_url, secure);

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
    let mut tcp_mode = false;
    let mut tcp_addr = "127.0.0.1:7439".to_string();
    let mut secure = true; // default secure

    for (i, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("Synapsis MCP Server");
                println!();
                println!("Usage:");
                println!(
                    "  synapsis mcp              Start MCP server (local stdio mode) - RECOMMENDED"
                );
                println!("  synapsis mcp --tcp        Start MCP TCP server (multi-client/remote)");
                println!(
                    "  synapsis mcp --tcp-addr ADDR  TCP server address (default: 127.0.0.1:7439)"
                );
                println!("  synapsis mcp --secure     Use secure PQC encryption (TCP only)");
                println!("  synapsis mcp --insecure   Use plaintext TCP (insecure)");
                println!();
                println!("Deprecated:");
                println!("  synapsis mcp --bridge     Connect to TCP server (legacy)");
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
            "--tcp" | "-t" => tcp_mode = true,
            "--tcp-addr" => {
                if let Some(addr) = args.get(i + 1) {
                    tcp_addr = addr.clone();
                }
            }
            "--secure" => secure = true,
            "--insecure" => secure = false,
            _ => {}
        }
    }

    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!(
        "║  Synapsis v{} - MCP Server                           ║",
        env!("CARGO_PKG_VERSION")
    );
    if bridge_mode {
        eprintln!(
            "║  Connecting to TCP server: {}                       ║",
            server_url
        );
        eprintln!(
            "║  Security: {}                                        ║",
            if secure {
                "PQC Encrypted"
            } else {
                "Insecure (Plaintext)"
            }
        );
    } else if tcp_mode {
        eprintln!(
            "║  MCP TCP Server: {}                                ║",
            tcp_addr
        );
        eprintln!(
            "║  Security: {}                                        ║",
            if secure {
                "PQC Encrypted"
            } else {
                "Insecure (Plaintext)"
            }
        );
    } else {
        eprintln!("║  MCP Memory Server (Local Mode - RECOMMENDED)       ║");
    }
    eprintln!("╚══════════════════════════════════════════════════════════╝");
    eprintln!();

    if bridge_mode {
        run_bridge_mode(&server_url, auto_start_server, secure);
    } else if tcp_mode {
        run_tcp_server(&tcp_addr, secure);
    } else {
        run_local_mcp();
    }
}
