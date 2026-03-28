// CRYSTALS-Dilithium Digital Signature Module
// Provides post-quantum digital signatures using CRYSTALS-Dilithium
// NIST Level 1 security (Dilithium2)

use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};

/// Dilithium2 keypair for digital signatures
#[derive(Clone)]
pub struct DilithiumKeypair {
    pub public_key: dilithium2::PublicKey,
    pub secret_key: dilithium2::SecretKey,
}

impl DilithiumKeypair {
    /// Generate a new Dilithium2 keypair
    pub fn generate() -> Self {
        let (pk, sk) = dilithium2::keypair();
        Self { public_key: pk, secret_key: sk }
    }
    
    /// Get public key size (Dilithium2: 1312 bytes)
    pub fn public_key_size() -> usize {
        1312
    }
    
    /// Get secret key size (Dilithium2: 2560 bytes)
    pub fn secret_key_size() -> usize {
        2560
    }
}

/// Sign a message using Dilithium2
pub fn sign_message(secret_key: &dilithium2::SecretKey, message: &[u8]) -> dilithium2::SignedMessage {
    dilithium2::sign(message, secret_key)
}

/// Verify a signed message
pub fn verify_signature(signed_msg: &dilithium2::SignedMessage, public_key: &dilithium2::PublicKey) -> Result<Vec<u8>, String> {
    dilithium2::open(signed_msg, public_key)
        .map_err(|e| format!("Verification failed: {:?}", e))
}

/// Sign and verify roundtrip
pub fn sign_verify_roundtrip(message: &[u8]) -> Result<bool, String> {
    let keypair = DilithiumKeypair::generate();
    let signed = sign_message(&keypair.secret_key, message);
    let opened = verify_signature(&signed, &keypair.public_key)?;
    Ok(opened == message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dilithium2_keypair() {
        let kp = DilithiumKeypair::generate();
        // Dilithium2 actual sizes
        assert_eq!(kp.public_key.as_bytes().len(), 1312);  // Actual PK size
        assert_eq!(kp.secret_key.as_bytes().len(), 2560);  // Actual SK size
    }

    #[test]
    fn test_dilithium2_sign_verify() {
        let msg = b"Test message";
        let kp = DilithiumKeypair::generate();
        let signed = sign_message(&kp.secret_key, msg);
        let opened = verify_signature(&signed, &kp.public_key).unwrap();
        assert_eq!(opened, msg);
    }

    #[test]
    fn test_dilithium2_tampered() {
        let msg = b"Original";
        let kp = DilithiumKeypair::generate();
        let signed = sign_message(&kp.secret_key, msg);
        
        // Tamper
        let mut bytes = signed.as_bytes().to_vec();
        if bytes.len() > 10 {
            bytes[5] ^= 0xFF;
        }
        let tampered = dilithium2::SignedMessage::from_bytes(&bytes).unwrap();
        
        // Should fail
        assert!(verify_signature(&tampered, &kp.public_key).is_err());
    }

    #[test]
    fn test_dilithium2_roundtrip() {
        assert!(sign_verify_roundtrip(b"Test").unwrap());
    }
}
