//! Integrity verification using HMAC-SHA3-512 and Merkle Trees

use hmac::{Hmac, Mac};
use sha3::{Sha3_512, Digest};
use std::collections::HashMap;

type HmacSha512 = Hmac<Sha3_512>;

/// Compute HMAC-SHA3-512 of data with given key
pub fn hmac_sha3_512(key: &[u8], data: &[u8]) -> Result<[u8; 64], String> {
    let mut mac = HmacSha512::new_from_slice(key)
        .map_err(|e| format!("Invalid key length: {}", e))?;
    mac.update(data);
    let result = mac.finalize();
    let mut output = [0u8; 64];
    output.copy_from_slice(&result.into_bytes());
    Ok(output)
}

/// Verify HMAC-SHA3-512 tag
pub fn verify_hmac_sha3_512(key: &[u8], data: &[u8], tag: &[u8]) -> bool {
    if tag.len() != 64 {
        return false;
    }
    match hmac_sha3_512(key, data) {
        Ok(computed) => computed.as_slice() == tag,
        Err(_) => false,
    }
}

/// Simple Merkle tree for integrity verification
pub struct MerkleTree {
    leaves: Vec<[u8; 64]>, // SHA3-512 hashes
    root: Option<[u8; 64]>,
    levels: Vec<Vec<[u8; 64]>>,
}

impl MerkleTree {
    /// Create a new Merkle tree from leaf data
    pub fn new(leaves: Vec<Vec<u8>>) -> Self {
        if leaves.is_empty() {
            return Self {
                leaves: Vec::new(),
                root: None,
                levels: Vec::new(),
            };
        }
        
        // Hash each leaf with SHA3-512
        let leaf_hashes: Vec<[u8; 64]> = leaves.iter()
            .map(|data| {
                let mut hasher = Sha3_512::new();
                hasher.update(data);
                let result = hasher.finalize();
                let mut hash = [0u8; 64];
                hash.copy_from_slice(&result);
                hash
            })
            .collect();
        
        let mut levels = Vec::new();
        levels.push(leaf_hashes.clone());
        
        // Build tree levels
        let mut current_level = levels[0].clone();
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for i in (0..current_level.len()).step_by(2) {
                if i + 1 < current_level.len() {
                    let combined = [&current_level[i][..], &current_level[i + 1][..]].concat();
                    let mut hasher = Sha3_512::new();
                    hasher.update(&combined);
                    let result = hasher.finalize();
                    let mut hash = [0u8; 64];
                    hash.copy_from_slice(&result);
                    next_level.push(hash);
                } else {
                    // Duplicate last element if odd number
                    next_level.push(current_level[i]);
                }
            }
            levels.push(next_level.clone());
            current_level = next_level;
        }
        
        let root = if current_level.is_empty() {
            None
        } else {
            Some(current_level[0])
        };
        
        Self {
            leaves: leaf_hashes,
            root,
            levels,
        }
    }
    
    /// Get the root hash
    pub fn root(&self) -> Option<&[u8; 64]> {
        self.root.as_ref()
    }
    
    /// Generate a Merkle proof for a leaf at given index
    pub fn proof(&self, index: usize) -> Option<Vec<(bool, [u8; 64])>> {
        if index >= self.leaves.len() {
            return None;
        }
        
        let mut proof = Vec::new();
        let mut idx = index;
        
        for level in 0..self.levels.len() - 1 {
            let level_len = self.levels[level].len();
            if idx.is_multiple_of(2) {
                // Need right sibling
                if idx + 1 < level_len {
                    proof.push((true, self.levels[level][idx + 1]));
                } else {
                    // No sibling (odd number of nodes), duplicate self
                    proof.push((true, self.levels[level][idx]));
                }
            } else {
                // Need left sibling
                proof.push((false, self.levels[level][idx - 1]));
            }
            idx /= 2;
        }
        
        Some(proof)
    }
    
    /// Verify a Merkle proof given leaf index and total leaves
    pub fn verify_proof_with_index(leaf: &[u8], index: usize, total_leaves: usize, proof: &[(bool, [u8; 64])], root: &[u8; 64]) -> bool {
        if index >= total_leaves {
            return false;
        }
        
        let mut hasher = Sha3_512::new();
        hasher.update(leaf);
        let leaf_hash = hasher.finalize();
        let mut current = [0u8; 64];
        current.copy_from_slice(&leaf_hash);
        for (is_right_sibling, sibling) in proof {
            let combined = if *is_right_sibling {
                // Current is left, sibling is right
                [&current[..], &sibling[..]].concat()
            } else {
                // Current is right, sibling is left
                [&sibling[..], &current[..]].concat()
            };
            let mut hasher = Sha3_512::new();
            hasher.update(&combined);
            let result = hasher.finalize();
            current.copy_from_slice(&result);

        }
        
        &current == root
    }
    
    /// Verify a Merkle proof (simplified, assumes proof includes position info)
    pub fn verify_proof(leaf: &[u8], proof: &[(bool, [u8; 64])], root: &[u8; 64]) -> bool {
        let mut hasher = Sha3_512::new();
        hasher.update(leaf);
        let leaf_hash = hasher.finalize();
        let mut current = [0u8; 64];
        current.copy_from_slice(&leaf_hash);
        
        for (is_right_sibling, sibling) in proof {
            let combined = if *is_right_sibling {
                // Current is left, sibling is right
                [&current[..], &sibling[..]].concat()
            } else {
                // Current is right, sibling is left
                [&sibling[..], &current[..]].concat()
            };
            let mut hasher = Sha3_512::new();
            hasher.update(&combined);
            let result = hasher.finalize();
            current.copy_from_slice(&result);
        }
        
        &current == root
    }
}

