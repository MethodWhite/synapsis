//! CLI Session Management
//! 
//! Each CLI instance gets a unique identity.
//! Multiple instances of the same CLI can run simultaneously.

use crate::core::session_id::{SessionId, SessionRegistry};
use std::sync::{Arc, RwLock};

/// Global session registry
lazy_static::lazy_static! {
    static ref SESSION_REGISTRY: Arc<RwLock<SessionRegistry>> = 
        Arc::new(RwLock::new(SessionRegistry::new()));
}

/// Initialize CLI session
pub fn init_cli_session(cli_type: &str) -> SessionId {
    let session = SessionId::new(cli_type);
    
    // Register in global registry
    {
        let mut registry = SESSION_REGISTRY.write().unwrap();
        registry.register(session.clone());
    }
    
    eprintln!("[CLI] Session initialized: {}", session.to_string());
    eprintln!("[CLI] Type: {}", session.cli_type);
    eprintln!("[CLI] UUID: {}", session.instance_uuid);
    eprintln!("[CLI] Host: {}", session.hostname);
    eprintln!("[CLI] PID: {}", session.pid);
    
    session
}

/// Get active sessions count for a CLI type
pub fn get_active_sessions(cli_type: &str) -> usize {
    let registry = SESSION_REGISTRY.read().unwrap();
    registry.count_by_cli_type(cli_type)
}

/// Get all active sessions
pub fn list_active_sessions(max_age_secs: i64) -> Vec<String> {
    let registry = SESSION_REGISTRY.read().unwrap();
    registry
        .get_active(max_age_secs)
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Cleanup stale sessions
pub fn cleanup_stale_sessions(max_age_secs: i64) -> usize {
    let mut registry = SESSION_REGISTRY.write().unwrap();
    registry.cleanup_stale(max_age_secs)
}

/// Check if another instance of the same CLI is running
pub fn has_sibling_instances(cli_type: &str, current_pid: u32) -> bool {
    let registry = SESSION_REGISTRY.read().unwrap();
    registry
        .get_by_cli_type(cli_type)
        .iter()
        .any(|s| s.pid != current_pid && !s.is_stale(60))
}
