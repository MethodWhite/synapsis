//! x402 Payment Required — Pay-per-use premium features
//! Based on HTTP 402 + USDC payments on Base (Coinbase L2)

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// A premium feature available behind x402 payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PremiumFeature {
    pub name: String,
    pub description: String,
    pub price_usdc: f64, // Price in USDC per call
    pub category: FeatureCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FeatureCategory {
    AiAnalysis,
    WebSearch,
    CryptoData,
    FinancialData,
    Security,
    Storage,
}

/// All premium features available via x402
pub fn all_premium_features() -> Vec<PremiumFeature> {
    vec![
        PremiumFeature {
            name: "pqc-encrypt".into(),
            description: "Post-quantum encryption (Kyber-768)".into(),
            price_usdc: 0.001,
            category: FeatureCategory::Security,
        },
        PremiumFeature {
            name: "session-bridge-multi".into(),
            description: "Multi-host session bridge (beyond local network)".into(),
            price_usdc: 0.01,
            category: FeatureCategory::Storage,
        },
        PremiumFeature {
            name: "web-search".into(),
            description: "Web search via external API".into(),
            price_usdc: 0.005,
            category: FeatureCategory::WebSearch,
        },
        PremiumFeature {
            name: "ai-analysis".into(),
            description: "AI-powered content analysis".into(),
            price_usdc: 0.01,
            category: FeatureCategory::AiAnalysis,
        },
        PremiumFeature {
            name: "crypto-data".into(),
            description: "Real-time cryptocurrency market data".into(),
            price_usdc: 0.005,
            category: FeatureCategory::CryptoData,
        },
        PremiumFeature {
            name: "sec-filings".into(),
            description: "SEC EDGAR filing search and retrieval".into(),
            price_usdc: 0.01,
            category: FeatureCategory::FinancialData,
        },
        PremiumFeature {
            name: "news-sentiment".into(),
            description: "News aggregation with sentiment analysis".into(),
            price_usdc: 0.005,
            category: FeatureCategory::AiAnalysis,
        },
        PremiumFeature {
            name: "company-intel".into(),
            description: "Company intelligence and competitor analysis".into(),
            price_usdc: 0.01,
            category: FeatureCategory::FinancialData,
        },
    ]
}

/// A record of a verified payment
#[derive(Debug, Clone)]
pub struct PaymentRecord {
    pub tx_hash: String,
    pub feature: String,
    pub amount_usdc: f64,
    pub payer_wallet: String,
    pub verified_at: i64,
    pub expires_at: i64, // Payment valid for 24h
}

pub struct X402Engine {
    /// Our receiving wallet address on Base
    pub receiver_wallet: String,
    /// Verified payments cache
    verified_payments: Mutex<Vec<PaymentRecord>>,
    /// Base RPC endpoint
    rpc_url: String,
}

impl X402Engine {
    pub fn new(receiver_wallet: &str, rpc_url: &str) -> Self {
        Self {
            receiver_wallet: receiver_wallet.to_string(),
            verified_payments: Mutex::new(Vec::new()),
            rpc_url: rpc_url.to_string(),
        }
    }

    /// Generate x402 payment requirements (/.well-known/x402)
    pub fn get_x402_discovery(&self) -> serde_json::Value {
        serde_json::json!({
            "protocol": "x402",
            "version": "0.1.0",
            "network": "base",
            "currency": "USDC",
            "receiver": self.receiver_wallet,
            "features": all_premium_features().iter().map(|f| {
                serde_json::json!({
                    "name": f.name,
                    "description": f.description,
                    "price_usdc": f.price_usdc,
                    "category": format!("{:?}", f.category),
                })
            }).collect::<Vec<_>>(),
        })
    }

    /// Verify a USDC transfer on-chain
    pub async fn verify_payment(&self, tx_hash: &str, feature: &str) -> Result<bool, String> {
        // Check cache first
        {
            let cached = self.verified_payments.lock().unwrap();
            if let Some(p) = cached.iter().find(|p| p.tx_hash == tx_hash) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                if p.expires_at > now {
                    return Ok(true);
                }
            }
        }

        // Verify via Base RPC
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1,
        });

        let resp = client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("RPC call failed: {}", e))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("RPC response: {}", e))?;

        // Check if transaction was to our wallet with USDC transfer
        // For now, accept any confirmed tx (full verification later)
        if let Some(result) = resp.get("result") {
            if !result.is_null() {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let record = PaymentRecord {
                    tx_hash: tx_hash.to_string(),
                    feature: feature.to_string(),
                    amount_usdc: 0.0, // Parse from logs in production
                    payer_wallet: "pending".into(),
                    verified_at: now,
                    expires_at: now + 86400, // 24h
                };
                self.verified_payments.lock().unwrap().push(record);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check if a feature is already paid for
    pub fn is_feature_paid(&self, feature: &str) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cached = self.verified_payments.lock().unwrap();
        cached
            .iter()
            .any(|p| p.feature == feature && p.expires_at > now)
    }

    /// Get premium features that require x402
    /// Filter out features already covered by a valid license
    pub fn get_unpaid_premium_features(&self, license_features: &[String]) -> Vec<PremiumFeature> {
        all_premium_features()
            .into_iter()
            .filter(|f| !license_features.contains(&f.name))
            .collect()
    }
}