/// ChaCha20-Poly1305 encryption wrapper
pub fn chacha20_poly1305_encrypt(key: &[u8; 32], nonce: &[u8; 12], plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, String> {
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit, AeadInPlace};
    
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| format!("Invalid key: {}", e))?;
    
    let mut buffer = plaintext.to_vec();
    let tag = cipher.encrypt_in_place_detached(nonce.into(), associated_data, &mut buffer)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    buffer.extend_from_slice(&tag);
    Ok(buffer)
}

/// ChaCha20-Poly1305 decryption wrapper
pub fn chacha20_poly1305_decrypt(key: &[u8; 32], nonce: &[u8; 12], ciphertext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, String> {
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit, AeadInPlace};
    
    if ciphertext.len() < 16 {
        return Err("Ciphertext too short".to_string());
    }
    
    let cipher = ChaCha20Poly1305::new_from_slice(key)
        .map_err(|e| format!("Invalid key: {}", e))?;
    
    let mut buffer = ciphertext[..ciphertext.len() - 16].to_vec();
    let tag = &ciphertext[ciphertext.len() - 16..];
    
    cipher.decrypt_in_place_detached(nonce.into(), associated_data, &mut buffer, tag.into())
        .map_err(|e| format!("Decryption failed: {}", e))?;
    
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    
    #[test]
    fn test_hmac_sha3_512() {
        let key = b"test-key";
        let data = b"Hello, world!";
        
        let tag = hmac_sha3_512(key, data).unwrap();
        assert_eq!(tag.len(), 64);
        
        let verified = verify_hmac_sha3_512(key, data, &tag);
        assert!(verified, "HMAC verification failed");
        
        // Tampered data should fail
        let tampered = verify_hmac_sha3_512(key, b"Hello, world?", &tag);
        assert!(!tampered, "HMAC should not verify tampered data");
        
        // Wrong key should fail
        let wrong_key = verify_hmac_sha3_512(b"wrong-key", data, &tag);
        assert!(!wrong_key, "HMAC should not verify with wrong key");
    }
    
    #[test]
    fn test_merkle_tree() {
        let leaves = vec![
            b"leaf1".to_vec(),
            b"leaf2".to_vec(),
            b"leaf3".to_vec(),
            b"leaf4".to_vec(),
        ];
        
        let tree = MerkleTree::new(leaves.clone());
        let root = tree.root().unwrap();
        
        // Verify each leaf
        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.proof(i).unwrap();
            let verified = MerkleTree::verify_proof(leaf, &proof, root);
            assert!(verified, "Merkle proof verification failed for leaf {}", i);
        }
        
        // Verify wrong leaf fails
        let wrong_leaf = b"wrong";
        let proof = tree.proof(0).unwrap();
        let verified = MerkleTree::verify_proof(wrong_leaf, &proof, root);
        assert!(!verified, "Merkle proof should not verify wrong leaf");
        
        // Verify with wrong root fails
        let mut wrong_root = *root;
        wrong_root[0] ^= 0xFF;
        let verified = MerkleTree::verify_proof(&leaves[0], &proof, &wrong_root);
        assert!(!verified, "Merkle proof should not verify with wrong root");
    }
    
    #[test]
    fn test_merkle_tree_single_leaf() {
        let leaves = vec![b"single leaf".to_vec()];
        let tree = MerkleTree::new(leaves.clone());
        assert!(tree.root().is_some());
        
        let root = tree.root().unwrap();
        let proof = tree.proof(0).unwrap();
        assert!(proof.is_empty()); // Single leaf has no proof
        
        let verified = MerkleTree::verify_proof(&leaves[0], &proof, root);
        assert!(verified, "Single leaf Merkle tree verification failed");
    }
    
    #[test]
    fn test_merkle_tree_empty() {
        let tree = MerkleTree::new(vec![]);
        assert!(tree.root().is_none());
        assert!(tree.proof(0).is_none());
    }
    
    #[test]
    fn test_chacha20_poly1305() {
        let mut rng = rand::thread_rng();
        let mut key = [0u8; 32];
        let mut nonce = [0u8; 12];
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut nonce);
        
        let plaintext = b"Secret message for ChaCha20-Poly1305 encryption";
        let associated_data = b"metadata";
        
        let ciphertext = chacha20_poly1305_encrypt(&key, &nonce, plaintext, associated_data).unwrap();
        assert!(ciphertext.len() > plaintext.len());
        
        let decrypted = chacha20_poly1305_decrypt(&key, &nonce, &ciphertext, associated_data).unwrap();
        assert_eq!(decrypted, plaintext);
        
        // Wrong key should fail
        let mut wrong_key = key;
        wrong_key[0] ^= 0xFF;
        let result = chacha20_poly1305_decrypt(&wrong_key, &nonce, &ciphertext, associated_data);
        assert!(result.is_err(), "Decryption should fail with wrong key");
        
        // Wrong nonce should fail
        let mut wrong_nonce = nonce;
        wrong_nonce[0] ^= 0xFF;
        let result = chacha20_poly1305_decrypt(&key, &wrong_nonce, &ciphertext, associated_data);
        assert!(result.is_err(), "Decryption should fail with wrong nonce");
        
        // Tampered ciphertext should fail
        let mut tampered = ciphertext.clone();
        if !tampered.is_empty() {
            tampered[0] ^= 0xFF;
        }
        let result = chacha20_poly1305_decrypt(&key, &nonce, &tampered, associated_data);
        assert!(result.is_err(), "Decryption should fail with tampered ciphertext");
        
        // Wrong associated data should fail
        let wrong_ad = b"wrong-metadata";
        let result = chacha20_poly1305_decrypt(&key, &nonce, &ciphertext, wrong_ad);
        assert!(result.is_err(), "Decryption should fail with wrong associated data");
    }
}