//! Synapsis TCP Server Binary
//!
//! Persistent TCP server for multi-agent coordination.
//! Maintains state across connections for long-running agent sessions.
//!
//! SECURITY: Implements challenge-response authentication with HMAC-SHA256

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use rand::RngCore;

use synapsis::core::uuid::Uuid;
use synapsis::infrastructure::database::Database;

type HmacSha256 = Hmac<Sha256>;

/// Server state shared across all connections
struct ServerState {
    db: Database,
    sessions: Mutex<HashMap<String, SessionInfo>>,
    /// Pending challenges for challenge-response auth (session_id -> (challenge, expires_at))
    challenges: Mutex<HashMap<String, (String, u64)>>,
    /// API keys for authentication (in production, load from secure config)
    api_keys: Vec<String>,
}

#[derive(Clone)]
struct SessionInfo {
    agent_type: String,
    project: String,
    connected_at: i64,
    authenticated: bool,
    api_key_hash: Option<String>,
}

/// Generate a random challenge (32 bytes, hex encoded)
fn generate_challenge() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Compute HMAC-SHA256 signature
fn compute_hmac(key: &str, message: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verify HMAC signature
fn verify_hmac(key: &str, message: &str, signature: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    mac.verify_slice(&hex::decode(signature).unwrap_or_default()).is_ok()
}

/// Check if API key is valid
fn is_valid_api_key(api_key: &str, valid_keys: &[String]) -> bool {
    valid_keys.contains(&api_key.to_string())
}

fn handle_client(
    stream: TcpStream,
    state: Arc<ServerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;

    // Track if this connection is authenticated
    let mut authenticated_session: Option<String> = None;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(req) => {
                let id = req.get("id");
                let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let params = req.get("params");
                
                // Check authentication for protected methods
                let requires_auth = matches!(method, 
                    "lock_acquire" | "lock_release" | "task_create" | "task_claim" | 
                    "context_export" | "context_import" | "session_heartbeat");
                
                if requires_auth && authenticated_session.is_none() {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "error": {"code": -32000, "message": "Authentication required. Call auth_challenge first."},
                        "id": id.cloned().unwrap_or(serde_json::Value::Null)
                    })
                } else {
                    let result = match method {
                        "ping" => serde_json::json!({"status": "ok"}),
                        
                        "auth_challenge" => {
                            // Generate challenge for challenge-response auth
                            let args = params.and_then(|p| p.get("arguments"));
                            let _api_key_id = args
                                .and_then(|a| a.get("api_key_id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("default");
                            
                            let challenge = generate_challenge();
                            let expires_at = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs() + 300; // 5 minute TTL
                            
                            // Store challenge with session identifier
                            let session_id = format!("pending-{}", Uuid::new_v4().to_hex_string());
                            state.challenges.lock().unwrap()
                                .insert(session_id.clone(), (challenge.clone(), expires_at));
                            
                            serde_json::json!({
                                "challenge": challenge,
                                "session_id": session_id,
                                "expires_in": 300
                            })
                        }
                        
                        "auth_verify" => {
                            // Verify challenge-response
                            let args = params.and_then(|p| p.get("arguments"));
                            let session_id = args
                                .and_then(|a| a.get("session_id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let response_sig = args
                                .and_then(|a| a.get("response"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let api_key = args
                                .and_then(|a| a.get("api_key"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            
                            // Verify API key
                            if !is_valid_api_key(api_key, &state.api_keys) {
                                serde_json::json!({"authenticated": false, "error": "Invalid API key"})
                            } else {
                                // Verify challenge response
                                let challenges = state.challenges.lock().unwrap();
                                if let Some((challenge, expires_at)) = challenges.get(session_id) {
                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs();
                                    
                                    if now > *expires_at {
                                        serde_json::json!({"authenticated": false, "error": "Challenge expired"})
                                    } else {
                                        // Verify: response = HMAC(api_key, challenge)
                                        let _expected = compute_hmac(api_key, challenge);
                                        if verify_hmac(api_key, challenge, response_sig) {
                                            // Mark session as authenticated
                                            drop(challenges);
                                            state.challenges.lock().unwrap().remove(session_id);
                                            
                                            // Create authenticated session
                                            let agent_type = args
                                                .and_then(|a| a.get("agent_type"))
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("authenticated-client");
                                            let project = args
                                                .and_then(|a| a.get("project"))
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("default");
                                            
                                            let full_session_id = format!(
                                                "{}-{}-{}",
                                                agent_type,
                                                Uuid::new_v4().to_hex_string(),
                                                now
                                            );
                                            
                                            let _ = state.db.register_agent_session(
                                                agent_type, &full_session_id, project, None
                                            );
                                            
                                            state.sessions.lock().unwrap().insert(
                                                full_session_id.clone(),
                                                SessionInfo {
                                                    agent_type: agent_type.to_string(),
                                                    project: project.to_string(),
                                                    connected_at: now as i64,
                                                    authenticated: true,
                                                    api_key_hash: Some(hex::encode(
                                                        &api_key.as_bytes()[..8]
                                                    )),
                                                },
                                            );
                                            
                                            authenticated_session = Some(full_session_id.clone());
                                            
                                            serde_json::json!({
                                                "authenticated": true,
                                                "session_id": full_session_id
                                            })
                                        } else {
                                            serde_json::json!({"authenticated": false, "error": "Invalid signature"})
                                        }
                                    }
                                } else {
                                    serde_json::json!({"authenticated": false, "error": "Invalid session"})
                                }
                            }
                        }
                        
                        "auth_quick" => {
                            // Quick auth with API key (simpler, less secure)
                            let args = params.and_then(|p| p.get("arguments"));
                            let api_key = args
                                .and_then(|a| a.get("api_key"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let agent_type = args
                                .and_then(|a| a.get("agent_type"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("quick-auth-client");
                            let project = args
                                .and_then(|a| a.get("project"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("default");
                            
                            if !is_valid_api_key(api_key, &state.api_keys) {
                                serde_json::json!({"authenticated": false, "error": "Invalid API key"})
                            } else {
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs();
                                
                                let session_id = format!(
                                    "{}-{}-{}",
                                    agent_type,
                                    Uuid::new_v4().to_hex_string(),
                                    now
                                );
                                
                                let _ = state.db.register_agent_session(
                                    agent_type, &session_id, project, None
                                );
                                
                                state.sessions.lock().unwrap().insert(
                                    session_id.clone(),
                                    SessionInfo {
                                        agent_type: agent_type.to_string(),
                                        project: project.to_string(),
                                        connected_at: now as i64,
                                        authenticated: true,
                                        api_key_hash: Some(hex::encode(&api_key.as_bytes()[..8])),
                                    },
                                );
                                
                                authenticated_session = Some(session_id.clone());
                                
                                serde_json::json!({
                                    "authenticated": true,
                                    "session_id": session_id
                                })
                            }
                        }
                        
                        "session_register" => {
                            // Allow session registration without auth (read-only operations)
                            let args = params.and_then(|p| p.get("arguments"));
                            let agent_type = args
                                .and_then(|a| a.get("agent_type"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let project = args
                                .and_then(|a| a.get("project"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("default");

                            let session_id = format!(
                                "{}-{}-{}",
                                agent_type,
                                Uuid::new_v4().to_hex_string(),
                                SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs()
                            );

                            if let Err(e) = state.db.register_agent_session(agent_type, &session_id, project, None) {
                                serde_json::json!({"error": format!("{:?}", e)})
                            } else {
                                let now = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs() as i64;

                                state.sessions.lock().unwrap().insert(
                                    session_id.clone(),
                                    SessionInfo {
                                        agent_type: agent_type.to_string(),
                                        project: project.to_string(),
                                        connected_at: now,
                                        authenticated: false,
                                        api_key_hash: None,
                                    },
                                );

                                // Don't auto-authenticate, but return session_id for future auth
                                serde_json::json!({
                                    "session_id": session_id,
                                    "authenticated": false,
                                    "notice": "Call auth_verify or auth_quick to enable write operations"
                                })
                            }
                        }
                        "session_heartbeat" => {
                            let args = params.and_then(|p| p.get("arguments"));
                            let session_id = args
                                .and_then(|a| a.get("session_id"))
                                .and_then(|v| v.as_str());
                            let task = args.and_then(|a| a.get("task")).and_then(|v| v.as_str());

                            match session_id {
                                Some(sid) => {
                                    if let Err(e) = state.db.agent_heartbeat(sid, task) {
                                        serde_json::json!({"error": format!("{:?}", e)})
                                    } else {
                                        serde_json::json!({"status": "ok"})
                                    }
                                }
                                None => serde_json::json!({"error": "session_id required"}),
                            }
                        }
                    "agents_active" => {
                        let project = params
                            .and_then(|p| p.get("project"))
                            .and_then(|v| v.as_str());

                        match state.db.get_active_agents(project) {
                            Ok(agents) => serde_json::json!({"agents": agents}),
                            Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                        }
                    }
                    "lock_acquire" => {
                        // SECURITY: Verify session ownership
                        let args = params.and_then(|p| p.get("arguments"));
                        let session_id = args
                            .and_then(|a| a.get("session_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        
                        // Verify the session_id matches the authenticated session
                        if let Some(auth_session) = &authenticated_session {
                            if auth_session != session_id {
                                serde_json::json!({
                                    "error": "Session mismatch: Cannot acquire lock for different session"
                                })
                            } else {
                                let lock_key = args
                                    .and_then(|a| a.get("lock_key"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let ttl = args
                                    .and_then(|a| a.get("ttl"))
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(30);

                                match state
                                    .db
                                    .acquire_lock(session_id, lock_key, "resource", None, ttl)
                                {
                                    Ok(acquired) => serde_json::json!({"acquired": acquired}),
                                    Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                                }
                            }
                        } else {
                            serde_json::json!({"error": "Not authenticated"})
                        }
                    }
                    "lock_release" => {
                        // SECURITY: Verify session ownership before releasing
                        let args = params.and_then(|p| p.get("arguments"));
                        let lock_key = args
                            .and_then(|a| a.get("lock_key"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let session_id = args
                            .and_then(|a| a.get("session_id"))
                            .and_then(|v| v.as_str());

                        // If session_id provided, verify ownership
                        if let Some(sid) = session_id {
                            if let Some(auth_session) = &authenticated_session {
                                if auth_session != sid {
                                    serde_json::json!({
                                        "error": "Session mismatch: Cannot release lock owned by different session"
                                    })
                                } else {
                                    match state.db.release_lock(lock_key) {
                                        Ok(_) => serde_json::json!({"released": true}),
                                        Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                                    }
                                }
                            } else {
                                serde_json::json!({"error": "Not authenticated"})
                            }
                        } else {
                            match state.db.release_lock(lock_key) {
                                Ok(_) => serde_json::json!({"released": true}),
                                Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                            }
                        }
                    }
                    "task_create" => {
                        let args = params.and_then(|p| p.get("arguments"));
                        let project = args
                            .and_then(|a| a.get("project"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("default");
                        let task_type = args
                            .and_then(|a| a.get("task_type"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("general");
                        let payload = args
                            .and_then(|a| a.get("payload"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let priority = args
                            .and_then(|a| a.get("priority"))
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0) as i32;

                        match state.db.create_task(project, task_type, payload, priority) {
                            Ok(task_id) => serde_json::json!({"task_id": task_id}),
                            Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                        }
                    }
                    "task_claim" => {
                        let args = params.and_then(|p| p.get("arguments"));
                        let session_id = args
                            .and_then(|a| a.get("session_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("default");

                        match state.db.claim_task(session_id, None) {
                            Ok(task) => serde_json::json!({"task": task}),
                            Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                        }
                    }
                    "context_export" => {
                        let args = params.and_then(|p| p.get("arguments"));
                        let project = args
                            .and_then(|a| a.get("project"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("default");

                        match state.db.export_context(project) {
                            Ok(export) => serde_json::json!({"context": export}),
                            Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                        }
                    }
                        "context_import" => {
                            let args = params.and_then(|p| p.get("arguments"));
                            let project = args
                                .and_then(|a| a.get("project"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("default");
                            let context = args
                                .and_then(|a| a.get("context"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            match state.db.import_context(project, context) {
                                Ok(_) => serde_json::json!({"imported": true}),
                                Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                            }
                        }
                    "stats" => match state.db.get_stats() {
                        Ok(stats) => serde_json::json!({"stats": stats}),
                        Err(e) => serde_json::json!({"error": format!("{:?}", e)}),
                    },
                    _ => serde_json::json!({"error": format!("Unknown method: {}", method)}),
                };

                let mut resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "result": result,
                });
                if let Some(id) = id {
                    resp["id"] = id.clone();
                }
                resp
            }
        }
            Err(e) => serde_json::json!({
                "error": format!("Parse error: {:?}", e),
                "id": null,
            }),
        };

        let response_str = serde_json::to_string(&response)?;
        writeln!(writer, "{}", response_str)?;
        writer.flush()?;
    }

    Ok(())
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!(
        "║  Synapsis v{} - TCP Server                          ║",
        env!("CARGO_PKG_VERSION")
    );
    println!("║  Persistent Memory Server for Multi-Agent Systems      ║");
    println!("║  SECURITY: Challenge-Response Auth Enabled             ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let addr = "127.0.0.1:7439";
    println!("Listening on {}", addr);
    println!("(Alternative TCP server - main server uses port 7438)");
    println!("Press Ctrl+C to stop");
    println!();

    // Initialize API keys from environment or generate default
    let api_keys = std::env::var("SYNAPSIS_API_KEYS")
        .unwrap_or_else(|_| "synapsis-default-key-change-in-production".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();

    println!("[Security] API keys loaded: {}", api_keys.len());
    println!("[Security] Auth methods: auth_challenge, auth_quick");
    println!();

    let db = Database::new();
    let state = Arc::new(ServerState {
        db,
        sessions: Mutex::new(std::collections::HashMap::new()),
        challenges: Mutex::new(std::collections::HashMap::new()),
        api_keys,
    });

    let listener = TcpListener::bind(addr).expect("Failed to bind");
    println!("[Server] Ready for connections");
    println!();
    println!("Protected methods: lock_acquire, lock_release, task_create, task_claim,");
    println!("                   context_export, context_import, session_heartbeat");
    println!();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    let peer = stream.peer_addr().unwrap();
                    println!("[Server] Client connected: {}", peer);

                    if let Err(e) = handle_client(stream, state) {
                        eprintln!("[Server] Client error: {}", e);
                    }

                    println!("[Server] Client disconnected: {}", peer);
                });
            }
            Err(e) => {
                eprintln!("[Server] Connection error: {}", e);
            }
        }
    }
}
