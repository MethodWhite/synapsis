//! Premium Features Gate
//! Checks: 1) License covers feature? -> allow
//!         2) x402 payment received? -> allow
//!         3) Otherwise -> return 402 Payment Required

use crate::core::license;
use crate::core::x402;

/// Check if a premium feature can be used
/// Returns Ok(()) if allowed, Err with payment info if not
pub fn check_premium_access(feature: &str) -> Result<(), PremiumPaymentRequired> {
    // 1. Check license -- free if licensed
    if let Some(lic) = license::load_license()
        && lic.data.features.iter().any(|f| f == feature)
    {
        return Ok(());
    }

    // 2. Check if feature is premium
    let premium_features = x402::all_premium_features();
    if let Some(f) = premium_features.iter().find(|f| f.name == feature) {
        return Err(PremiumPaymentRequired {
            feature: feature.to_string(),
            price_usdc: f.price_usdc,
            message: format!(
                "{} requires payment of ${:.3} USDC",
                f.description, f.price_usdc
            ),
            payment_url: format!("/.well-known/x402?feature={}", feature),
        });
    }

    // Not a premium feature -- allow
    Ok(())
}

/// Return a payment error JSON object for an MCP response
pub fn payment_error(
    id: &serde_json::Value,
    payment: &PremiumPaymentRequired,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": payment.message,
            "data": {
                "feature": payment.feature,
                "price_usdc": payment.price_usdc,
                "payment_url": payment.payment_url,
                "x402": true
            }
        }
    })
}

/// Get premium status: license info + all premium features + availability
pub fn premium_status() -> serde_json::Value {
    let license_info = match license::load_license() {
        Some(lic) => serde_json::json!({
            "status": "active",
            "customer": lic.data.customer,
            "license_type": lic.data.license_type,
            "expires_at": lic.data.expires_at,
            "features": lic.data.features,
        }),
        None => serde_json::json!({
            "status": "none",
            "message": license::current_license_status(),
        }),
    };

    let licensed_features: Vec<String> = match license::load_license() {
        Some(ref lic) => lic.data.features.clone(),
        None => vec![],
    };

    let features: Vec<serde_json::Value> = x402::all_premium_features()
        .into_iter()
        .map(|f| {
            let available = licensed_features.contains(&f.name);
            serde_json::json!({
                "name": f.name,
                "description": f.description,
                "price_usdc": f.price_usdc,
                "category": format!("{:?}", f.category),
                "available": available,
                "payment_required": !available,
                "payment_url": format!("/.well-known/x402?feature={}", f.name),
            })
        })
        .collect();

    serde_json::json!({
        "license": license_info,
        "premium_features": features,
    })
}

pub struct PremiumPaymentRequired {
    pub feature: String,
    pub price_usdc: f64,
    pub message: String,
    pub payment_url: String,
}
