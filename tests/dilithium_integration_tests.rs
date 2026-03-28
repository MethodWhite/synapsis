//! CRYSTALS-Dilithium Integration Tests
//! 
//! Tests for post-quantum digital signatures

#[cfg(test)]
mod tests {
    use synapsis::dilithium::{
        DilithiumKeypair,
        sign_message,
        verify_signature,
        sign_verify_roundtrip,
    };

    #[test]
    fn test_dilithium2_full_integration() {
        // Generate keypair
        let keypair = DilithiumKeypair::generate();
        
        // Verify sizes
        assert_eq!(keypair.public_key.len(), 2420);
        assert_eq!(keypair.secret_key.len(), 4864);
        
        // Sign a message
        let message = b"Integration test message";
        let signature = sign_message(&keypair.secret_key, message).unwrap();
        
        // Verify signature size
        assert_eq!(signature.len(), 4595);
        
        // Verify signature
        let valid = verify_signature(&keypair.public_key, message, &signature).unwrap();
        assert!(valid);
    }

    #[test]
    fn test_dilithium2_authenticity() {
        let message = b"Authentic message";
        let keypair = DilithiumKeypair::generate();
        
        // Sign
        let signature = sign_message(&keypair.secret_key, message).unwrap();
        
        // Verify with correct key
        let valid = verify_signature(&keypair.public_key, message, &signature).unwrap();
        assert!(valid, "Valid signature should verify");
        
        // Tamper with message
        let tampered = b"Tampered message";
        let valid_tampered = verify_signature(&keypair.public_key, tampered, &signature).unwrap();
        assert!(!valid_tampered, "Tampered message should not verify");
    }

    #[test]
    fn test_dilithium2_unforgeability() {
        let message = b"Test message";
        let keypair1 = DilithiumKeypair::generate();
        let keypair2 = DilithiumKeypair::generate();
        
        // Sign with key1
        let signature = sign_message(&keypair1.secret_key, message).unwrap();
        
        // Try to verify with key2 (should fail)
        let valid = verify_signature(&keypair2.public_key, message, &signature).unwrap();
        assert!(!valid, "Signature from different key should not verify");
    }

    #[test]
    fn test_dilithium2_roundtrip_multiple() {
        for i in 0..10 {
            let message = format!("Roundtrip test {}", i);
            let valid = sign_verify_roundtrip(message.as_bytes()).unwrap();
            assert!(valid, "Roundtrip {} should succeed", i);
        }
    }

    #[test]
    fn test_dilithium2_large_message() {
        // Test with 1MB message
        let large_message = vec![0x42u8; 1024 * 1024];
        let valid = sign_verify_roundtrip(&large_message).unwrap();
        assert!(valid, "Large message should sign/verify");
    }

    #[test]
    fn test_dilithium2_empty_message() {
        let valid = sign_verify_roundtrip(b"").unwrap();
        assert!(valid, "Empty message should sign/verify");
    }

    #[test]
    fn test_dilithium2_binary_data() {
        // Test with binary data including null bytes
        let binary_data = vec![0x00u8, 0x01, 0x02, 0xFF, 0xFE, 0xFD];
        let valid = sign_verify_roundtrip(&binary_data).unwrap();
        assert!(valid, "Binary data should sign/verify");
    }

    #[test]
    fn test_dilithium2_performance() {
        use std::time::Instant;
        
        let message = b"Performance test message";
        let iterations = 100;
        
        let start = Instant::now();
        
        for _ in 0..iterations {
            let keypair = DilithiumKeypair::generate();
            let signature = sign_message(&keypair.secret_key, message).unwrap();
            let valid = verify_signature(&keypair.public_key, message, &signature).unwrap();
            assert!(valid);
        }
        
        let duration = start.elapsed();
        
        // Should complete 100 iterations in reasonable time (< 10 seconds)
        assert!(duration.as_secs() < 10, "Too slow: {:?}", duration);
        
        println!("Dilithium2: {} iterations in {:?}", iterations, duration);
    }

    #[test]
    fn test_dilithium2_keypair_uniqueness() {
        let keypair1 = DilithiumKeypair::generate();
        let keypair2 = DilithiumKeypair::generate();
        
        // Keys should be different
        assert_ne!(keypair1.public_key, keypair2.public_key);
        assert_ne!(keypair1.secret_key, keypair2.secret_key);
    }

    #[test]
    fn test_dilithium2_signature_uniqueness() {
        // Signatures should be deterministic for same message and key
        let message = b"Deterministic test";
        let keypair = DilithiumKeypair::generate();
        
        let sig1 = sign_message(&keypair.secret_key, message).unwrap();
        let sig2 = sign_message(&keypair.secret_key, message).unwrap();
        
        assert_eq!(sig1, sig2, "Signatures should be deterministic");
    }

    #[test]
    fn test_dilithium2_message_variations() {
        let test_cases = vec![
            b"Short",
            b"Medium length message for testing",
            b"This is a longer message with more content to sign and verify. It contains multiple words and should test the signature algorithm with a reasonable amount of data.",
            &[0u8; 100],  // 100 null bytes
            &[0xFFu8; 1000],  // 1000 0xFF bytes
            b"Special chars: !@#$%^&*()_+-=[]{}|;':\",./<>?",
        ];
        
        for (i, message) in test_cases.iter().enumerate() {
            let valid = sign_verify_roundtrip(message).unwrap();
            assert!(valid, "Test case {} should succeed", i);
        }
    }
}
