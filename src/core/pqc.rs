//! PQC (Post-Quantum Cryptography) Implementation
//! 
//! Provides post-quantum secure encryption, key exchange, and digital signatures.
//! Implements CRYSTALS-Kyber-512 for key exchange and CRYSTALS-Dilithium-4 for signatures.
//! Includes AES-256-GCM as a fallback for compatibility.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use pqcrypto_kyber::kyber512;
use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::kem::{PublicKey, SecretKey, Ciphertext, SharedSecret};
use pqcrypto_traits::sign::{PublicKey as SignPublicKey, SecretKey as SignSecretKey, DetachedSignature};
use rand::RngCore;

use super::security::SecureRng;

/// Encrypt data with AES-256-GCM
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("Key error: {}", e))?;
    
    let mut nonce_bytes = [0u8; 12];
    SecureRng::fill_random(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption error: {}", e))?;
    
    let mut result = Vec::new();
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    
    Ok(result)
}

/// Decrypt data with AES-256-GCM
pub fn decrypt(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < 12 {
        return Err("Ciphertext too short".to_string());
    }
    
    let nonce_bytes = &ciphertext[..12];
    let data = &ciphertext[12..];
    
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| format!("Key error: {}", e))?;
    
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext = cipher
        .decrypt(nonce, data)
        .map_err(|e| format!("Decryption error: {}", e))?;
    
    Ok(plaintext)
}

/// Generate a random key
pub fn generate_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    SecureRng::fill_random(&mut key);
    key
}

/// Generate a Kyber512 keypair
pub fn generate_kyber_keypair() -> Result<(Vec<u8>, Vec<u8>), String> {
    let (pk, sk) = kyber512::keypair();
    Ok((pk.as_bytes().to_vec(), sk.as_bytes().to_vec()))
}

/// Encapsulate a shared secret using Kyber512
pub fn kyber_encapsulate(pk: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let public_key = kyber512::PublicKey::from_bytes(pk)
        .map_err(|e| format!("Invalid public key: {}", e))?;
    let (ss, ct) = kyber512::encapsulate(&public_key);
    Ok((ct.as_bytes().to_vec(), ss.as_bytes().to_vec()))
}

/// Decapsulate a shared secret using Kyber512
pub fn kyber_decapsulate(ct: &[u8], sk: &[u8]) -> Result<Vec<u8>, String> {
    let secret_key = kyber512::SecretKey::from_bytes(sk)
        .map_err(|e| format!("Invalid secret key: {}", e))?;
    let ciphertext = kyber512::Ciphertext::from_bytes(ct)
        .map_err(|e| format!("Invalid ciphertext: {}", e))?;
    let ss = kyber512::decapsulate(&ciphertext, &secret_key);
    Ok(ss.as_bytes().to_vec())
}

/// Generate a Dilithium5 keypair
pub fn generate_dilithium_keypair() -> Result<(Vec<u8>, Vec<u8>), String> {
    let (pk, sk) = dilithium5::keypair();
    Ok((pk.as_bytes().to_vec(), sk.as_bytes().to_vec()))
}

/// Sign a message with Dilithium5
pub fn dilithium_sign(sk: &[u8], msg: &[u8]) -> Result<Vec<u8>, String> {
    let secret_key = dilithium5::SecretKey::from_bytes(sk)
        .map_err(|e| format!("Failed to parse secret key: {}", e))?;
    let sig = dilithium5::detached_sign(msg, &secret_key);
    Ok(sig.as_bytes().to_vec())
}

/// Verify a signature with Dilithium5
pub fn dilithium_verify(pk: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    let public_key = match dilithium5::PublicKey::from_bytes(pk) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    let signature = match dilithium5::DetachedSignature::from_bytes(sig) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    dilithium5::verify_detached_signature(&signature, msg, &public_key).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encrypt_decrypt() {
        let key = generate_key();
        let plaintext = b"Hello, World!";
        
        let ciphertext = encrypt(plaintext, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key).unwrap();
        
        assert_eq!(plaintext, &decrypted[..]);
    }
    
    #[test]
    fn test_kyber_key_exchange() {
        // Generate keypair
        let (pk, sk) = generate_kyber_keypair().unwrap();
        
        // Encapsulate shared secret
        let (ct, ss1) = kyber_encapsulate(&pk).unwrap();
        
        // Decapsulate shared secret
        let ss2 = kyber_decapsulate(&ct, &sk).unwrap();
        
        // Shared secrets should match
        assert_eq!(ss1, ss2);
        assert!(!ss1.is_empty());
    }
    
    #[test]
    fn test_dilithium_sign_verify() {
        // Generate keypair
        let (pk, sk) = generate_dilithium_keypair().unwrap();
        
        // Sign message
        let message = b"Test message for Dilithium signature";
        let signature = dilithium_sign(&sk, message).unwrap();
        
        // Verify signature
        let verified = dilithium_verify(&pk, message, &signature);
        assert!(verified, "Signature verification failed");
        
        // Verify wrong message fails
        let wrong_message = b"Wrong message";
        let wrong_verified = dilithium_verify(&pk, wrong_message, &signature);
        assert!(!wrong_verified, "Signature should not verify for wrong message");
        
        // Verify tampered signature fails
        let mut tampered_sig = signature.clone();
        if !tampered_sig.is_empty() {
            tampered_sig[0] ^= 0xFF; // Flip bits
        }
        let tampered_verified = dilithium_verify(&pk, message, &tampered_sig);
        assert!(!tampered_verified, "Tampered signature should not verify");
    }
}
