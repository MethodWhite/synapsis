//! PQC Integration Tests
//!
//! Verify that post-quantum cryptography is properly integrated
//! and functional across the Synapsis security stack.

use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use synapsis::core::pqc;
use synapsis::core::vault::SecureVault;
use std::path::PathBuf;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn create_test_vault() -> (SecureVault, String) {
    // Generate unique test directory
    let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let test_dir = format!("/tmp/synapsis-vault-test-{}-{}", test_id, timestamp);
    
    println!("[TEST] Creating test vault directory: {}", test_dir);
    let data_dir = PathBuf::from(&test_dir);
    std::fs::create_dir_all(&data_dir).unwrap();
    
    // Set PQC passphrase environment variable (required by vault)
    env::set_var("SYNAPSIS_PQC_PASSPHRASE", "test-pqc-passphrase");
    
    let vault = SecureVault::new(data_dir);
    (vault, test_dir)
}

#[test]
fn test_kyber_key_exchange_integration() {
    // Generate Kyber keypair
    let (pk, sk) = pqc::generate_kyber_keypair().expect("Failed to generate Kyber keypair");
    
    // Encapsulate shared secret
    let (ciphertext, shared_secret1) = pqc::kyber_encapsulate(&pk).expect("Failed to encapsulate");
    
    // Decapsulate shared secret
    let shared_secret2 = pqc::kyber_decapsulate(&ciphertext, &sk).expect("Failed to decapsulate");
    
    // Shared secrets should match
    assert_eq!(shared_secret1, shared_secret2);
    assert!(!shared_secret1.is_empty(), "Shared secret should not be empty");
    
    // Use shared secret as AES key (first 32 bytes)
    let aes_key = if shared_secret1.len() >= 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&shared_secret1[..32]);
        key
    } else {
        // Pad if necessary (Kyber-512 produces 32-byte shared secret)
        let mut key = [0u8; 32];
        key[..shared_secret1.len()].copy_from_slice(&shared_secret1);
        key
    };
    
    // Encrypt and decrypt with AES-256-GCM
    let plaintext = b"Test message for PQC integration";
    let ciphertext_aes = pqc::encrypt(plaintext, &aes_key).expect("AES encryption failed");
    let decrypted = pqc::decrypt(&ciphertext_aes, &aes_key).expect("AES decryption failed");
    
    assert_eq!(plaintext, &decrypted[..]);
}

#[test]
fn test_dilithium_signature_integration() {
    // Generate Dilithium keypair
    let (pk, sk) = pqc::generate_dilithium_keypair().expect("Failed to generate Dilithium keypair");
    
    // Sign a message
    let message = b"Critical security operation: delete observation 123";
    let signature = pqc::dilithium_sign(&sk, message).expect("Failed to sign message");
    
    // Verify signature
    let verified = pqc::dilithium_verify(&pk, message, &signature);
    assert!(verified, "Valid signature should verify");
    
    // Verify wrong message fails
    let wrong_message = b"Critical security operation: delete observation 124";
    let wrong_verified = pqc::dilithium_verify(&pk, wrong_message, &signature);
    assert!(!wrong_verified, "Signature should not verify for wrong message");
    
    // Verify tampered signature fails
    let mut tampered_sig = signature.clone();
    if !tampered_sig.is_empty() {
        tampered_sig[0] ^= 0xFF; // Flip bits
    }
    let tampered_verified = pqc::dilithium_verify(&pk, message, &tampered_sig);
    assert!(!tampered_verified, "Tampered signature should not verify");
}

#[test]
fn test_vault_pqc_initialization() {
    use synapsis::core::vault::SessionKey;
    
    let (vault, test_dir) = create_test_vault();
    
    // Initialize vault - should generate PQC keypair and master key
    vault.initialize().expect("Vault initialization failed");
    
    // Check that vault can store and retrieve a session key
    let session_id = "test-session-1";
    let encryption_key = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32];
    let mac_key = vec![33, 34, 35, 36, 37, 38, 39, 40];
    
    let session_key = SessionKey {
        session_id: session_id.to_string(),
        encryption_key: encryption_key.clone(),
        mac_key: mac_key.clone(),
        created_at: 1234567890,
        last_used: 1234567890,
        rotation_count: 0,
        expires_at: None,
    };
    
    vault.store_session_key(session_id, &session_key)
        .expect("Failed to store session key");
    
    let retrieved = vault.get_session_key(session_id)
        .expect("Failed to retrieve session key")
        .expect("Session key should exist");
    
    assert_eq!(retrieved.encryption_key, encryption_key);
    assert_eq!(retrieved.mac_key, mac_key);
    
    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
}

