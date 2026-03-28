//! Synapsis PQC Security Tests
//! 
//! Tests for CRYSTALS-Kyber implementation
//! Run with: cargo test --test pqc_security_tests

#[cfg(test)]
mod tests {
    // Import pqcrypto-kyber
    use pqcrypto_kyber::kyber512;
    // Import traits for as_bytes methods
    use pqcrypto_traits::kem::{PublicKey, SecretKey, Ciphertext, SharedSecret};

    // ========================================================================
    // Kyber512 Core Tests
    // ========================================================================

    #[test]
    fn test_kyber512_keypair_generation() {
        // Generate Kyber512 keypair
        let (pk, sk) = kyber512::keypair();
        
        // Verify key sizes (Kyber512: pk=800 bytes, sk=1632 bytes)
        assert_eq!(pk.as_bytes().len(), 800, "Kyber512 PK should be 800 bytes");
        assert_eq!(sk.as_bytes().len(), 1632, "Kyber512 SK should be 1632 bytes");
    }

    #[test]
    fn test_kyber512_encapsulate_decapsulate() {
        // Generate keypair
        let (pk, sk) = kyber512::keypair();
        
        // Encapsulate: returns (shared_secret, ciphertext)
        let (ss1, ct) = kyber512::encapsulate(&pk);
        
        // Verify ciphertext size (Kyber512: 768 bytes)
        assert_eq!(ct.as_bytes().len(), 768, "Kyber512 ciphertext should be 768 bytes");
        
        // Decapsulate
        let ss2 = kyber512::decapsulate(&ct, &sk);
        
        // Verify shared secrets match
        assert_eq!(ss1.as_bytes(), ss2.as_bytes(), 
                   "Decapsulated shared secret should match");
        
        // Verify shared secret size (32 bytes)
        assert_eq!(ss1.as_bytes().len(), 32, "Shared secret should be 32 bytes");
    }

    #[test]
    fn test_kyber512_shared_secret_uniqueness() {
        // Generate keypair
        let (pk, _) = kyber512::keypair();
        
        // Encapsulate twice
        let (_, ct1) = kyber512::encapsulate(&pk);
        let (_, ct2) = kyber512::encapsulate(&pk);
        
        // Ciphertexts should be different (random)
        assert_ne!(ct1.as_bytes(), ct2.as_bytes(), 
                   "Each encapsulation should produce different ciphertext");
    }

    #[test]
    fn test_kyber512_multiple_rounds() {
        // Run multiple keygen/encap/decap rounds
        for i in 0..10 {
            let (pk, sk) = kyber512::keypair();
            let (ss1, ct) = kyber512::encapsulate(&pk);
            let ss2 = kyber512::decapsulate(&ct, &sk);
            
            assert_eq!(ss1.as_bytes(), ss2.as_bytes(), "Round {} failed", i);
        }
    }

    #[test]
    fn test_kyber512_key_sizes_comprehensive() {
        let (pk, sk) = kyber512::keypair();
        let (ss, ct) = kyber512::encapsulate(&pk);
        
        // Verify all sizes
        assert_eq!(pk.as_bytes().len(), 800, "PK size");
        assert_eq!(sk.as_bytes().len(), 1632, "SK size");
        assert_eq!(ct.as_bytes().len(), 768, "Ciphertext size");
        assert_eq!(ss.as_bytes().len(), 32, "Shared secret size");
    }

    #[test]
    fn test_kyber512_decapsulation_correctness() {
        // Generate keypair
        let (pk, sk) = kyber512::keypair();
        
        // Encapsulate
        let (ss_original, ct) = kyber512::encapsulate(&pk);
        
        // Decapsulate
        let ss_recovered = kyber512::decapsulate(&ct, &sk);
        
        // Should always match
        assert_eq!(ss_original.as_bytes(), ss_recovered.as_bytes());
    }

    #[test]
    fn test_kyber512_performance_basic() {
        use std::time::Instant;
        
        let start = Instant::now();
        
        // Generate 10 keypairs and encapsulate/decapsulate
        for _ in 0..10 {
            let (pk, sk) = kyber512::keypair();
            let (ss, ct) = kyber512::encapsulate(&pk);
            let ss2 = kyber512::decapsulate(&ct, &sk);
            assert_eq!(ss.as_bytes(), ss2.as_bytes());
        }
        
        let duration = start.elapsed();
        
        // Should complete in reasonable time (< 5 seconds for 10 rounds)
        assert!(duration.as_secs() < 5, "Kyber operations too slow: {:?}", duration);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_full_kyber_workflow() {
        // 1. Generate keypair
        let (pk, sk) = kyber512::keypair();
        
        // 2. Encapsulate: returns (shared_secret, ciphertext)
        let (ss1, ct) = kyber512::encapsulate(&pk);
        
        // 3. Decapsulate
        let ss2 = kyber512::decapsulate(&ct, &sk);
        
        // 4. Verify shared secrets match
        assert_eq!(ss1.as_bytes(), ss2.as_bytes(), "Shared secrets should match");
    }

    #[test]
    fn test_kyber512_batch_operations() {
        // Generate multiple keypairs
        let keys: Vec<_> = (0..5).map(|_| kyber512::keypair()).collect();
        
        // Test each keypair
        for (i, (pk, sk)) in keys.iter().enumerate() {
            let (ss1, ct) = kyber512::encapsulate(pk);
            let ss2 = kyber512::decapsulate(&ct, sk);
            
            assert_eq!(ss1.as_bytes(), ss2.as_bytes(), 
                       "Batch key {} failed", i);
        }
    }
}
