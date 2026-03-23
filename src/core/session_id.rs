//! Session Identity Management with HMAC-SHA256 Security
//!
//! Provides unique session identifiers for CLI instances with cryptographic signatures.
//! Multiple CLI instances can run simultaneously with distinct identities.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Unique Session Identifier with HMAC signature
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionId {
    /// CLI type identifier (qwen, opencode, prusia, etc.)
    pub cli_type: String,

    /// Unique instance UUID
    pub instance_uuid: String,

    /// Hostname for multi-machine scenarios
    pub hostname: String,

    /// Process ID for additional uniqueness
    pub pid: u32,

    /// Session start timestamp
    pub started_at: i64,

    /// HMAC signature for integrity verification
    pub signature: String,
}

/// Get secret key from environment or use default (for development only)
fn get_secret_key() -> Vec<u8> {
    env::var("SYNAPSIS_SECRET_KEY")
        .unwrap_or_else(|_| "synapsis-default-secret-key-change-in-production".to_string())
        .as_bytes()
        .to_vec()
}

/// Generate HMAC-SHA256 signature for session data
fn generate_signature(cli_type: &str, uuid: &str, timestamp: i64) -> String {
    let secret = get_secret_key();
    let mut mac = HmacSha256::new_from_slice(&secret)
        .expect("HMAC can take key of any size");
    
    let data = format!("{}|{}|{}", cli_type, uuid, timestamp);
    mac.update(data.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verify HMAC signature
pub fn verify_signature(cli_type: &str, uuid: &str, timestamp: i64, signature: &str) -> bool {
    let expected = generate_signature(cli_type, uuid, timestamp);
    signature == expected
}

impl SessionId {
    /// Create a new unique session ID with HMAC signature
    pub fn new(cli_type: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let uuid = Self::generate_uuid();
        let signature = generate_signature(cli_type, &uuid, now);

        Self {
            cli_type: cli_type.to_string(),
            instance_uuid: uuid,
            hostname: hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            pid: std::process::id(),
            started_at: now,
            signature,
        }
    }

    /// Generate a UUID v4
    fn generate_uuid() -> String {
        let mut buf = [0u8; 16];
        getrandom::getrandom(&mut buf).unwrap();

        // Set version to 4 (random)
        buf[6] = (buf[6] & 0x0f) | 0x40;
        // Set variant to RFC 4122
        buf[8] = (buf[8] & 0x3f) | 0x80;

        hex::encode(buf)
    }

    /// Format as string for storage/display (includes signature)
    pub fn to_string(&self) -> String {
        format!(
            "{}-{}-{}-{}-{}-{}",
            self.cli_type,
            &self.instance_uuid[..8],
            &self.hostname[..8.min(self.hostname.len())],
            self.pid,
            self.started_at,
            &self.signature[..16]
        )
    }

    /// Parse from string (with signature)
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() >= 6 {
            let signature = format!("{}-{}", parts[5], parts.get(6).unwrap_or(&""));
            Some(Self {
                cli_type: parts[0].to_string(),
                instance_uuid: parts[1].to_string(),
                hostname: parts[2].to_string(),
                pid: parts[3].parse().unwrap_or(0),
                started_at: parts[4].parse().unwrap_or(0),
                signature: signature[..32.min(signature.len())].to_string(),
            })
        } else if parts.len() == 5 {
            // Legacy format without signature - generate signature for compatibility
            let cli_type = parts[0].to_string();
            let instance_uuid = parts[1].to_string();
            let started_at: i64 = parts[4].parse().unwrap_or(0);
            let signature = generate_signature(&cli_type, &instance_uuid, started_at);
            
            Some(Self {
                cli_type,
                instance_uuid,
                hostname: parts[2].to_string(),
                pid: parts[3].parse().unwrap_or(0),
                started_at,
                signature,
            })
        } else {
            None
        }
    }

    /// Verify session integrity using HMAC signature
    pub fn verify(&self) -> bool {
        verify_signature(&self.cli_type, &self.instance_uuid, self.started_at, &self.signature)
    }

    /// Check if this session is from the same CLI type
    pub fn same_cli_type(&self, other: &Self) -> bool {
        self.cli_type == other.cli_type
    }

    /// Check if this session is from the same host
    pub fn same_host(&self, other: &Self) -> bool {
        self.hostname == other.hostname
    }

    /// Get session age in seconds
    pub fn age(&self) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        now - self.started_at
    }

    /// Check if session is stale (older than threshold)
    pub fn is_stale(&self, threshold_secs: i64) -> bool {
        self.age() > threshold_secs
    }
}

