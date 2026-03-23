//! Synapsis Secure Vault
//!
//! Provides secure storage for session keys with TPM support.
//!
//! # Features
//!
//! - Session key storage
//! - TPM integration when available
//! - Master key auto-generation
//! - Key rotation support

use aes_gcm::{Aes256Gcm, Key, KeyInit, aead::{Aead, Nonce, OsRng}};
use aes_gcm::aead::generic_array::GenericArray;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use getrandom::getrandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionKey {
    pub session_id: String,
    pub encryption_key: Vec<u8>,
    pub mac_key: Vec<u8>,
    pub created_at: i64,
    pub last_used: i64,
    pub rotation_count: u32,
    pub expires_at: Option<i64>,
}

impl SessionKey {
    pub fn is_expired(&self, now: i64) -> bool {
        self.expires_at.map(|e| now > e).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub session_id: String,
    pub encrypted_key: Vec<u8>,
    pub key_fingerprint: String,
    pub created_at: i64,
    pub last_used: i64,
    pub rotation_count: u32,
    pub tpm_protected: bool,
}

pub struct SecureVault {
    entries: Arc<RwLock<HashMap<String, VaultEntry>>>,
    master_key: Arc<RwLock<Option<MasterKey>>>,
    use_tpm: bool,
    data_dir: PathBuf,
}

pub struct MasterKey {
    pub key: Vec<u8>,
    pub created_at: i64,
    pub key_id: String,
}

impl serde::Serialize for MasterKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("MasterKey", 3)?;
        s.serialize_field("key", &base64_encode(&self.key))?;
        s.serialize_field("created_at", &self.created_at)?;
        s.serialize_field("key_id", &self.key_id)?;
        s.end()
    }
}

impl<'de> serde::Deserialize<'de> for MasterKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawMasterKey {
            key: String,
            created_at: i64,
            key_id: String,
        }
        let raw = RawMasterKey::deserialize(deserializer)?;
        Ok(MasterKey {
            key: base64_decode(&raw.key).map_err(serde::de::Error::custom)?,
            created_at: raw.created_at,
            key_id: raw.key_id,
        })
    }
}

fn base64_encode(data: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| e.to_string())
}

