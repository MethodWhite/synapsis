//! Security Module - Minimal external dependencies

use getrandom::getrandom;

pub struct SecureRng;

impl SecureRng {
    pub fn new() -> Self {
        Self
    }

    pub fn fill_random(buf: &mut [u8]) {
        if let Err(e) = getrandom(buf) {
            // Fallback: use OS RNG via rand crate if getrandom fails
            // This should be extremely rare (only in specific environments)
            eprintln!(
                "[SECURITY WARNING] getrandom failed: {}, using fallback RNG",
                e
            );
            use rand::RngCore;
            let mut rng = rand::rngs::OsRng;
            rng.fill_bytes(buf);
        }
    }

    pub fn random_u64(&self) -> u64 {
        let mut buf = [0u8; 8];
        Self::fill_random(&mut buf);
        u64::from_le_bytes(buf)
    }

    pub fn random_u32(&self) -> u32 {
        (self.random_u64() & 0xFFFFFFFF) as u32
    }

    pub fn random_u8(&self) -> u8 {
        (self.random_u64() & 0xFF) as u8
    }

    pub fn random_bool(&self) -> bool {
        (self.random_u64() & 1) == 1
    }
}

impl Default for SecureRng {
    fn default() -> Self {
        Self::new()
    }
}

static CSPRNG: SecureRng = SecureRng {};

pub fn secure_random_u64() -> u64 {
    CSPRNG.random_u64()
}

pub fn secure_random_u32() -> u32 {
    CSPRNG.random_u32()
}

pub struct SysCall;

/// Determine the sandbox root for file operations. Defaults to the current
/// working directory, overridable via SYNAPSIS_SANDBOX_DIR. This prevents MCP
/// file tools from reading/writing arbitrary paths outside the workspace.
fn sandbox_root() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("SYNAPSIS_SANDBOX_DIR") {
        let p = std::path::PathBuf::from(dir);
        if p.exists() {
            return p;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

/// Resolve `input` and ensure it stays within the sandbox root.
fn resolve_within_root(input: &str) -> std::io::Result<std::path::PathBuf> {
    use std::path::Component;
    let root = sandbox_root();
    let root_canon = root.canonicalize()?;
    let candidate = std::path::Path::new(input);
    let resolved = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    // Reject obvious traversal before touching the filesystem.
    if resolved.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Path traversal detected",
        ));
    }
    let canonical = resolved.canonicalize().or_else(|_| {
        // Path may not exist yet (e.g. write target): canonicalize the parent.
        if let Some(parent) = resolved.parent() {
            let cp = parent.canonicalize()?;
            if let Some(name) = resolved.file_name() {
                return Ok(cp.join(name));
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot resolve path",
        ))
    })?;
    if canonical.starts_with(&root_canon) {
        Ok(canonical)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Path outside sandbox root",
        ))
    }
}

impl SysCall {
    pub fn timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn write_file(path: &str, data: &[u8]) -> std::io::Result<()> {
        let safe = resolve_within_root(path)?;
        std::fs::write(safe, data)
    }

    pub fn read_file(path: &str) -> std::io::Result<Vec<u8>> {
        let safe = resolve_within_root(path)?;
        std::fs::read(safe)
    }

    pub fn delete_file(path: &str) -> std::io::Result<()> {
        let safe = resolve_within_root(path)?;
        std::fs::remove_file(safe)
    }

    pub fn atomic_rename(old_path: &str, new_path: &str) -> std::io::Result<()> {
        let old_safe = resolve_within_root(old_path)?;
        let new_safe = resolve_within_root(new_path)?;
        std::fs::rename(old_safe, new_safe)
    }

    pub fn list_directory(path: &str) -> std::io::Result<Vec<String>> {
        let safe = resolve_within_root(path)?;
        let mut entries = Vec::new();
        for e in std::fs::read_dir(safe)?.flatten() {
            entries.push(e.file_name().into_string().unwrap_or_default());
        }
        Ok(entries)
    }
}