/// Session Registry - tracks active CLI sessions
#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: std::collections::HashMap<String, SessionId>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
        }
    }

    /// Register a new session with signature verification
    pub fn register(&mut self, session: SessionId) -> Option<&str> {
        // Verify signature before registering
        if !session.verify() {
            eprintln!("[SessionRegistry] Invalid signature for session: {}", session.cli_type);
            return None;
        }
        
        let key = session.to_string();
        self.sessions.insert(key.clone(), session);
        self.sessions.get(&key).map(|s| s.instance_uuid.as_str())
    }

    /// Unregister a session
    pub fn unregister(&mut self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    /// Get all sessions for a CLI type
    pub fn get_by_cli_type(&self, cli_type: &str) -> Vec<&SessionId> {
        self.sessions
            .values()
            .filter(|s| s.cli_type == cli_type)
            .collect()
    }

    /// Get all active sessions
    pub fn get_active(&self, max_age_secs: i64) -> Vec<&SessionId> {
        self.sessions
            .values()
            .filter(|s| !s.is_stale(max_age_secs))
            .collect()
    }

    /// Count sessions by CLI type
    pub fn count_by_cli_type(&self, cli_type: &str) -> usize {
        self.sessions
            .values()
            .filter(|s| s.cli_type == cli_type)
            .count()
    }

    /// Clean stale sessions
    pub fn cleanup_stale(&mut self, max_age_secs: i64) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| !s.is_stale(max_age_secs));
        before - self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_uniqueness() {
        let s1 = SessionId::new("qwen");
        let s2 = SessionId::new("qwen");

        assert_ne!(s1.instance_uuid, s2.instance_uuid);
        assert_ne!(s1.to_string(), s2.to_string());
    }

    #[test]
    fn test_session_id_signature() {
        let s1 = SessionId::new("qwen");
        assert!(s1.verify(), "Signature should be valid");
    }

    #[test]
    fn test_session_id_tampering() {
        let mut s1 = SessionId::new("qwen");
        s1.cli_type = "opencode".to_string(); // Tamper with data
        assert!(!s1.verify(), "Signature should be invalid after tampering");
    }

    #[test]
    fn test_session_id_same_cli_type() {
        let s1 = SessionId::new("qwen");
        let s2 = SessionId::new("qwen");
        let s3 = SessionId::new("opencode");

        assert!(s1.same_cli_type(&s2));
        assert!(!s1.same_cli_type(&s3));
    }

    #[test]
    fn test_session_registry() {
        let mut registry = SessionRegistry::new();

        let s1 = SessionId::new("qwen");
        let s2 = SessionId::new("qwen");
        let s3 = SessionId::new("opencode");

        registry.register(s1.clone());
        registry.register(s2.clone());
        registry.register(s3.clone());

        assert_eq!(registry.count_by_cli_type("qwen"), 2);
        assert_eq!(registry.count_by_cli_type("opencode"), 1);
    }

    #[test]
    fn test_session_registry_invalid_signature() {
        let mut registry = SessionRegistry::new();
        let mut s1 = SessionId::new("qwen");
        s1.cli_type = "tampered".to_string(); // Invalidate signature

        let result = registry.register(s1);
        assert!(result.is_none(), "Should reject session with invalid signature");
    }
}
