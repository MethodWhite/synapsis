//! Security classification tool

use serde_json::json;

/// Security classification levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    Public,
    Internal,
    Confidential,
    Secret,
    TopSecret,
}

impl SecurityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            SecurityLevel::Public => "PUBLIC",
            SecurityLevel::Internal => "INTERNAL",
            SecurityLevel::Confidential => "CONFIDENTIAL",
            SecurityLevel::Secret => "SECRET",
            SecurityLevel::TopSecret => "TOP_SECRET",
        }
    }

    pub fn numeric(&self) -> u8 {
        match self {
            SecurityLevel::Public => 0,
            SecurityLevel::Internal => 1,
            SecurityLevel::Confidential => 2,
            SecurityLevel::Secret => 3,
            SecurityLevel::TopSecret => 4,
        }
    }
}

/// Classify text based on security keywords
pub fn classify_security(text: &str, context: &str) -> (SecurityLevel, Vec<String>) {
    let mut indicators = Vec::new();
    let text_lower = text.to_lowercase();

    // Keyword patterns for different security levels
    let top_secret_keywords = [
        "top secret",
        "ts//sci",
        "codeword",
        "compartment",
        "nsa",
        "cia",
        "mi6",
        "classified",
        "secret clearance",
        "sci",
        "si",
        "tk",
        "hcs",
        "//",
    ];

    let secret_keywords = [
        "secret",
        "confidential",
        "restricted",
        "for official use only",
        "fouo",
        "not for distribution",
        "eyes only",
        "need-to-know",
        "clearance",
    ];

    let confidential_keywords = [
        "internal use only",
        "proprietary",
        "company confidential",
        "trade secret",
        "ndas",
        "non-disclosure",
        "customer data",
        "personally identifiable",
        "pii",
        "gdpr",
        "hipaa",
        "compliance",
    ];

    let internal_keywords = [
        "internal",
        "staff only",
        "employee",
        "hr",
        "finance",
        "payroll",
        "roadmap",
        "strategy",
        "budget",
        "meeting notes",
    ];

    // Check for top secret indicators
    for kw in &top_secret_keywords {
        if text_lower.contains(kw) {
            indicators.push(format!("top_secret_keyword: {}", kw));
        }
    }

    // Check for secret indicators
    for kw in &secret_keywords {
        if text_lower.contains(kw) {
            indicators.push(format!("secret_keyword: {}", kw));
        }
    }

    // Check for confidential indicators
    for kw in &confidential_keywords {
        if text_lower.contains(kw) {
            indicators.push(format!("confidential_keyword: {}", kw));
        }
    }

    // Check for internal indicators
    for kw in &internal_keywords {
        if text_lower.contains(kw) {
            indicators.push(format!("internal_keyword: {}", kw));
        }
    }

    // Additional context-based classification
    let context_adjustment = match context.to_lowercase().as_str() {
        "government" | "defense" | "intelligence" => 1,
        "healthcare" | "finance" | "legal" => 1,
        "social" | "public" => -1,
        _ => 0,
    };

    // Determine security level based on indicators
    let base_level = if indicators
        .iter()
        .any(|i| i.starts_with("top_secret_keyword:"))
    {
        SecurityLevel::TopSecret
    } else if indicators.iter().any(|i| i.starts_with("secret_keyword:")) {
        SecurityLevel::Secret
    } else if indicators
        .iter()
        .any(|i| i.starts_with("confidential_keyword:"))
    {
        SecurityLevel::Confidential
    } else if indicators
        .iter()
        .any(|i| i.starts_with("internal_keyword:"))
    {
        SecurityLevel::Internal
    } else {
        SecurityLevel::Public
    };

    // Apply context adjustment (simple)
    let adjusted_level = match (base_level.numeric() as i8 + context_adjustment) as u8 {
        0 => SecurityLevel::Public,
        1 => SecurityLevel::Internal,
        2 => SecurityLevel::Confidential,
        3 => SecurityLevel::Secret,
        _ => SecurityLevel::TopSecret,
    };

    (adjusted_level, indicators)
}

/// MCP tools handler
pub mod mcp_tools {
    use super::*;

    pub fn handle_security_classify(text: &str, context: &str) -> serde_json::Value {
        let (level, indicators) = classify_security(text, context);

        json!({
            "status": "ok",
            "security_level": level.as_str(),
            "security_level_numeric": level.numeric(),
            "indicators_found": indicators.len(),
            "indicators": indicators,
            "text_length": text.len(),
            "context": context,
        })
    }
}
