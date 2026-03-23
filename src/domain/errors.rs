//! Synapsis Core Error Types

use std::error::Error as StdError;
use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, SynapsisError>;
pub type Error = SynapsisError;

// Convert rusqlite errors to SynapsisError
impl From<rusqlite::Error> for SynapsisError {
    fn from(err: rusqlite::Error) -> Self {
        match err {
            rusqlite::Error::SqliteFailure(e, _) => {
                SynapsisError::new(ErrorKind::Storage, 0x0101, &format!("SQLite error: {}", e))
            }
            _ => SynapsisError::new(ErrorKind::Internal, 0x0A01, &format!("Database error: {}", err))
        }
    }
}

// Convert serde_json errors to SynapsisError
impl From<serde_json::Error> for SynapsisError {
    fn from(err: serde_json::Error) -> Self {
        SynapsisError::new(ErrorKind::Internal, 0x0A04, &format!("JSON error: {}", err))
    }
}

pub fn invalid_data(msg: &str) -> SynapsisError {
    SynapsisError::new(ErrorKind::Validation, 0x0308, msg)
}

pub fn io_error(msg: impl Into<String>) -> SynapsisError {
    SynapsisError::new(ErrorKind::Storage, 0x0100, msg)
}

pub fn serialization(msg: impl Into<String>) -> SynapsisError {
    SynapsisError::new(ErrorKind::Internal, 0x0A04, msg)
}

