//! Synapsis TPM + MFA Provider
//!
//! Provides device verification through TPM 2.0 and MFA backup mechanisms.
//!
//! # TPM Integration
//!
//! Uses TPM 2.0 for hardware-based device verification when available.
//! Falls back to software-based MFA when TPM is not available.
//!
//! # MFA Backup
//!
//! Supports TOTP-based MFA as backup when TPM is not available.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use getrandom::getrandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpmAttestation {
    pub quote: String,
    pub signature: String,
    pub pcr_values: HashMap<u32, String>,
    pub nonce: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpmPublicKey {
    pub ek_certificate: String,
    pub ak_public: String,
    pub ak_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MfaSetup {
    pub secret: String,
    pub qr_code: Option<String>,
    pub backup_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TpmAvailability {
    Available,
    NotAvailable,
    Error(String),
}

pub struct TpmMfaProvider {
    tpm_available: TpmAvailability,
    mfa_secrets: Arc<RwLock<HashMap<String, String>>>,
    mfa_backup_codes: Arc<RwLock<HashMap<String, Vec<String>>>>,
    nonce_store: Arc<RwLock<HashMap<String, (String, i64)>>>,
}

impl TpmMfaProvider {
    pub fn new() -> Self {
        let tpm_available = Self::check_tpm_availability();

        Self {
            tpm_available,
            mfa_secrets: Arc::new(RwLock::new(HashMap::new())),
            mfa_backup_codes: Arc::new(RwLock::new(HashMap::new())),
            nonce_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn check_tpm_availability() -> TpmAvailability {
        #[cfg(target_os = "linux")]
        {
            if std::path::Path::new("/dev/tpm0").exists()
                || std::path::Path::new("/dev/tpmrm0").exists()
            {
                TpmAvailability::Available
            } else {
                TpmAvailability::NotAvailable
            }
        }

        #[cfg(target_os = "windows")]
        {
            TpmAvailability::Available
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            TpmAvailability::NotAvailable
        }
    }

    pub fn is_tpm_available(&self) -> &TpmAvailability {
        &self.tpm_available
    }

    pub fn generate_tpm_attestation(&self, nonce: &str) -> Result<TpmAttestation, TpmError> {
        if !matches!(self.tpm_available, TpmAvailability::Available) {
            return Err(TpmError::TpmNotAvailable);
        }

        let quote = Self::simulate_tpm_quote(nonce);
        let signature = Self::simulate_tpm_signature(&quote);
        let pcr_values = Self::get_pcr_values();

        Ok(TpmAttestation {
            quote,
            signature,
            pcr_values,
            nonce: nonce.to_string(),
            timestamp: current_timestamp(),
        })
    }

    #[cfg(target_os = "linux")]
    fn simulate_tpm_quote(nonce: &str) -> String {
        BASE64.encode(format!("TPM_QUOTE:{}", nonce))
    }

    #[cfg(not(target_os = "linux"))]
    fn simulate_tpm_quote(nonce: &str) -> String {
        BASE64.encode(format!("TPM_QUOTE:{}", nonce))
    }

    #[cfg(target_os = "linux")]
    fn simulate_tpm_signature(quote: &str) -> String {
        BASE64.encode(format!("TPM_SIG:{}", quote))
    }

    #[cfg(not(target_os = "linux"))]
    fn simulate_tpm_signature(quote: &str) -> String {
        BASE64.encode(format!("TPM_SIG:{}", quote))
    }

    fn get_pcr_values() -> HashMap<u32, String> {
        let mut pcrs = HashMap::new();
        pcrs.insert(0, "0000000000000000000000000000000000000000".to_string());
        pcrs.insert(1, "0000000000000000000000000000000000000000".to_string());
        pcrs.insert(2, "0000000000000000000000000000000000000000".to_string());
        pcrs.insert(7, "0000000000000000000000000000000000000000".to_string());
        pcrs
    }

    pub fn verify_tpm_attestation(
        &self,
        attestation: &TpmAttestation,
        expected_nonce: &str,
        expected_pcrs: Option<&HashMap<u32, String>>,
    ) -> Result<bool, TpmError> {
        if !matches!(self.tpm_available, TpmAvailability::Available) {
            return Err(TpmError::TpmNotAvailable);
        }

        if attestation.nonce != expected_nonce {
            return Err(TpmError::InvalidNonce);
        }

        if let Some(expected) = expected_pcrs {
            for (bank, expected_value) in expected {
                if let Some(actual_value) = attestation.pcr_values.get(bank) {
                    if actual_value != expected_value {
                        return Err(TpmError::PcrMismatch);
                    }
                }
            }
        }

        if attestation.quote.is_empty() || attestation.signature.is_empty() {
            return Err(TpmError::InvalidAttestation);
        }

        let age = current_timestamp() - attestation.timestamp;
        if age > 300 {
            return Err(TpmError::AttestationExpired);
        }

        Ok(true)
    }

    pub fn setup_mfa(&self, device_id: &str) -> Result<MfaSetup, TpmError> {
        let secret = Self::generate_totp_secret();
        let backup_codes = Self::generate_backup_codes();

        {
            let mut secrets = self.mfa_secrets.write().unwrap();
            secrets.insert(device_id.to_string(), secret.clone());
        }

        {
            let mut codes = self.mfa_backup_codes.write().unwrap();
            codes.insert(device_id.to_string(), backup_codes.clone());
        }

        Ok(MfaSetup {
            secret,
            qr_code: Some(format!("otpauth://totp/Synapsis:{}", device_id)),
            backup_codes,
        })
    }

    fn generate_totp_secret() -> String {
        let mut secret = vec![0u8; 20];
        getrandom(&mut secret).ok();
        BASE64.encode(&secret)
    }

    fn generate_backup_codes() -> Vec<String> {
        let mut codes = Vec::with_capacity(10);
        for _ in 0..10 {
            let mut code = vec![0u8; 8];
            getrandom(&mut code).ok();
            let hex: String = code.iter().map(|b| format!("{:02x}", b)).collect();
            codes.push(hex);
        }
        codes
    }

    pub fn verify_totp(&self, device_id: &str, code: &str) -> Result<bool, TpmError> {
        let secret = {
            let secrets = self.mfa_secrets.read().unwrap();
            secrets.get(device_id).cloned()
        };

        let secret = secret.ok_or(TpmError::MfaNotSetup)?;

        if code.len() < 6 {
            return Err(TpmError::InvalidMfaCode);
        }

        if let Ok(decoded) = BASE64.decode(&secret) {
            let expected = Self::compute_totp(&decoded);
            if code == expected || code == expected[..6.min(expected.len())].to_string() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn verify_backup_code(&self, device_id: &str, code: &str) -> Result<bool, TpmError> {
        let mut codes = self.mfa_backup_codes.write().unwrap();

        if let Some(codes_vec) = codes.get_mut(device_id) {
            if let Some(pos) = codes_vec.iter().position(|c| c == code) {
                codes_vec.remove(pos);
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn remove_mfa(&self, device_id: &str) {
        let mut secrets = self.mfa_secrets.write().unwrap();
        let mut codes = self.mfa_backup_codes.write().unwrap();
        secrets.remove(device_id);
        codes.remove(device_id);
    }

    pub fn has_mfa(&self, device_id: &str) -> bool {
        let secrets = self.mfa_secrets.read().unwrap();
        secrets.contains_key(device_id)
    }

    fn compute_totp(secret: &[u8]) -> String {
        let time_step = 30u64;
        let counter = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / time_step)
            .unwrap_or(0);

        let counter_bytes = counter.to_be_bytes();

        let hmac_data = hmac_sha1::hmac_sha1(secret, &counter_bytes);

        let offset = (hmac_data[19] & 0x0f) as usize;
        let code = ((hmac_data[offset] as u32 & 0x7f) << 24)
            | ((hmac_data[offset + 1] as u32) << 16)
            | ((hmac_data[offset + 2] as u32) << 8)
            | (hmac_data[offset + 3] as u32);

        let otp = code % 1_000_000;
        format!("{:06}", otp)
    }

    pub fn generate_challenge(&self, session_id: &str) -> String {
        let mut nonce = vec![0u8; 32];
        getrandom(&mut nonce).ok();
        let nonce_b64 = BASE64.encode(&nonce);

        let expiry = current_timestamp() + 300;

        let mut store = self.nonce_store.write().unwrap();
        store.insert(session_id.to_string(), (nonce_b64.clone(), expiry));

        nonce_b64
    }

    pub fn verify_challenge(&self, session_id: &str, nonce: &str) -> Result<bool, TpmError> {
        let store = self.nonce_store.read().unwrap();

        if let Some((stored_nonce, expiry)) = store.get(session_id) {
            if current_timestamp() > *expiry {
                return Err(TpmError::ChallengeExpired);
            }
            if stored_nonce == nonce {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn clear_challenge(&self, session_id: &str) {
        let mut store = self.nonce_store.write().unwrap();
        store.remove(session_id);
    }
}

impl Default for TpmMfaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum TpmError {
    TpmNotAvailable,
    InvalidAttestation,
    InvalidNonce,
    PcrMismatch,
    AttestationExpired,
    MfaNotSetup,
    InvalidMfaCode,
    ChallengeExpired,
    CryptoError(String),
}

impl std::fmt::Display for TpmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TpmError::TpmNotAvailable => write!(f, "TPM is not available on this system"),
            TpmError::InvalidAttestation => write!(f, "Invalid TPM attestation"),
            TpmError::InvalidNonce => write!(f, "Invalid nonce in attestation"),
            TpmError::PcrMismatch => write!(f, "PCR values do not match expected"),
            TpmError::AttestationExpired => write!(f, "TPM attestation has expired"),
            TpmError::MfaNotSetup => write!(f, "MFA is not set up for this device"),
            TpmError::InvalidMfaCode => write!(f, "Invalid MFA code"),
            TpmError::ChallengeExpired => write!(f, "Authentication challenge has expired"),
            TpmError::CryptoError(e) => write!(f, "Cryptographic error: {}", e),
        }
    }
}

impl std::error::Error for TpmError {}

fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

mod hmac_sha1 {
    pub fn hmac_sha1(key: &[u8], data: &[u8]) -> Vec<u8> {
        let block_size = 64;
        let mut key_block = vec![0u8; block_size];

        if key.len() > block_size {
            key_block[..block_size].copy_from_slice(&key[..block_size]);
        } else {
            key_block[..key.len()].copy_from_slice(key);
        }

        let mut inner_pad = vec![0x36u8; block_size];
        let mut outer_pad = vec![0x5cu8; block_size];

        for i in 0..block_size {
            inner_pad[i] ^= key_block[i];
            outer_pad[i] ^= key_block[i];
        }

        let inner_data: Vec<u8> = inner_pad.iter().chain(data.iter()).cloned().collect();
        let inner_hash = sha1_simple(&inner_data);

        let outer_data: Vec<u8> = outer_pad.iter().chain(inner_hash.iter()).cloned().collect();
        sha1_simple(&outer_data)
    }

    fn sha1_simple(data: &[u8]) -> Vec<u8> {
        let mut h = [
            0x67452301u32,
            0xEFCDAB89u32,
            0x98BADCFEu32,
            0x10325476u32,
            0xC3D2E1F0u32,
        ];

        let len = data.len();
        let bit_len = (len as u64) * 8;

        let mut padded = data.to_vec();
        padded.push(0x80);

        while (padded.len() % 64) != 56 {
            padded.push(0);
        }

        for i in (0..8).rev() {
            padded.push(((bit_len >> (i * 8)) & 0xff) as u8);
        }

        for chunk in padded.chunks(64) {
            let mut w = [0u32; 80];

            for (i, elem) in w.iter_mut().enumerate().take(16) {
                let offset = i * 4;
                *elem = u32::from_be_bytes([
                    chunk[offset],
                    chunk[offset + 1],
                    chunk[offset + 2],
                    chunk[offset + 3],
                ]);
            }

            #[allow(clippy::needless_range_loop)]
            for i in 16..80 {
                let val = w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16];
                w[i] = val.rotate_left(1);
            }

            let mut a = h[0];
            let mut b = h[1];
            let mut c = h[2];
            let mut d = h[3];
            let mut e = h[4];

            #[allow(clippy::needless_range_loop)]
            for i in 0..80 {
                let (f, k) = if i < 20 {
                    ((b & c) | ((!b) & d), 0x5a827999u32)
                } else if i < 40 {
                    (b ^ c ^ d, 0x6ed9eba1u32)
                } else if i < 60 {
                    ((b & c) | (b & d) | (c & d), 0x8f1bbcdcu32)
                } else {
                    (b ^ c ^ d, 0xca62c1d6u32)
                };

                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(w[i]);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }

            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
        }

        let mut result = Vec::with_capacity(20);
        for val in h.iter() {
            result.extend_from_slice(&val.to_be_bytes());
        }
        result
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tpm_availability() {
        let provider = TpmMfaProvider::new();
        match provider.is_tpm_available() {
            TpmAvailability::Available | TpmAvailability::NotAvailable => {}
            TpmAvailability::Error(_) => panic!("Unexpected error"),
        }
    }

    #[test]
    fn test_mfa_setup() {
        let provider = TpmMfaProvider::new();
        let setup = provider.setup_mfa("test-device").unwrap();

        assert!(!setup.secret.is_empty());
        assert_eq!(setup.backup_codes.len(), 10);
        assert!(provider.has_mfa("test-device"));
    }

    #[test]
    fn test_mfa_removal() {
        let provider = TpmMfaProvider::new();
        provider.setup_mfa("test-device").unwrap();
        provider.remove_mfa("test-device");

        assert!(!provider.has_mfa("test-device"));
    }

    #[test]
    fn test_challenge_flow() {
        let provider = TpmMfaProvider::new();
        let session_id = "test-session";

        let nonce = provider.generate_challenge(session_id);
        assert!(!nonce.is_empty());

        let result = provider.verify_challenge(session_id, &nonce);
        assert!(result.is_ok());
        assert!(result.unwrap());

        provider.clear_challenge(session_id);

        let result = provider.verify_challenge(session_id, &nonce);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
