//! Synapsis Challenge-Response Authentication
//!
//! Provides challenge-response authentication for agents that don't have
//! Dilithium keys or TPM verification.
//!
//! # Flow
//!
//! ```text
//! 1. Client connects and sends registration info
//! 2. Server generates random challenge
//! 3. Client signs challenge with available method
//! 4. Server verifies response
//! ```

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub id: String,
    pub nonce: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub agent_type: String,
    pub verified: bool,
}

impl Challenge {
    pub fn is_expired(&self) -> bool {
        current_timestamp() > self.expires_at
    }
}

pub struct ChallengeResponse {
    challenges: Arc<RwLock<HashMap<String, Challenge>>>,
    challenge_ttl_secs: u64,
}

impl ChallengeResponse {
    pub fn new() -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            challenge_ttl_secs: 300,
        }
    }

    pub fn with_ttl(ttl_secs: u64) -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            challenge_ttl_secs: ttl_secs,
        }
    }

    pub fn generate_challenge(
        &self,
        _session_id: &str,
        agent_type: &str,
    ) -> Result<Challenge, ChallengeError> {
        let id = generate_id();
        let nonce = generate_nonce(32)?;
        let now = current_timestamp();

        let challenge = Challenge {
            id: id.clone(),
            nonce: nonce.clone(),
            created_at: now,
            expires_at: now + self.challenge_ttl_secs as i64,
            agent_type: agent_type.to_string(),
            verified: false,
        };

        {
            let mut challenges = self.challenges.write().unwrap();
            challenges.insert(id.clone(), challenge.clone());
        }

        Ok(challenge)
    }

    pub fn get_challenge(&self, challenge_id: &str) -> Option<Challenge> {
        let challenges = self.challenges.read().unwrap();
        challenges.get(challenge_id).cloned()
    }

    pub fn verify_response(
        &self,
        challenge_id: &str,
        response: &str,
        verifier: &dyn ResponseVerifier,
    ) -> Result<bool, ChallengeError> {
        let mut challenges = self.challenges.write().unwrap();

        let challenge = challenges
            .get_mut(challenge_id)
            .ok_or(ChallengeError::ChallengeNotFound)?;

        if challenge.is_expired() {
            return Err(ChallengeError::ChallengeExpired);
        }

        if challenge.verified {
            return Err(ChallengeError::AlreadyVerified);
        }

        if verifier.verify(&challenge.nonce, response)? {
            challenge.verified = true;
            return Ok(true);
        }

        Ok(false)
    }

    pub fn verify_and_consume(
        &self,
        challenge_id: &str,
        response: &str,
        verifier: &dyn ResponseVerifier,
    ) -> Result<bool, ChallengeError> {
        let result = self.verify_response(challenge_id, response, verifier)?;

        if result {
            let mut challenges = self.challenges.write().unwrap();
            challenges.remove(challenge_id);
        }

        Ok(result)
    }

    pub fn is_verified(&self, challenge_id: &str) -> bool {
        let challenges = self.challenges.read().unwrap();
        challenges
            .get(challenge_id)
            .map(|c| c.verified)
            .unwrap_or(false)
    }

    pub fn cleanup_expired(&self) -> usize {
        let _now = current_timestamp();
        let mut challenges = self.challenges.write().unwrap();
        let initial_len = challenges.len();

        challenges.retain(|_, c| !c.is_expired());

        initial_len - challenges.len()
    }

    pub fn revoke_challenge(&self, challenge_id: &str) -> bool {
        let mut challenges = self.challenges.write().unwrap();
        challenges.remove(challenge_id).is_some()
    }

    pub fn revoke_all_for_agent(&self, agent_type: &str) -> usize {
        let mut challenges = self.challenges.write().unwrap();
        let initial_len = challenges.len();

        challenges.retain(|_, c| c.agent_type != agent_type);

        initial_len - challenges.len()
    }
}

impl Default for ChallengeResponse {
    fn default() -> Self {
        Self::new()
    }
}

pub trait ResponseVerifier: Send + Sync {
    fn verify(&self, nonce: &str, response: &str) -> Result<bool, ChallengeError>;
}

pub struct HmacVerifier {
    secret: Vec<u8>,
}

impl HmacVerifier {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            secret: secret.to_vec(),
        }
    }
}

