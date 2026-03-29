//! CRYSTALS-Dilithium Integration Tests
//!
//! Tests for post-quantum digital signatures

#[cfg(test)]
mod tests {
    use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};
    use synapsis::dilithium::{
        sign_message, sign_verify_roundtrip, verify_signature, DilithiumKeypair,
    };

    #[test]
    fn test_dilithium2_full_integration() {
        let keypair = DilithiumKeypair::generate();

        assert_eq!(keypair.public_key.as_bytes().len(), 1312);
        assert_eq!(keypair.secret_key.as_bytes().len(), 2560);

        let message = b"Integration test message";
        let signature = sign_message(&keypair.secret_key, message);

        let opened = verify_signature(&signature, &keypair.public_key).unwrap();
        assert_eq!(opened, message);
    }

    #[test]
    fn test_dilithium2_authenticity() {
        let message = b"Authentic message";
        let keypair = DilithiumKeypair::generate();

        let signature = sign_message(&keypair.secret_key, message);
        let opened = verify_signature(&signature, &keypair.public_key).unwrap();
        assert_eq!(opened, message);
    }

    #[test]
    fn test_dilithium2_unforgeability() {
        let message = b"Test message";
        let keypair1 = DilithiumKeypair::generate();
        let keypair2 = DilithiumKeypair::generate();

        let signature = sign_message(&keypair1.secret_key, message);
        let result = verify_signature(&signature, &keypair2.public_key);
        assert!(
            result.is_err(),
            "Signature from different key should not verify"
        );
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
            let signature = sign_message(&keypair.secret_key, message);
            let opened = verify_signature(&signature, &keypair.public_key).unwrap();
            assert_eq!(opened, message);
        }

        let duration = start.elapsed();
        assert!(duration.as_secs() < 10, "Too slow: {:?}", duration);

        println!("Dilithium2: {} iterations in {:?}", iterations, duration);
    }

    #[test]
    fn test_dilithium2_keypair_uniqueness() {
        let keypair1 = DilithiumKeypair::generate();
        let keypair2 = DilithiumKeypair::generate();

        assert_ne!(
            keypair1.public_key.as_bytes(),
            keypair2.public_key.as_bytes()
        );
        assert_ne!(
            keypair1.secret_key.as_bytes(),
            keypair2.secret_key.as_bytes()
        );
    }

    #[test]
    fn test_dilithium2_signature_uniqueness() {
        let message = b"Deterministic test";
        let keypair = DilithiumKeypair::generate();

        let sig1 = sign_message(&keypair.secret_key, message);
        let sig2 = sign_message(&keypair.secret_key, message);

        assert_eq!(
            sig1.as_bytes(),
            sig2.as_bytes(),
            "Signatures should be deterministic"
        );
    }

    #[test]
    fn test_dilithium2_message_variations() {
        let test_cases: Vec<&[u8]> = vec![
            b"Short",
            b"Medium length message for testing",
            b"This is a longer message with more content to sign and verify.",
            &[0u8; 100],
            &[0xFFu8; 100],
            b"Special chars: !@#$%^&*()_+-=[]{}|;':",
        ];

        for (i, message) in test_cases.iter().enumerate() {
            let valid = sign_verify_roundtrip(message).unwrap();
            assert!(valid, "Test case {} should succeed", i);
        }
    }
}
