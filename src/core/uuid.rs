//! Custom UUID v4 implementation for supply chain control

use crate::core::security::SecureRng;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Default)]
pub struct Uuid(pub [u8; 16]);

impl Uuid {
    pub fn new_v4() -> Self {
        let mut bytes = [0u8; 16];
        SecureRng::fill_random(&mut bytes);

        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;

        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn to_hex_string(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut result = String::with_capacity(36);

        for (i, &byte) in self.0.iter().enumerate() {
            if i == 8 || i == 13 || i == 18 || i == 23 {
                result.push('-');
            }
            result.push(HEX[(byte >> 4) as usize] as char);
            result.push(HEX[(byte & 0x0f) as usize] as char);
        }

        result
    }
}

