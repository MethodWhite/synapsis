//! Secure TCP transport for Synapsis MCP using post-quantum cryptography.
//!
//! Provides encrypted, authenticated communication between MCP clients and server
//! using Kyber512 key exchange and AES-256-GCM encryption.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use base64::{engine::general_purpose, Engine as _};

use crate::presentation::mcp::McpServer;
use synapsis_core::core::crypto_provider::SynapsisPqcProvider;
use synapsis_core::core::pqc;
use synapsis_core::domain::crypto::{CryptoProvider, PqcAlgorithm};

/// Secure TCP server for MCP protocol
pub struct SecureTcpServer {
    mcp_server: Arc<McpServer>,
    listener: TcpListener,
    crypto_provider: Arc<dyn CryptoProvider>,
}

impl SecureTcpServer {
    /// Create new secure TCP server bound to address
    pub fn new(mcp_server: Arc<McpServer>, addr: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let crypto_provider: Arc<dyn CryptoProvider> =
            Arc::new(SynapsisPqcProvider::new()) as Arc<dyn CryptoProvider>;
        Ok(Self {
            mcp_server,
            listener,
            crypto_provider,
        })
    }

    /// Start secure TCP server (blocking)
    pub fn run(&self) -> std::io::Result<()> {
        eprintln!(
            "[MCP Secure TCP] Server listening on {}",
            self.listener.local_addr()?
        );

        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let mcp_server = self.mcp_server.clone();
                    let crypto_provider = self.crypto_provider.clone();
                    thread::spawn(move || {
                        if let Err(e) =
                            handle_secure_connection(mcp_server, crypto_provider, stream)
                        {
                            eprintln!("[MCP Secure TCP] Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[MCP Secure TCP] Accept error: {}", e);
                }
            }
        }

        Ok(())
    }
}

/// Handle a single secure TCP connection
fn handle_secure_connection(
    mcp_server: Arc<McpServer>,
    crypto_provider: Arc<dyn CryptoProvider>,
    stream: TcpStream,
) -> std::io::Result<()> {
    let peer_addr = stream.peer_addr()?;
    eprintln!("[MCP Secure TCP] New connection from {}", peer_addr);

    // Perform Kyber key exchange handshake
    let shared_secret = match perform_kyber_handshake(crypto_provider.as_ref(), &stream) {
        Ok(secret) => secret,
        Err(e) => {
            eprintln!(
                "[MCP Secure TCP] Handshake failed with {}: {}",
                peer_addr, e
            );
            return Ok(());
        }
    };

    eprintln!(
        "[MCP Secure TCP] Secure channel established with {}",
        peer_addr
    );

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    // Use shared_secret as AES key (first 32 bytes)
    let mut aes_key = [0u8; 32];
    if shared_secret.len() >= 32 {
        aes_key.copy_from_slice(&shared_secret[..32]);
    } else {
        // Pad with zeros (should not happen with Kyber512)
        let len = shared_secret.len();
        aes_key[..len].copy_from_slice(&shared_secret);
    }

    // Simple line-based protocol with encryption
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // EOF
                eprintln!("[MCP Secure TCP] Connection closed by {}", peer_addr);
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Decrypt the message
                let decrypted = match decrypt_message(crypto_provider.as_ref(), &aes_key, line) {
                    Ok(msg) => msg,
                    Err(e) => {
                        eprintln!(
                            "[MCP Secure TCP] Decryption error from {}: {}",
                            peer_addr, e
                        );
                        break;
                    }
                };

                // Handle message through MCP server
                if let Some(response) = mcp_server.handle_message(&decrypted) {
                    // Encrypt response
                    let encrypted =
                        match encrypt_message(crypto_provider.as_ref(), &aes_key, &response) {
                            Ok(enc) => enc,
                            Err(e) => {
                                eprintln!(
                                    "[MCP Secure TCP] Encryption error to {}: {}",
                                    peer_addr, e
                                );
                                break;
                            }
                        };

                    if let Err(e) = writeln!(writer, "{}", encrypted) {
                        eprintln!("[MCP Secure TCP] Write error to {}: {}", peer_addr, e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("[MCP Secure TCP] Read error from {}: {}", peer_addr, e);
                break;
            }
        }
    }

    Ok(())
}

/// Perform Kyber key exchange handshake
fn perform_kyber_handshake(
    crypto_provider: &dyn CryptoProvider,
    stream: &TcpStream,
) -> Result<Vec<u8>, String> {
    // Generate server keypair
    let (server_pk, server_sk) = crypto_provider
        .generate_keypair(PqcAlgorithm::Kyber512)
        .map_err(|e| format!("Failed to generate server keypair: {}", e))?;

    // Read client public key (first line)
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| format!("Clone stream: {}", e))?,
    );
    let mut client_pk_line = String::new();
    reader
        .read_line(&mut client_pk_line)
        .map_err(|e| format!("Read client public key: {}", e))?;
    let client_pk_line = client_pk_line.trim();

    // Decode client public key (base64)
    let client_pk = general_purpose::STANDARD
        .decode(client_pk_line)
        .map_err(|e| format!("Decode client public key: {}", e))?;

    // Encapsulate shared secret using client's public key
    let (ciphertext, shared_secret) = crypto_provider
        .encapsulate(&client_pk, PqcAlgorithm::Kyber512)
        .map_err(|e| format!("Encapsulate shared secret: {}", e))?;

    // Send server public key and ciphertext to client (one line, space separated)
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("Clone stream for write: {}", e))?;
    let response = format!(
        "{} {}\n",
        general_purpose::STANDARD.encode(&server_pk),
        general_purpose::STANDARD.encode(&ciphertext)
    );
    writer
        .write_all(response.as_bytes())
        .map_err(|e| format!("Write handshake response: {}", e))?;
    writer
        .flush()
        .map_err(|e| format!("Flush handshake response: {}", e))?;

    // Server also needs to derive shared secret from client's ciphertext? Wait.
    // Actually, in Kyber, the client encapsulates to server's public key.
    // But we are doing opposite: client sends its public key, server encapsulates.
    // That's fine: shared secret is derived from client's public key and server's secret key.
    // We already have shared_secret from encapsulate.
    // Client will decapsulate using its secret key and received ciphertext.
    // Need to ensure client knows to send its public key first.

    Ok(shared_secret)
}

