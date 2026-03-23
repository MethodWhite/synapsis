//! Synapsis Smart Categorizer
//!
//! Intelligently categorizes messages based on content analysis and rules.

use regex::Regex;
use serde::{Deserialize, Serialize};
// use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[derive(Default)]
pub enum MessageCategory {
    Critical,
    Sensitive,
    Important,
    #[default]
    Standard,
    Ephemeral,
}

impl Hash for MessageCategory {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self as i32).hash(state);
    }
}


impl MessageCategory {
    pub fn ttl_seconds(&self) -> Option<i64> {
        match self {
            MessageCategory::Critical => None,
            MessageCategory::Sensitive => Some(30 * 24 * 60 * 60),
            MessageCategory::Important => Some(90 * 24 * 60 * 60),
            MessageCategory::Standard => Some(30 * 24 * 60 * 60),
            MessageCategory::Ephemeral => Some(0),
        }
    }

    pub fn should_index(&self) -> bool {
        !matches!(self, MessageCategory::Ephemeral)
    }

    pub fn requires_extra_encryption(&self) -> bool {
        matches!(self, MessageCategory::Critical | MessageCategory::Sensitive)
    }

    pub fn should_never_delete(&self) -> bool {
        matches!(self, MessageCategory::Critical)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            MessageCategory::Critical => "critical",
            MessageCategory::Sensitive => "sensitive",
            MessageCategory::Important => "important",
            MessageCategory::Standard => "standard",
            MessageCategory::Ephemeral => "ephemeral",
        }
    }
}

impl std::fmt::Display for MessageCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct CategorizationRule {
    pub pattern: Regex,
    pub category: MessageCategory,
    pub priority: i32,
    pub description: String,
}

