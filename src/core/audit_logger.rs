//! Audit Logging Module for Security Events
//! 
//! Provides comprehensive audit logging for security-critical events

use std::fs::{File, OpenOptions};
use std::io::{Write, BufWriter};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

/// Security event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum SecurityEvent {
    #[serde(rename = "auth_success")]
    AuthenticationSuccess { agent_id: String, session_id: String },
    
    #[serde(rename = "auth_failure")]
    AuthenticationFailure { agent_id: String, reason: String },
    
    #[serde(rename = "session_created")]
    SessionCreated { session_id: String, agent_id: String },
    
    #[serde(rename = "session_terminated")]
    SessionTerminated { session_id: String, reason: String },
    
    #[serde(rename = "rate_limit_exceeded")]
    RateLimitExceeded { ip: String, attempts: u32 },
    
    #[serde(rename = "pqc_handshake")]
    PqcHandshake { status: String, algorithm: String },
    
    #[serde(rename = "encryption_enabled")]
    EncryptionEnabled { cipher: String },
    
    #[serde(rename = "security_violation")]
    SecurityViolation { violation: String, severity: String },
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub event: SecurityEvent,
    pub source_ip: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl AuditEntry {
    pub fn new(event: SecurityEvent, source_ip: Option<String>) -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            event,
            source_ip,
            metadata: None,
        }
    }
    
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Audit logger
pub struct AuditLogger {
    log_file: PathBuf,
    writer: BufWriter<File>,
}

impl AuditLogger {
    pub fn new(log_dir: PathBuf) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(&log_dir)?;
        
        let log_file = log_dir.join(format!(
            "audit_{}.log",
            chrono::Local::now().format("%Y%m%d")
        ));
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;
        
        Ok(Self {
            log_file,
            writer: BufWriter::new(file),
        })
    }
    
    pub fn log(&mut self, entry: AuditEntry) -> Result<(), std::io::Error> {
        let json = serde_json::to_string(&entry)?;
        writeln!(self.writer, "{}", json)?;
        self.writer.flush()?;
        Ok(())
    }
    
    pub fn log_auth_success(&mut self, agent_id: &str, session_id: &str) -> Result<(), std::io::Error> {
        let entry = AuditEntry::new(
            SecurityEvent::AuthenticationSuccess {
                agent_id: agent_id.to_string(),
                session_id: session_id.to_string(),
            },
            None,
        );
        self.log(entry)
    }
    
    pub fn log_auth_failure(&mut self, agent_id: &str, reason: &str) -> Result<(), std::io::Error> {
        let entry = AuditEntry::new(
            SecurityEvent::AuthenticationFailure {
                agent_id: agent_id.to_string(),
                reason: reason.to_string(),
            },
            None,
        );
        self.log(entry)
    }
    
    pub fn log_rate_limit(&mut self, ip: &str, attempts: u32) -> Result<(), std::io::Error> {
        let entry = AuditEntry::new(
            SecurityEvent::RateLimitExceeded {
                ip: ip.to_string(),
                attempts,
            },
            None,
        );
        self.log(entry)
    }
    
    pub fn log_pqc_handshake(&mut self, status: &str, algorithm: &str) -> Result<(), std::io::Error> {
        let entry = AuditEntry::new(
            SecurityEvent::PqcHandshake {
                status: status.to_string(),
                algorithm: algorithm.to_string(),
            },
            None,
        );
        self.log(entry)
    }
    
    pub fn log_security_violation(&mut self, violation: &str, severity: &str) -> Result<(), std::io::Error> {
        let entry = AuditEntry::new(
            SecurityEvent::SecurityViolation {
                violation: violation.to_string(),
                severity: severity.to_string(),
            },
            None,
        );
        self.log(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;
    use tempfile::tempdir;

    #[test]
    fn test_audit_logger_creation() {
        let temp_dir = tempdir().unwrap();
        let logger = AuditLogger::new(temp_dir.path().to_path_buf());
        assert!(logger.is_ok(), "AuditLogger should be created");
    }

    #[test]
    fn test_log_auth_success() {
        let temp_dir = tempdir().unwrap();
        let mut logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();
        
        let result = logger.log_auth_success("test-agent", "test-session");
        assert!(result.is_ok(), "Should log auth success");
    }

    #[test]
    fn test_log_auth_failure() {
        let temp_dir = tempdir().unwrap();
        let mut logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();
        
        let result = logger.log_auth_failure("test-agent", "invalid_credentials");
        assert!(result.is_ok(), "Should log auth failure");
    }

    #[test]
    fn test_log_rate_limit() {
        let temp_dir = tempdir().unwrap();
        let mut logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();
        
        let result = logger.log_rate_limit("192.168.1.1", 100);
        assert!(result.is_ok(), "Should log rate limit");
    }

    #[test]
    fn test_log_pqc_handshake() {
        let temp_dir = tempdir().unwrap();
        let mut logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();
        
        let result = logger.log_pqc_handshake("success", "Kyber512");
        assert!(result.is_ok(), "Should log PQC handshake");
    }

    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry::new(
            SecurityEvent::AuthenticationSuccess {
                agent_id: "test".to_string(),
                session_id: "session".to_string(),
            },
            Some("192.168.1.1".into()),
        );
        
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("auth_success"), "Should contain event type");
        assert!(json.contains("test"), "Should contain agent_id");
    }

    #[test]
    fn test_multiple_events() {
        let temp_dir = tempdir().unwrap();
        let mut logger = AuditLogger::new(temp_dir.path().to_path_buf()).unwrap();
        
        // Log multiple events
        logger.log_auth_success("agent1", "session1").unwrap();
        logger.log_auth_failure("agent2", "reason").unwrap();
        logger.log_rate_limit("1.2.3.4", 50).unwrap();
        
        // Verify file exists and has content
        let log_file = temp_dir.path().join("audit_test.log");
        let content = std::fs::read_to_string(&log_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        
        assert_eq!(lines.len(), 3, "Should have 3 log entries");
    }
}
