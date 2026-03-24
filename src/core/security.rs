//! Security Module - Minimal external dependencies

use getrandom::getrandom;
use rand::RngCore;

pub struct SecureRng;

impl SecureRng {
    pub fn new() -> Self {
        Self
    }

    pub fn fill_random(buf: &mut [u8]) {
        if let Err(e) = getrandom(buf) {
            // Fallback: use OS RNG via rand crate if getrandom fails
            // This should be extremely rare (only in specific environments)
            eprintln!("[SECURITY WARNING] getrandom failed: {}, using fallback RNG", e);
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

impl SysCall {
    pub fn timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    pub fn write_file(path: &str, data: &[u8]) -> std::io::Result<()> {
        std::fs::write(path, data)
    }

    pub fn read_file(path: &str) -> std::io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    pub fn delete_file(path: &str) -> std::io::Result<()> {
        std::fs::remove_file(path)
    }

    pub fn atomic_rename(old_path: &str, new_path: &str) -> std::io::Result<()> {
        std::fs::rename(old_path, new_path)
    }

    pub fn list_directory(path: &str) -> std::io::Result<Vec<String>> {
        let mut entries = Vec::new();
        for e in std::fs::read_dir(path)?.flatten() {
            entries.push(e.file_name().into_string().unwrap_or_default());
        }
        Ok(entries)
    }
}

impl rand::RngCore for SecureRng {
    fn next_u32(&mut self) -> u32 {
        self.random_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.random_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        SecureRng::fill_random(dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
