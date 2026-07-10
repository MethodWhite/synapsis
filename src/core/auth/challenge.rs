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

use crate::core::lock_utils::*;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
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
    failed_attempts: Arc<RwLock<HashMap<String, u32>>>,
    max_failed_per_agent: u32,
}

const DEFAULT_MAX_FAILED: u32 = 5;

impl ChallengeResponse {
    pub fn new() -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            challenge_ttl_secs: 300,
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
            max_failed_per_agent: DEFAULT_MAX_FAILED,
        }
    }

    pub fn with_ttl(ttl_secs: u64) -> Self {
        Self {
            challenges: Arc::new(RwLock::new(HashMap::new())),
            challenge_ttl_secs: ttl_secs,
            failed_attempts: Arc::new(RwLock::new(HashMap::new())),
            max_failed_per_agent: DEFAULT_MAX_FAILED,
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
            let mut challenges = self.challenges.write_safe();
            challenges.insert(id.clone(), challenge.clone());
        }

        Ok(challenge)
    }

    pub fn get_challenge(&self, challenge_id: &str) -> Option<Challenge> {
        let challenges = self.challenges.read_safe();
        challenges.get(challenge_id).cloned()
    }

    pub fn verify_response(
        &self,
        challenge_id: &str,
        response: &str,
        verifier: &dyn ResponseVerifier,
    ) -> Result<bool, ChallengeError> {
        let agent_type = {
            let challenges = self.challenges.read_safe();
            challenges.get(challenge_id).map(|c| c.agent_type.clone())
        };

        if let Some(ref agent) = agent_type {
            let failed = self.failed_attempts.read_safe();
            if failed.get(agent).copied().unwrap_or(0) >= self.max_failed_per_agent {
                return Err(ChallengeError::RateLimited);
            }
        }

        let mut challenges = self.challenges.write_safe();

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
            self.failed_attempts
                .write_safe()
                .remove(&agent_type.unwrap_or_default());
            return Ok(true);
        }

        if let Some(agent) = agent_type {
            let mut failed = self.failed_attempts.write_safe();
            *failed.entry(agent).or_insert(0) += 1;
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
            let mut challenges = self.challenges.write_safe();
            challenges.remove(challenge_id);
        }

        Ok(result)
    }

    pub fn is_verified(&self, challenge_id: &str) -> bool {
        let challenges = self.challenges.read_safe();
        challenges
            .get(challenge_id)
            .map(|c| c.verified)
            .unwrap_or(false)
    }

    pub fn cleanup_expired(&self) -> usize {
        let _now = current_timestamp();
        let mut challenges = self.challenges.write_safe();
        let initial_len = challenges.len();

        challenges.retain(|_, c| !c.is_expired());

        initial_len - challenges.len()
    }

    pub fn revoke_challenge(&self, challenge_id: &str) -> bool {
        let mut challenges = self.challenges.write_safe();
        challenges.remove(challenge_id).is_some()
    }

    pub fn revoke_all_for_agent(&self, agent_type: &str) -> usize {
        let mut challenges = self.challenges.write_safe();
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
    valid_keys: Vec<String>,
}

impl ApiKeyVerifier {
    pub fn new(valid_keys: Vec<String>) -> Self {
        Self { valid_keys }
    }
}

impl ResponseVerifier for ApiKeyVerifier {
    fn verify(&self, _nonce: &str, response: &str) -> Result<bool, ChallengeError> {
        Ok(self.valid_keys.iter().any(|k| k == response))
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

    pub fn with_api_key_verifier(mut self, api_keys: Vec<String>) -> Self {
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
    RateLimited,
    CryptoError(String),
}

impl std::fmt::Display for ChallengeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChallengeError::ChallengeNotFound => write!(f, "Challenge not found"),
            ChallengeError::ChallengeExpired => write!(f, "Challenge has expired"),
            ChallengeError::AlreadyVerified => write!(f, "Challenge already verified"),
            ChallengeError::VerificationFailed => write!(f, "Response verification failed"),
            ChallengeError::RateLimited => write!(f, "Rate limited: too many failed attempts"),
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
    getrandom::getrandom(dest).unwrap();
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

fn compute_hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn constant_time_compare(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let max_len = a_bytes.len().max(b_bytes.len());
    let mut result: u8 = if a_bytes.len() != b_bytes.len() {
        0xFF
    } else {
        0
    };
    for i in 0..max_len {
        let x = a_bytes.get(i).copied().unwrap_or(0);
        let y = b_bytes.get(i).copied().unwrap_or(0);
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
