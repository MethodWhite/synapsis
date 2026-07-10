//! x402 Discovery — /.well-known/x402 endpoint for payment discovery
//! Implements the x402 protocol standard

pub fn generate_x402_response(engine: &crate::core::x402::X402Engine) -> String {
    let discovery = engine.get_x402_discovery();
    serde_json::to_string_pretty(&discovery).unwrap_or_default()
}
