//! TCP server for Synapsis MCP
//!
//! Provides secure TCP transport for MCP protocol with support for
//! multiple concurrent client connections.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use crate::presentation::mcp::McpServer;

/// TCP server for MCP protocol
pub struct TcpServer {
    mcp_server: Arc<McpServer>,
    listener: TcpListener,
}

impl TcpServer {
    /// Create new TCP server bound to address
    pub fn new(mcp_server: Arc<McpServer>, addr: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self {
            mcp_server,
            listener,
        })
    }
    
    /// Start TCP server (blocking)
    pub fn run(&self) -> std::io::Result<()> {
        eprintln!("[MCP TCP] Server listening on {}", self.listener.local_addr()?);
        
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let mcp_server = self.mcp_server.clone();
                    thread::spawn(move || {
                        if let Err(e) = handle_connection(mcp_server, stream) {
                            eprintln!("[MCP TCP] Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[MCP TCP] Accept error: {}", e);
                }
            }
        }
        
        Ok(())
    }
}

/// Handle a single TCP connection
fn handle_connection(mcp_server: Arc<McpServer>, stream: TcpStream) -> std::io::Result<()> {
    let peer_addr = stream.peer_addr()?;
    eprintln!("[MCP TCP] New connection from {}", peer_addr);
    
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    
    // Simple line-based protocol: each line is a JSON-RPC message
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // EOF
                eprintln!("[MCP TCP] Connection closed by {}", peer_addr);
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                
                // Handle message through MCP server
                if let Some(response) = mcp_server.handle_message(line) {
                    if let Err(e) = writeln!(writer, "{}", response) {
                        eprintln!("[MCP TCP] Write error to {}: {}", peer_addr, e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("[MCP TCP] Read error from {}: {}", peer_addr, e);
                break;
            }
        }
    }
    
    Ok(())
}

/// Start TCP server with given MCP server instance
pub fn start_tcp_server(mcp_server: Arc<McpServer>, addr: &str) -> std::io::Result<()> {
    let server = TcpServer::new(mcp_server, addr)?;
    server.run()
}