#[test]
fn test_pqc_randomness_security() {
    // Generate multiple keys to ensure randomness
    let key1 = pqc::generate_key();
    let key2 = pqc::generate_key();
    
    // Keys should be different (extremely low probability of collision)
    assert_ne!(key1, key2, "Random keys should be different");
    
    // Check key length
    assert_eq!(key1.len(), 32, "AES-256 key should be 32 bytes");
    assert_eq!(key2.len(), 32, "AES-256 key should be 32 bytes");
    
    // Test encryption with different keys
    let plaintext = b"Test message";
    let ciphertext1 = pqc::encrypt(plaintext, &key1).expect("Encryption with key1 failed");
    let ciphertext2 = pqc::encrypt(plaintext, &key2).expect("Encryption with key2 failed");
    
    // Ciphertexts should be different (due to random nonce)
    assert_ne!(ciphertext1, ciphertext2, "Ciphertexts should differ due to random nonce");
    
    // Each should decrypt with its own key
    let decrypted1 = pqc::decrypt(&ciphertext1, &key1).expect("Decryption with key1 failed");
    let decrypted2 = pqc::decrypt(&ciphertext2, &key2).expect("Decryption with key2 failed");
    
    assert_eq!(plaintext, &decrypted1[..]);
    assert_eq!(plaintext, &decrypted2[..]);
    
    // Wrong key should fail
    let wrong_decrypt = pqc::decrypt(&ciphertext1, &key2);
    assert!(wrong_decrypt.is_err(), "Decryption with wrong key should fail");
}

#[test]
fn test_pqc_comprehensive() {
    // This test demonstrates a comprehensive PQC workflow:
    // 1. Generate Kyber keypair for key exchange
    // 2. Generate Dilithium keypair for signatures
    // 3. Perform key exchange to establish shared secret
    // 4. Use shared secret to encrypt a message
    // 5. Sign the encrypted message with Dilithium
    // 6. Verify the signature
    
    // Step 1: Kyber key exchange
    let (kyber_pk, kyber_sk) = pqc::generate_kyber_keypair().unwrap();
    let (ciphertext, shared_secret) = pqc::kyber_encapsulate(&kyber_pk).unwrap();
    let decrypted_secret = pqc::kyber_decapsulate(&ciphertext, &kyber_sk).unwrap();
    assert_eq!(shared_secret, decrypted_secret);
    
    // Step 2: Derive AES key from shared secret
    let aes_key = if shared_secret.len() >= 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(&shared_secret[..32]);
        key
    } else {
        let mut key = [0u8; 32];
        key[..shared_secret.len()].copy_from_slice(&shared_secret);
        key
    };
    
    // Step 3: Encrypt a message
    let message = b"Synapsis PQC integration test message";
    let encrypted_message = pqc::encrypt(message, &aes_key).unwrap();
    
    // Step 4: Generate Dilithium keypair and sign the encrypted message
    let (dilithium_pk, dilithium_sk) = pqc::generate_dilithium_keypair().unwrap();
    let signature = pqc::dilithium_sign(&dilithium_sk, &encrypted_message).unwrap();
    
    // Step 5: Verify signature
    let verified = pqc::dilithium_verify(&dilithium_pk, &encrypted_message, &signature);
    assert!(verified, "Signature should verify");
    
    // Step 6: Decrypt message (full cycle)
    let decrypted_message = pqc::decrypt(&encrypted_message, &aes_key).unwrap();
    assert_eq!(message, &decrypted_message[..]);
    
    println!("[TEST] PQC comprehensive test passed - Kyber + Dilithium + AES-256-GCM");
}