pub fn deserialization(msg: impl Into<String>) -> SynapsisError {
    SynapsisError::new(ErrorKind::Validation, 0x0309, msg)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorKind {
    Storage = 0x01,
    Crypto = 0x02,
    Validation = 0x03,
    Transport = 0x04,
    Sync = 0x05,
    NotFound = 0x06,
    Security = 0x07,
    Concurrency = 0x08,
    Integrity = 0x09,
    Internal = 0x0A,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Storage => write!(f, "STORAGE"),
            ErrorKind::Crypto => write!(f, "CRYPTO"),
            ErrorKind::Validation => write!(f, "VALIDATION"),
            ErrorKind::Transport => write!(f, "TRANSPORT"),
            ErrorKind::Sync => write!(f, "SYNC"),
            ErrorKind::NotFound => write!(f, "NOT_FOUND"),
            ErrorKind::Security => write!(f, "SECURITY"),
            ErrorKind::Concurrency => write!(f, "CONCURRENCY"),
            ErrorKind::Integrity => write!(f, "INTEGRITY"),
            ErrorKind::Internal => write!(f, "INTERNAL"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SynapsisError {
    kind: ErrorKind,
    code: u16,
    message: String,
    source: Option<Box<SynapsisError>>,
    context: Option<String>,
}

impl SynapsisError {
    pub fn new(kind: ErrorKind, code: u16, message: impl Into<String>) -> Self {
        Self {
            kind,
            code,
            message: message.into(),
            source: None,
            context: None,
        }
    }

    pub fn with_source(mut self, source: SynapsisError) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub const fn code(&self) -> u16 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn error_code(&self) -> String {
        format!("{:02X}-{:04X}", self.kind as u8, self.code)
    }

    pub fn storage_io(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Storage, 0x0101, message)
    }

    pub fn storage_full() -> Self {
        Self::new(ErrorKind::Storage, 0x0102, "Storage capacity exceeded")
    }

    pub fn storage_corrupt(details: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Storage,
            0x0103,
            format!("Database corruption: {}", details.into()),
        )
    }

    pub fn crypto_keygen() -> Self {
        Self::new(ErrorKind::Crypto, 0x0201, "Key generation failed")
    }

    pub fn crypto_cipher() -> Self {
        Self::new(ErrorKind::Crypto, 0x0202, "Cipher operation failed")
    }

    pub fn crypto_verify() -> Self {
        Self::new(ErrorKind::Crypto, 0x0203, "Signature verification failed")
    }

    pub fn crypto_pqc_not_supported() -> Self {
        Self::new(
            ErrorKind::Crypto,
            0x0204,
            "PQC algorithm not supported on this platform",
        )
    }

    pub fn validation_too_large(max: usize, actual: usize) -> Self {
        Self::new(
            ErrorKind::Validation,
            0x0301,
            format!("Content too large: {} > {} bytes", actual, max),
        )
    }

    pub fn validation_empty(field: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Validation,
            0x0302,
            format!("Field '{}' cannot be empty", field.into()),
        )
    }

    pub fn validation_malformed_json() -> Self {
        Self::new(ErrorKind::Validation, 0x0303, "Malformed JSON input")
    }

    pub fn transport_connect() -> Self {
        Self::new(ErrorKind::Transport, 0x0401, "Connection failed")
    }

    pub fn transport_timeout() -> Self {
        Self::new(ErrorKind::Transport, 0x0402, "Operation timed out")
    }

    pub fn transport_closed() -> Self {
        Self::new(
            ErrorKind::Transport,
            0x0403,
            "Connection closed unexpectedly",
        )
    }

    pub fn terminal_error(msg: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Transport,
            0x0404,
            format!("Terminal error: {}", msg.into()),
        )
    }

    pub fn sync_conflict() -> Self {
        Self::new(ErrorKind::Sync, 0x0501, "Sync conflict detected")
    }

    pub fn sync_version_mismatch() -> Self {
        Self::new(ErrorKind::Sync, 0x0502, "Version mismatch during sync")
    }

    pub fn not_found(what: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::NotFound,
            0x0601,
            format!("{} not found", what.into()),
        )
    }

    pub fn not_found_id(id: i64) -> Self {
        Self::new(ErrorKind::NotFound, 0x0602, format!("ID {} not found", id))
    }

    pub fn security_unauthorized() -> Self {
        Self::new(ErrorKind::Security, 0x0701, "Unauthorized access")
    }

    pub fn security_integrity() -> Self {
        Self::new(ErrorKind::Security, 0x0702, "Integrity check failed")
    }

    pub fn security_tampering_detected() -> Self {
        Self::new(
            ErrorKind::Security,
            0x0703,
            "Tampering detected - data may be compromised",
        )
    }

    pub fn concurrency_locked() -> Self {
        Self::new(ErrorKind::Concurrency, 0x0801, "Resource is locked")
    }

    pub fn concurrency_deadlock() -> Self {
        Self::new(ErrorKind::Concurrency, 0x0802, "Deadlock detected")
    }

    pub fn integrity_hash_mismatch() -> Self {
        Self::new(ErrorKind::Integrity, 0x0901, "Hash verification failed")
    }

    pub fn integrity_checksum_failed() -> Self {
        Self::new(ErrorKind::Integrity, 0x0902, "Checksum verification failed")
    }

    pub fn internal_bug(msg: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Internal,
            0x0A01,
            format!("Internal error: {}", msg.into()),
        )
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, 0x0A00, msg)
    }

    pub fn internal_unimplemented() -> Self {
        Self::new(ErrorKind::Internal, 0x0A02, "Feature not implemented")
    }

    pub fn internal_panic(msg: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Internal,
            0x0A03,
            format!("Panic: {}", msg.into()),
        )
    }
}

impl fmt::Display for SynapsisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}:{:04X}] {}", self.kind, self.code, self.message)?;
        if let Some(ctx) = &self.context {
            write!(f, " ({})", ctx)?;
        }
        Ok(())
    }
}

impl StdError for SynapsisError {}

impl From<std::fmt::Error> for SynapsisError {
    fn from(_: std::fmt::Error) -> Self {
        Self::internal_bug("Format error")
    }
}

impl From<std::str::Utf8Error> for SynapsisError {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::new(
            ErrorKind::Validation,
            0x0304,
            format!("Invalid UTF-8: {}", e),
        )
    }
}

impl From<std::string::FromUtf8Error> for SynapsisError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Self::new(
            ErrorKind::Validation,
            0x0306,
            format!("Invalid UTF-8: {}", e),
        )
    }
}

// Removed duplicate From<serde_json::Error>

impl From<io::Error> for SynapsisError {
    fn from(e: io::Error) -> Self {
        Self::new(ErrorKind::Storage, 0x0100, format!("IO error: {}", e))
    }
}