impl ResponseVerifier for HmacVerifier {
    fn verify(&self, nonce: &str, response: &str) -> Result<bool, ChallengeError> {
        let expected = compute_hmac_sha256(&self.secret, nonce.as_bytes());
        let expected_b64 = BASE64.encode(&expected);

        Ok(constant_time_compare(&expected_b64, response))
    }
}

pub struct ApiKeyVerifier {
    api_keys: HashMap<String, String>,
}

impl ApiKeyVerifier {
    pub fn new(api_keys: HashMap<String, String>) -> Self {
        Self { api_keys }
    }
}

impl ResponseVerifier for ApiKeyVerifier {
    fn verify(&self, nonce: &str, response: &str) -> Result<bool, ChallengeError> {
        if let Some(secret) = self.api_keys.get(response) {
            let expected = compute_hmac_sha256(secret.as_bytes(), nonce.as_bytes());
            let expected_b64 = BASE64.encode(&expected);

            Ok(constant_time_compare(&expected_b64, response))
        } else {
            Ok(false)
        }
    }
}

pub struct SimpleVerifier {
    pub password: String,
}

impl SimpleVerifier {
    pub fn new(password: &str) -> Self {
        Self {
            password: password.to_string(),
        }
    }
}

impl ResponseVerifier for SimpleVerifier {
    fn verify(&self, _nonce: &str, response: &str) -> Result<bool, ChallengeError> {
        Ok(constant_time_compare(&self.password, response))
    }
}

pub struct ChallengeResponseBuilder {
    challenge_response: ChallengeResponse,
    verifiers: Vec<Box<dyn ResponseVerifier>>,
}

impl ChallengeResponseBuilder {
    pub fn new() -> Self {
        Self {
            challenge_response: ChallengeResponse::new(),
            verifiers: Vec::new(),
        }
    }

    pub fn with_hmac_verifier(mut self, secret: &[u8]) -> Self {
        self.verifiers.push(Box::new(HmacVerifier::new(secret)));
        self
    }

    pub fn with_api_key_verifier(mut self, api_keys: HashMap<String, String>) -> Self {
        self.verifiers.push(Box::new(ApiKeyVerifier::new(api_keys)));
        self
    }

    pub fn with_simple_verifier(mut self, password: &str) -> Self {
        self.verifiers.push(Box::new(SimpleVerifier::new(password)));
        self
    }

    pub fn build(self) -> (ChallengeResponse, Vec<Box<dyn ResponseVerifier>>) {
        (self.challenge_response, self.verifiers)
    }
}

impl Default for ChallengeResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum ChallengeError {
    ChallengeNotFound,
    ChallengeExpired,
    AlreadyVerified,
    VerificationFailed,
    CryptoError(String),
}

impl std::fmt::Display for ChallengeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChallengeError::ChallengeNotFound => write!(f, "Challenge not found"),
            ChallengeError::ChallengeExpired => write!(f, "Challenge has expired"),
            ChallengeError::AlreadyVerified => write!(f, "Challenge already verified"),
            ChallengeError::VerificationFailed => write!(f, "Response verification failed"),
            ChallengeError::CryptoError(e) => write!(f, "Crypto error: {}", e),
        }
    }
}

impl std::error::Error for ChallengeError {}

fn generate_id() -> String {
    let mut id = vec![0u8; 16];
    fill_random(&mut id);
    hex_encode(&id)
}

fn generate_nonce(len: usize) -> Result<String, ChallengeError> {
    let mut nonce = vec![0u8; len];
    fill_random(&mut nonce);
    Ok(BASE64.encode(&nonce))
}