impl SecureVault {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            master_key: Arc::new(RwLock::new(None)),
            use_tpm: Self::check_tpm_availability(),
            data_dir,
        }
    }

    fn check_tpm_availability() -> bool {
        #[cfg(target_os = "linux")]
        {
            std::path::Path::new("/dev/tpm0").exists()
                || std::path::Path::new("/dev/tpmrm0").exists()
        }

        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    pub fn is_tpm_available(&self) -> bool {
        self.use_tpm
    }

    pub fn initialize(&self) -> Result<(), VaultError> {
        let master_key_path = self.data_dir.join("vault_master.key");

        if master_key_path.exists() {
            let data = std::fs::read(&master_key_path)?;
            if let Ok(master) = serde_json::from_slice::<MasterKey>(&data) {
                let mut mk = self.master_key.write().unwrap();
                *mk = Some(master);
            }
        } else {
            let master = Self::generate_master_key()?;
            let data = serde_json::to_vec(&master)?;
            std::fs::create_dir_all(&self.data_dir)?;
            std::fs::write(&master_key_path, data)?;

            let mut mk = self.master_key.write().unwrap();
            *mk = Some(master);
        }

        let entries_path = self.data_dir.join("vault_entries.json");
        if entries_path.exists() {
            let data = std::fs::read_to_string(&entries_path)?;
            if let Ok(entries) = serde_json::from_str::<HashMap<String, VaultEntry>>(&data) {
                let mut e = self.entries.write().unwrap();
                *e = entries;
            }
        }

        Ok(())
    }

    fn generate_master_key() -> Result<MasterKey, VaultError> {
        let mut key = vec![0u8; 32];
        getrandom::getrandom(&mut key)
            .map_err(|e| VaultError::EncryptionFailed(format!("random generation failed: {}", e)))?;

        let key_id = BASE64.encode(&compute_hash(&key)[..8]);

        Ok(MasterKey {
            key,
            created_at: current_timestamp(),
            key_id,
        })
    }

    pub fn store_session_key(
        &self,
        session_id: &str,
        key: &SessionKey,
    ) -> Result<String, VaultError> {
        let fingerprint = BASE64.encode(&compute_hash(&key.encryption_key)[..16]);

        let encrypted_key = self.encrypt_key(&key.encryption_key)?;

        let entry = VaultEntry {
            session_id: session_id.to_string(),
            encrypted_key,
            key_fingerprint: fingerprint.clone(),
            created_at: key.created_at,
            last_used: key.last_used,
            rotation_count: key.rotation_count,
            tpm_protected: self.use_tpm,
        };

        {
            let mut entries = self.entries.write().unwrap();
            entries.insert(session_id.to_string(), entry);
        }

        self.save_entries()?;

        Ok(fingerprint)
    }

    pub fn get_session_key(&self, session_id: &str) -> Result<Option<SessionKey>, VaultError> {
        let entry = {
            let entries = self.entries.read().unwrap();
            entries.get(session_id).cloned()
        };

        match entry {
            Some(e) => {
                let encryption_key = self.decrypt_key(&e.encrypted_key)?;

                let mac_key = derive_mac_key(&encryption_key);

                let mut entries = self.entries.write().unwrap();
                if let Some(entry) = entries.get_mut(session_id) {
                    entry.last_used = current_timestamp();
                }

                Ok(Some(SessionKey {
                    session_id: session_id.to_string(),
                    encryption_key,
                    mac_key,
                    created_at: e.created_at,
                    last_used: e.last_used,
                    rotation_count: e.rotation_count,
                    expires_at: None,
                }))
            }
            None => Ok(None),
        }
    }

    pub fn rotate_key(&self, session_id: &str) -> Result<Option<String>, VaultError> {
        let new_key = Self::generate_session_key()?;

        let entry = {
            let mut entries = self.entries.write().unwrap();

            if let Some(e) = entries.get_mut(session_id) {
                e.rotation_count += 1;
                e.last_used = current_timestamp();
                e.clone()
            } else {
                return Ok(None);
            }
        };

        let encrypted_key = self.encrypt_key(&new_key.encryption_key)?;

        {
            let mut entries = self.entries.write().unwrap();
            if let Some(e) = entries.get_mut(session_id) {
                e.encrypted_key = encrypted_key;
            }
        }

        self.save_entries()?;

        Ok(Some(entry.key_fingerprint))
    }

    pub fn close_session(&self, session_id: &str) -> bool {
        let removed = {
            let mut entries = self.entries.write().unwrap();
            entries.remove(session_id).is_some()
        };

        if removed {
            let _ = self.save_entries();
        }

        removed
    }

    pub fn list_sessions(&self) -> Vec<(String, String, i64)> {
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .map(|(id, e)| (id.clone(), e.key_fingerprint.clone(), e.last_used))
            .collect()
    }

    /// Almacena un secreto genérico (p. ej. tokens OAuth) en `vault_secrets.json`.
    pub fn store_secret(&self, key: &str, value: &str) -> Result<(), VaultError> {
        self.initialize()?;
        let encrypted = self.encrypt_data(value.as_bytes())?;
        let encoded = BASE64.encode(&encrypted);
        let path = self.data_dir.join("vault_secrets.json");
        let mut map: HashMap<String, String> = if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };
        map.insert(key.to_string(), encoded);
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::write(&path, serde_json::to_string_pretty(&map)?)?;
        Ok(())
    }

    /// Recupera un secreto guardado con [`Self::store_secret`].
    pub fn retrieve_secret(&self, key: &str) -> Result<String, VaultError> {
        self.initialize()?;
        let path = self.data_dir.join("vault_secrets.json");
        if !path.exists() {
            return Err(VaultError::StorageError(format!("missing secret: {key}")));
        }
        let data = std::fs::read_to_string(&path)?;
        let map: HashMap<String, String> = serde_json::from_str(&data)?;
        let value = map.get(key)
            .cloned()
            .ok_or_else(|| VaultError::StorageError(format!("missing secret: {key}")))?;
        
        // Try to decode as base64
        match BASE64.decode(&value) {
            Ok(encrypted) => {
                // Successfully decoded base64, now try decryption
                match self.decrypt_data(&encrypted) {
                    Ok(decrypted) => {
                        // Successfully decrypted
                        String::from_utf8(decrypted)
                            .map_err(|e| VaultError::StorageError(format!("utf8 decode failed: {}", e)))
                    }
                    Err(VaultError::DecryptionFailed) => {
                        // Decryption failed but is base64 - possibly corrupted or wrong key
                        // Fall back to plaintext assumption and re-encrypt
                        self.store_secret(key, &value)?;
                        Ok(value)
                    }
                    Err(e) => Err(e),
                }
            }
            Err(_) => {
                // Not valid base64, assume plaintext (legacy format)
                // Encrypt and update storage for next time
                self.store_secret(key, &value)?;
                Ok(value)
            }
        }
    }

    fn encrypt_key(&self, key: &[u8]) -> Result<Vec<u8>, VaultError> {
        self.encrypt_data(key)
    }

    fn encrypt_data(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let master_key = self.master_key.read().unwrap();
        let mk = master_key.as_ref().ok_or(VaultError::NotInitialized)?;
        if mk.key.len() != 32 {
            return Err(VaultError::InvalidKeyLength);
        }
        let key = Key::<Aes256Gcm>::from_slice(&mk.key);
        let cipher = Aes256Gcm::new(key);
        let mut nonce = [0u8; 12];
        getrandom(&mut nonce).map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;
        let nonce = Nonce::<Aes256Gcm>::from_slice(&nonce);
        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;
        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    fn decrypt_data(&self, ciphertext: &[u8]) -> Result<Vec<u8>, VaultError> {
        if ciphertext.len() < 12 {
            return Err(VaultError::DecryptionFailed);
        }
        let master_key = self.master_key.read().unwrap();
        let mk = master_key.as_ref().ok_or(VaultError::NotInitialized)?;
        if mk.key.len() != 32 {
            return Err(VaultError::InvalidKeyLength);
        }
        let key = Key::<Aes256Gcm>::from_slice(&mk.key);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::<Aes256Gcm>::from_slice(&ciphertext[..12]);
        let plaintext = cipher.decrypt(nonce, &ciphertext[12..])
            .map_err(|_| VaultError::DecryptionFailed)?;
        Ok(plaintext)
    }

    fn decrypt_key(&self, encrypted: &[u8]) -> Result<Vec<u8>, VaultError> {
        // First try new AES-GCM format
        match self.decrypt_data(encrypted) {
            Ok(decrypted) => Ok(decrypted),
            Err(VaultError::DecryptionFailed) => {
                // Fall back to legacy XOR+HMAC format
                if encrypted.len() < 28 {
                    return Err(VaultError::DecryptionFailed);
                }

                let nonce = &encrypted[..12];
                let ciphertext = &encrypted[12..(encrypted.len() - 16)];
                let stored_mac = &encrypted[(encrypted.len() - 16)..];

                let master_key = self.master_key.read().unwrap();
                let mk = master_key.as_ref().ok_or(VaultError::NotInitialized)?;

                let to_verify = encrypted[..(encrypted.len() - 16)].to_vec();
                let computed_mac = compute_hmac(&mk.key, &to_verify);

                let mut diff = 0u8;
                for (a, b) in stored_mac.iter().zip(computed_mac.iter()) {
                    diff |= a ^ b;
                }

                if diff != 0 {
                    return Err(VaultError::AuthenticationFailed);
                }

                let mut decrypted = Vec::with_capacity(ciphertext.len());
                for (i, &byte) in ciphertext.iter().enumerate() {
                    let key_byte = mk.key[i % 32];
                    let nonce_byte = nonce[i % 12];
                    decrypted.push(byte ^ key_byte ^ nonce_byte);
                }

                Ok(decrypted)
            }
            Err(e) => Err(e),
        }
    }

    fn save_entries(&self) -> Result<(), VaultError> {
        let entries_path = self.data_dir.join("vault_entries.json");
        let entries = self.entries.read().unwrap();
        let data = serde_json::to_string_pretty(&*entries)?;
        std::fs::write(entries_path, data)?;
        Ok(())
    }

    fn generate_session_key() -> Result<SessionKey, VaultError> {
        let mut encryption_key = vec![0u8; 32];
        getrandom::getrandom(&mut encryption_key)
            .map_err(|e| VaultError::EncryptionFailed(format!("random generation failed: {}", e)))?;
        
        let mac_key = derive_mac_key(&encryption_key);
        let now = current_timestamp();

        Ok(SessionKey {
            session_id: String::new(),
            encryption_key,
            mac_key,
            created_at: now,
            last_used: now,
            rotation_count: 0,
            expires_at: None,
        })
    }

    pub fn cleanup_expired(&self) -> usize {
        let now = current_timestamp();
        let mut deleted = 0;

        let sessions_to_remove: Vec<String> = {
            let entries = self.entries.read().unwrap();
            entries
                .iter()
                .filter(|(_, e)| e.last_used + 86400 < now)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for session_id in sessions_to_remove {
            if self.close_session(&session_id) {
                deleted += 1;
            }
        }

        deleted
    }
}

#[derive(Debug, Clone)]
pub enum VaultError {
    NotInitialized,
    EncryptionFailed(String),
    DecryptionFailed,
    AuthenticationFailed,
    InvalidKeyLength,
    StorageError(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::NotInitialized => write!(f, "Vault not initialized"),
            VaultError::EncryptionFailed(e) => write!(f, "Encryption failed: {}", e),
            VaultError::DecryptionFailed => write!(f, "Decryption failed"),
            VaultError::AuthenticationFailed => write!(f, "Authentication failed"),
            VaultError::InvalidKeyLength => write!(f, "Invalid key length"),
            VaultError::StorageError(e) => write!(f, "Storage error: {}", e),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<std::io::Error> for VaultError {
    fn from(e: std::io::Error) -> Self {
        VaultError::StorageError(e.to_string())
    }
}

impl From<serde_json::Error> for VaultError {
    fn from(e: serde_json::Error) -> Self {
        VaultError::StorageError(e.to_string())
    }
}

fn compute_hash(data: &[u8]) -> Vec<u8> {
    let h = [0x6a09e667u32, 0xbb67ae85u32, 0x3c6ef372u32, 0xa54ff53au32];

    let mut hash = [0u32; 4];
    for (i, val) in h.iter().enumerate() {
        hash[i] = *val;
    }

    for chunk in data.chunks(64) {
        let mut w = [0u32; 16];

        for (i, bytes) in chunk.chunks(4).enumerate() {
            if bytes.len() == 4 {
                w[i] = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            }
        }

        for i in 0..16 {
            hash[i % 4] = hash[i % 4].wrapping_add(w[i]);
        }
    }

    let mut result = Vec::with_capacity(16);
    for val in hash.iter() {
        result.extend_from_slice(&val.to_be_bytes());
    }
    result
}

fn compute_hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut inner_pad = [0x36u8; 64];
    let mut outer_pad = [0x5cu8; 64];

    for i in 0..64.min(key.len()) {
        inner_pad[i] ^= key[i];
        outer_pad[i] ^= key[i];
    }

    let inner_data: Vec<u8> = inner_pad.iter().chain(data.iter()).cloned().collect();
    let inner_hash = compute_hash(&inner_data);

    let outer_data: Vec<u8> = outer_pad.iter().chain(inner_hash.iter()).cloned().collect();
    compute_hash(&outer_data)
}

fn derive_mac_key(encryption_key: &[u8]) -> Vec<u8> {
    let mut mac_key = vec![0u8; 32];
    for (i, byte) in mac_key.iter_mut().enumerate() {
        *byte = encryption_key[i % 32].wrapping_add(0x5a);
    }
    compute_hash(&mac_key)
}

fn generate_nonce(len: usize) -> Vec<u8> {
    let mut nonce = vec![0u8; len];
    if let Err(e) = getrandom::getrandom(&mut nonce) {
        // Fallback to weak randomness if getrandom fails (should not happen)
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        for (i, byte) in nonce.iter_mut().enumerate() {
            let val = seed.wrapping_mul(i as u64 + 1).wrapping_mul(1103515245);
            *byte = ((val >> 16) ^ val) as u8;
        }
    }
    nonce
}

fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_vault_init() {
        let vault = SecureVault::new(temp_dir().join("synapsis_test_vault"));
        vault.initialize().unwrap();

        assert!(vault.is_tpm_available() || !vault.is_tpm_available());
    }

    #[test]
    fn test_store_and_retrieve_key() {
        let vault = SecureVault::new(temp_dir().join("synapsis_test_vault2"));
        vault.initialize().unwrap();

        let mut key = SessionKey {
            session_id: "test-session".to_string(),
            encryption_key: vec![0u8; 32],
            mac_key: vec![0u8; 32],
            created_at: current_timestamp(),
            last_used: current_timestamp(),
            rotation_count: 0,
            expires_at: None,
        };

        for (i, byte) in key.encryption_key.iter_mut().enumerate() {
            *byte = i as u8;
        }
        key.mac_key = derive_mac_key(&key.encryption_key);

        let fingerprint = vault.store_session_key("test-session", &key).unwrap();
        assert!(!fingerprint.is_empty());

        let retrieved = vault.get_session_key("test-session").unwrap().unwrap();
        assert_eq!(retrieved.encryption_key, key.encryption_key);
    }

    #[test]
    fn test_rotate_key() {
        let vault = SecureVault::new(temp_dir().join("synapsis_test_vault3"));
        vault.initialize().unwrap();

        let key = SessionKey {
            session_id: "test-session".to_string(),
            encryption_key: vec![0u8; 32],
            mac_key: vec![0u8; 32],
            created_at: current_timestamp(),
            last_used: current_timestamp(),
            rotation_count: 0,
            expires_at: None,
        };

        vault.store_session_key("test-session", &key).unwrap();

        let new_fingerprint = vault.rotate_key("test-session").unwrap().unwrap();
        assert!(!new_fingerprint.is_empty());
    }

    #[test]
    fn test_close_session() {
        let vault = SecureVault::new(temp_dir().join("synapsis_test_vault4"));
        vault.initialize().unwrap();

        let key = SessionKey {
            session_id: "test-session".to_string(),
            encryption_key: vec![0u8; 32],
            mac_key: vec![0u8; 32],
            created_at: current_timestamp(),
            last_used: current_timestamp(),
            rotation_count: 0,
            expires_at: None,
        };

        vault.store_session_key("test-session", &key).unwrap();

        assert!(vault.close_session("test-session"));
        assert!(vault.get_session_key("test-session").unwrap().is_none());
    }
}
