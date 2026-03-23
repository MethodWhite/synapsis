//! Security Module - Minimal external dependencies

pub struct SecureRng;

impl SecureRng {
    pub fn new() -> Self {
        Self
    }

    pub fn fill_random(buf: &mut [u8]) {
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut state = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let pid = std::process::id() as u64;

        for byte in buf.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            state = state.rotate_left(1);
            state ^= pid.wrapping_mul(31337);
            state ^= 17u64.wrapping_mul(state >> 32);
            *byte = (state >> 32) as u8;
        }

        for i in (8..buf.len()).step_by(8) {
            let mut seed = state;
            let end = buf.len().min(i + 8);
            for byte in &mut buf[i..end] {
                seed = seed.wrapping_mul(2805825398015834229);
                seed = seed.rotate_right(3);
                *byte = (seed >> 24) as u8;
            }
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
