use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SYNAPSIS_PUBKEY_HEX: Option<&str> = option_env!("SYNAPSIS_PUBKEY");

#[derive(Debug, Serialize, Deserialize)]
pub struct LicenseData {
    pub customer: String,
    pub features: Vec<String>,
    pub issued_at: String,
    pub expires_at: String,
    pub license_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedLicense {
    pub data: LicenseData,
    pub signature: String,
}

impl SignedLicense {
    pub fn verify(&self) -> Result<(), String> {
        let pubkey_hex = match SYNAPSIS_PUBKEY_HEX {
            Some(k) if !k.is_empty() && k != "SYNAPSIS_PUBKEY_PLACEHOLDER" => k,
            _ => return Ok(()),
        };
        let pubkey_bytes = hex::decode(pubkey_hex).map_err(|e| format!("Invalid pubkey: {}", e))?;
        let verifying_key =
            VerifyingKey::from_bytes(&pubkey_bytes.try_into().map_err(|_| "Invalid pubkey length")?)
                .map_err(|e| format!("Invalid pubkey: {}", e))?;

        let data_json =
            serde_json::to_string(&self.data).map_err(|e| format!("Serialization: {}", e))?;
        let sig_bytes = hex::decode(&self.signature)
            .map_err(|e| format!("Invalid signature hex: {}", e))?;
        let signature =
            Signature::from_slice(&sig_bytes).map_err(|e| format!("Invalid signature: {}", e))?;

        verifying_key
            .verify(data_json.as_bytes(), &signature)
            .map_err(|_| "License signature verification failed".to_string())?;

        let expires = chrono::DateTime::parse_from_rfc3339(&self.data.expires_at)
            .map_err(|_| "Invalid expiry date".to_string())?;
        if chrono::Utc::now() > expires {
            return Err("License expired".to_string());
        }

        Ok(())
    }
}

pub fn load_license() -> Option<SignedLicense> {
    let paths = [
        dirs::home_dir().map(|h| h.join(".synapsis-license")),
        Some(PathBuf::from(".synapsis-license")),
        std::env::var("SYNAPSIS_LICENSE").ok().map(PathBuf::from),
    ];
    for path in paths.into_iter().flatten() {
        if path.exists() {
            let content = std::fs::read_to_string(&path).ok()?;
            let license: SignedLicense = serde_json::from_str(&content).ok()?;
            if license.verify().is_ok() {
                return Some(license);
            }
        }
    }
    None
}

pub fn current_license_status() -> String {
    match load_license() {
        Some(lic) => format!(
            "License: {} | Customer: {} | Type: {} | Expires: {} | Features: {}",
            "ACTIVE",
            lic.data.customer,
            lic.data.license_type,
            lic.data.expires_at,
            lic.data.features.join(", ")
        ),
        None => match SYNAPSIS_PUBKEY_HEX {
            Some(k) if !k.is_empty() && k != "SYNAPSIS_PUBKEY_PLACEHOLDER" => {
                    "License: NOT FOUND | Synapsis free for individuals & SMEs (<$500k revenue). Commercial license required for enterprise. Contact: methodwhite@proton.me".to_string()
            }
            _ => "License: DEVELOPMENT MODE (no check)".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_status_does_not_panic() {
        let status = current_license_status();
        assert!(!status.is_empty());
    }
}