/// Encrypt a message with AES-256-GCM
fn encrypt_message(
    crypto_provider: &dyn CryptoProvider,
    key: &[u8; 32],
    plaintext: &str,
) -> Result<String, String> {
    let ciphertext = crypto_provider
        .encrypt(&key[..], plaintext.as_bytes(), PqcAlgorithm::Aes256Gcm)
        .map_err(|e| format!("Encrypt message: {}", e))?;
    Ok(general_purpose::STANDARD.encode(&ciphertext))
}

/// Decrypt a message with AES-256-GCM
fn decrypt_message(
    crypto_provider: &dyn CryptoProvider,
    key: &[u8; 32],
    encrypted: &str,
) -> Result<String, String> {
    let ciphertext = general_purpose::STANDARD
        .decode(encrypted)
        .map_err(|e| format!("Decode ciphertext: {}", e))?;
    let plaintext = crypto_provider
        .decrypt(&key[..], &ciphertext, PqcAlgorithm::Aes256Gcm)
        .map_err(|e| format!("Decrypt message: {}", e))?;
    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8 plaintext: {}", e))
}

/// Secure TCP client for connecting to secure TCP server
pub struct SecureTcpClient {
    stream: TcpStream,
    aes_key: [u8; 32],
    crypto_provider: Arc<dyn CryptoProvider>,
}

impl SecureTcpClient {
    /// Connect to secure TCP server and perform handshake
    pub fn connect(addr: &str) -> Result<Self, String> {
        let stream = TcpStream::connect(addr).map_err(|e| format!("Connect to server: {}", e))?;
        let crypto_provider: Arc<dyn CryptoProvider> =
            Arc::new(SynapsisPqcProvider::new()) as Arc<dyn CryptoProvider>;

        // Perform Kyber handshake
        let shared_secret = perform_client_handshake(crypto_provider.as_ref(), &stream)?;

        // Derive AES key
        let mut aes_key = [0u8; 32];
        if shared_secret.len() >= 32 {
            aes_key.copy_from_slice(&shared_secret[..32]);
        } else {
            let len = shared_secret.len();
            aes_key[..len].copy_from_slice(&shared_secret);
        }

        Ok(Self {
            stream,
            aes_key,
            crypto_provider,
        })
    }

    /// Send an encrypted message and receive encrypted response
    pub fn send(&mut self, message: &str) -> Result<String, String> {
        // Encrypt message
        let encrypted = encrypt_message(self.crypto_provider.as_ref(), &self.aes_key, message)?;

        // Send with newline
        writeln!(&self.stream, "{}", encrypted).map_err(|e| format!("Write to server: {}", e))?;
        self.stream.flush().map_err(|e| format!("Flush: {}", e))?;

        // Read response
        let mut reader = BufReader::new(&self.stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("Read response: {}", e))?;
        let line = line.trim();

        // Decrypt response
        decrypt_message(self.crypto_provider.as_ref(), &self.aes_key, line)
    }
}

/// Perform client-side Kyber handshake
fn perform_client_handshake(
    crypto_provider: &dyn CryptoProvider,
    stream: &TcpStream,
) -> Result<Vec<u8>, String> {
    // Generate client keypair
    let (client_pk, client_sk) = crypto_provider
        .generate_keypair(PqcAlgorithm::Kyber512)
        .map_err(|e| format!("Failed to generate client keypair: {}", e))?;

    // Send client public key
    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("Clone stream for write: {}", e))?;
    writeln!(writer, "{}", general_purpose::STANDARD.encode(&client_pk))
        .map_err(|e| format!("Send client public key: {}", e))?;
    writer
        .flush()
        .map_err(|e| format!("Flush client public key: {}", e))?;

    // Read server response (public key + ciphertext)
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| format!("Clone stream for read: {}", e))?,
    );
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| format!("Read server response: {}", e))?;
    let response_line = response_line.trim();

    // Parse server public key and ciphertext
    let parts: Vec<&str> = response_line.split_whitespace().collect();
    if parts.len() != 2 {
        return Err("Invalid server response format".to_string());
    }
    let server_pk = general_purpose::STANDARD
        .decode(parts[0])
        .map_err(|e| format!("Decode server public key: {}", e))?;
    let ciphertext = general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|e| format!("Decode ciphertext: {}", e))?;

    // Client decapsulates shared secret using its secret key and ciphertext
    // Actually, in our protocol, server encapsulated to client's public key.
    // Client needs to decapsulate using its secret key and the ciphertext.
    // But we didn't send ciphertext yet? Wait, server sent ciphertext.
    // The ciphertext is from server's encapsulation using client's public key.
    // So client decapsulates using its secret key and ciphertext.
    let shared_secret = crypto_provider
        .decapsulate(&ciphertext, &client_sk, PqcAlgorithm::Kyber512)
        .map_err(|e| format!("Decapsulate shared secret: {}", e))?;

    Ok(shared_secret)
}

/// Start secure TCP server with given MCP server instance
pub fn start_secure_tcp_server(mcp_server: Arc<McpServer>, addr: &str) -> std::io::Result<()> {
    let server = SecureTcpServer::new(mcp_server, addr)?;
    server.run()
}