fn fill_random(dest: &mut [u8]) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    for (i, byte) in dest.iter_mut().enumerate() {
        let val = seed.wrapping_mul(i as u64 + 1).wrapping_mul(1103515245);
        *byte = ((val >> 16) ^ val) as u8;
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

fn compute_hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let block_size = 64;
    let mut key_block = vec![0u8; block_size];

    if key.len() > block_size {
        for (i, byte) in key.iter().enumerate().take(block_size) {
            key_block[i] = *byte;
        }
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
    let inner_hash = sha256_simple(&inner_data);

    let outer_data: Vec<u8> = outer_pad.iter().chain(inner_hash.iter()).cloned().collect();
    sha256_simple(&outer_data)
}

fn sha256_simple(data: &[u8]) -> Vec<u8> {
    let h = [
        0x6a09e667u32,
        0xbb67ae85u32,
        0x3c6ef372u32,
        0xa54ff53au32,
        0x510e527fu32,
        0x9b05688cu32,
        0x1f83d9abu32,
        0x5be0cd19u32,
    ];

    let k = [
        0x428a2f98u32,
        0x71374491u32,
        0xb5c0fbcfu32,
        0xe9b5dba5u32,
        0x3956c25bu32,
        0x59f111f1u32,
        0x923f82a4u32,
        0xab1c5ed5u32,
        0xd807aa98u32,
        0x12835b01u32,
        0x243185beu32,
        0x550c7dc3u32,
        0x72be5d74u32,
        0x80deb1feu32,
        0x9bdc06a7u32,
        0xc19bf174u32,
        0xe49b69c1u32,
        0xefbe4786u32,
        0x0fc19dc6u32,
        0x240ca1ccu32,
        0x2de92c6fu32,
        0x4a7484aau32,
        0x5cb0a9dcu32,
        0x76f988dau32,
        0x983e5152u32,
        0xa831c66du32,
        0xb00327c8u32,
        0xbf597fc7u32,
        0xc6e00bf3u32,
        0xd5a79147u32,
        0x06ca6351u32,
        0x14292967u32,
        0x27b70a85u32,
        0x2e1b2138u32,
        0x4d2c6dfcu32,
        0x53380d13u32,
        0x650a7354u32,
        0x766a0abbu32,
        0x81c2c92eu32,
        0x92722c85u32,
        0xa2bfe8a1u32,
        0xa81a664bu32,
        0xc24b8b70u32,
        0xc76c51a3u32,
        0xd192e819u32,
        0xd6990624u32,
        0xf40e3585u32,
        0x106aa070u32,
        0x19a4c116u32,
        0x1e376c08u32,
        0x2748774cu32,
        0x34b0bcb5u32,
        0x391c0cb3u32,
        0x4ed8aa4au32,
        0x5b9cca4fu32,
        0x682e6ff3u32,
        0x748f82eeu32,
        0x78a5636fu32,
        0x84c87814u32,
        0x8cc70208u32,
        0x90befffau32,
        0xa4506cebu32,
        0xbef9a3f7u32,
        0xc67178f2u32,
    ];

    let mut w = [0u32; 64];

    for i in 0..16 {
        let offset = i * 4;
        w[i] = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
    }

    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let mut a = h[0];
    let mut b = h[1];
    let mut c = h[2];
    let mut d = h[3];
    let mut e = h[4];
    let mut f = h[5];
    let mut g = h[6];
    let mut hh = h[7];

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(k[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    let mut result = Vec::with_capacity(32);
    for (i, val) in [a, b, c, d, e, f, g, hh].iter().enumerate() {
        result.extend_from_slice(&val.wrapping_add(h[i]).to_be_bytes());
    }

    result
}

fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }

    result == 0
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

    #[test]
    fn test_challenge_generation() {
        let cr = ChallengeResponse::new();
        let challenge = cr.generate_challenge("session-1", "test-agent").unwrap();

        assert!(!challenge.nonce.is_empty());
        assert_eq!(challenge.agent_type, "test-agent");
        assert!(!challenge.verified);
    }

    #[test]
    fn test_challenge_verification() {
        let cr = ChallengeResponse::new();
        let verifier = SimpleVerifier::new("password123");

        let challenge = cr.generate_challenge("session-1", "test-agent").unwrap();
        let response = "password123";

        let result = cr.verify_and_consume(&challenge.id, response, &verifier);

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_challenge_expiration() {
        let cr = ChallengeResponse::with_ttl(1);
        let challenge = cr.generate_challenge("session-1", "test-agent").unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));

        let verifier = SimpleVerifier::new("password123");
        let result = cr.verify_response(&challenge.id, "password123", &verifier);

        assert!(matches!(result, Err(ChallengeError::ChallengeExpired)));
    }

    #[test]
    fn test_cleanup_expired() {
        let cr = ChallengeResponse::with_ttl(1);

        cr.generate_challenge("session-1", "test-agent").unwrap();
        std::thread::sleep(std::time::Duration::from_secs(2));

        let cleaned = cr.cleanup_expired();
        assert_eq!(cleaned, 1);
    }
}