impl CategorizationRule {
    pub fn new(
        pattern: &str,
        category: MessageCategory,
        priority: i32,
        description: &str,
    ) -> Option<Self> {
        Regex::new(pattern).ok().map(|pattern| Self {
            pattern,
            category,
            priority,
            description: description.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorizationResult {
    pub category: MessageCategory,
    pub matched_rule: Option<String>,
    pub keywords: Vec<String>,
    pub confidence: f32,
    pub reasons: Vec<String>,
}

pub struct SmartCategorizer {
    rules: Vec<CategorizationRule>,
    custom_rules: Arc<RwLock<Vec<CategorizationRule>>>,
}

impl SmartCategorizer {
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
            custom_rules: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn default_rules() -> Vec<CategorizationRule> {
        let mut rules = Vec::new();

        if let Some(rule) = CategorizationRule::new(
            r"(?i)(anomal|cern|lhc|emp|biosphere|quantum|vacuum)",
            MessageCategory::Critical,
            100,
            "Critical topics",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r"(?i)(secret|api_key|password|credential|token|private.key)",
            MessageCategory::Sensitive,
            95,
            "Sensitive credentials",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r"(?i)(classified|confidential|restricted|top.secret)",
            MessageCategory::Sensitive,
            90,
            "Classified information",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r#""type"\s*:\s*"heartbeat""#,
            MessageCategory::Ephemeral,
            85,
            "Heartbeat messages",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r#""method"\s*:\s*"(ping|health.check|status)""#,
            MessageCategory::Ephemeral,
            80,
            "Health check messages",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r#""method"\s*:\s*"task_""#,
            MessageCategory::Important,
            75,
            "Task-related messages",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r#""method"\s*:\s*"(broadcast|message_send)""#,
            MessageCategory::Important,
            70,
            "Inter-agent communication",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r"(?i)(security|audit|firewall|intrusion|malware|virus|exploit)",
            MessageCategory::Important,
            60,
            "Security-related content",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r"(?i)(error|exception|fail|panic|crash|bug)",
            MessageCategory::Standard,
            50,
            "Error messages",
        ) {
            rules.push(rule);
        }

        if let Some(rule) = CategorizationRule::new(
            r"(?i)(debug|trace|verbose|log)",
            MessageCategory::Standard,
            40,
            "Debug messages",
        ) {
            rules.push(rule);
        }

        rules
    }

    pub fn add_rule(&self, rule: CategorizationRule) {
        let mut rules = self.custom_rules.write().unwrap();
        rules.push(rule);
    }

    pub fn add_custom_rule(
        &self,
        pattern: &str,
        category: MessageCategory,
        priority: i32,
        description: &str,
    ) -> Option<()> {
        let rule = CategorizationRule::new(pattern, category, priority, description)?;
        self.add_rule(rule);
        Some(())
    }

    pub fn remove_rule(&self, description: &str) -> bool {
        let mut rules = self.custom_rules.write().unwrap();
        let len_before = rules.len();
        rules.retain(|r| r.description != description);
        rules.len() != len_before
    }

    pub fn categorize(
        &self,
        content: &str,
        metadata: Option<&MessageMetadata>,
    ) -> CategorizationResult {
        let mut best_priority = -1;
        let mut matched_rule = None;
        let mut keywords = Vec::new();
        let mut reasons = Vec::new();

        let custom_rules: Vec<CategorizationRule> = {
            let guard = self.custom_rules.read().unwrap();
            guard.iter().cloned().collect()
        };
        let all_rules: Vec<&CategorizationRule> =
            self.rules.iter().chain(custom_rules.iter()).collect();

        for rule in all_rules {
            if let Some(captures) = rule.pattern.captures(content) {
                if rule.priority > best_priority {
                    best_priority = rule.priority;
                    matched_rule = Some(rule.description.clone());

                    for name in rule.pattern.capture_names() {
                        if let Some(m) = name.and_then(|n| captures.name(n)) {
                            keywords.push(m.as_str().to_lowercase());
                        }
                    }

                    if keywords.is_empty() {
                        keywords.push(rule.description.to_lowercase());
                    }

                    reasons.push(format!("Matched: {}", rule.description));
                }
            }
        }

        if let Some(meta) = metadata {
            if meta.is_encrypted && best_priority < 80 {
                reasons.push("Encrypted content".to_string());
                if best_priority < 50 {
                    best_priority = 50;
                }
            }

            if let Some(ref task_id) = meta.task_id {
                keywords.push(format!("task:{}", task_id));
            }

            if let Some(ref agent_type) = meta.agent_type {
                keywords.push(format!("agent:{}", agent_type));
            }
        }

        if best_priority < 0 {
            reasons.push("No rules matched (default to Standard)".to_string());
        }

        let category = if best_priority >= 100 {
            MessageCategory::Critical
        } else if best_priority >= 85 {
            MessageCategory::Sensitive
        } else if best_priority >= 70 {
            MessageCategory::Important
        } else if best_priority >= 40 {
            MessageCategory::Standard
        } else {
            MessageCategory::Ephemeral
        };

        let confidence = if best_priority >= 100 {
            1.0
        } else if best_priority >= 80 {
            0.95
        } else if best_priority >= 60 {
            0.8
        } else if best_priority >= 40 {
            0.6
        } else {
            0.3
        };

        CategorizationResult {
            category,
            matched_rule,
            keywords,
            confidence,
            reasons,
        }
    }

    pub fn get_rules(&self) -> Vec<(String, MessageCategory, i32)> {
        let custom = self.custom_rules.read().unwrap();
        let mut rules: Vec<(String, MessageCategory, i32)> = self
            .rules
            .iter()
            .map(|r| (r.description.clone(), r.category, r.priority))
            .chain(
                custom
                    .iter()
                    .map(|r| (r.description.clone(), r.category, r.priority)),
            )
            .collect();

        rules.sort_by(|a, b| b.2.cmp(&a.2));
        rules
    }
}

impl Default for SmartCategorizer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct MessageMetadata {
    pub task_id: Option<String>,
    pub agent_type: Option<String>,
    pub session_id: Option<String>,
    pub project: Option<String>,
    pub method: Option<String>,
    pub is_encrypted: bool,
    pub is_broadcast: bool,
}


impl MessageMetadata {
    pub fn from_jsonrpc(content: &str) -> Option<Self> {
        let json: serde_json::Value = serde_json::from_str(content).ok()?;

        let method = json
            .get("method")
            .and_then(|m| m.as_str())
            .map(String::from);

        let params = json.get("params");

        let task_id = params
            .and_then(|p| p.get("task_id"))
            .and_then(|t| t.as_str())
            .map(String::from);

        let session_id = params
            .and_then(|p| p.get("session_id"))
            .and_then(|s| s.as_str())
            .map(String::from);

        Some(Self {
            task_id,
            agent_type: None,
            session_id,
            project: None,
            method,
            is_encrypted: false,
            is_broadcast: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_critical() {
        let categorizer = SmartCategorizer::new();
        let result = categorizer.categorize("CERN anomaly detected in LHC data", None);
        assert_eq!(result.category, MessageCategory::Critical);
    }

    #[test]
    fn test_categorize_sensitive() {
        let categorizer = SmartCategorizer::new();
        let result = categorizer.categorize("API_KEY=sk_1234567890abcdef", None);
        assert_eq!(result.category, MessageCategory::Sensitive);
    }

    #[test]
    fn test_categorize_ephemeral() {
        let categorizer = SmartCategorizer::new();
        let result = categorizer.categorize(r#"{"type":"heartbeat"}"#, None);
        assert_eq!(result.category, MessageCategory::Ephemeral);
    }

    #[test]
    fn test_categorize_task() {
        let categorizer = SmartCategorizer::new();
        let result = categorizer.categorize(r#"{"method":"task_create"}"#, None);
        assert_eq!(result.category, MessageCategory::Important);
    }

    #[test]
    fn test_category_ttl() {
        assert_eq!(MessageCategory::Critical.ttl_seconds(), None);
        assert_eq!(
            MessageCategory::Sensitive.ttl_seconds(),
            Some(30 * 24 * 60 * 60)
        );
        assert_eq!(
            MessageCategory::Important.ttl_seconds(),
            Some(90 * 24 * 60 * 60)
        );
        assert_eq!(MessageCategory::Ephemeral.ttl_seconds(), Some(0));
    }
}
