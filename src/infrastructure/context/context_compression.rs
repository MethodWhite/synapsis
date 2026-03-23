//! # Context Compression & Budget
//!
//! Sistema de compresión de contexto basado en principios de:
//! - Goldilocks Context: mínimo necesario
//! - Context Tiers: esenciales, detalles, comprimido
//! - Lost in the Middle: info crítica al inicio/final

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Token budget del contexto
#[derive(Debug, Clone)]
pub struct ContextBudget {
    pub limit: usize,
    pub used: usize,
    pub reserved_essentials: usize,
    pub comprehensive_limit: usize,
    pub safety_buffer: usize,
}

impl ContextBudget {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            used: 0,
            reserved_essentials: limit / 5,
            comprehensive_limit: (limit * 3) / 4,
            safety_buffer: limit / 10,
        }
    }

    pub fn available(&self) -> usize {
        self.comprehensive_limit.saturating_sub(self.used)
    }

    pub fn needs_compression(&self) -> bool {
        self.used > self.comprehensive_limit
    }

    pub fn is_critical(&self) -> bool {
        self.used + self.safety_buffer > self.limit
    }

    pub fn update(&mut self, used: usize) {
        self.used = used;
    }

    pub fn compression_needed(&self) -> usize {
        if self.needs_compression() {
            self.used - self.comprehensive_limit + self.safety_buffer
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    None,
    Light,
    Medium,
    Heavy,
    Minimal,
}

impl CompressionLevel {
    pub fn compression_ratio(&self) -> f64 {
        match self {
            CompressionLevel::None => 1.0,
            CompressionLevel::Light => 0.7,
            CompressionLevel::Medium => 0.5,
            CompressionLevel::Heavy => 0.3,
            CompressionLevel::Minimal => 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentTier {
    Essential,
    Standard,
    OnDemand,
    Compressible,
    Archive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFragment {
    pub id: String,
    pub content: String,
    pub tier: ContentTier,
    pub tokens: usize,
    pub relevance: f64,
    pub last_access: i64,
    pub access_count: u64,
}

impl ContextFragment {
    pub fn new(content: String, tier: ContentTier) -> Self {
        let tokens = estimate_tokens(&content);
        Self {
            id: generate_id(),
            content,
            tier,
            tokens,
            relevance: 0.5,
            last_access: now_ts(),
            access_count: 0,
        }
    }

    pub fn touch(&mut self) {
        self.last_access = now_ts();
        self.access_count += 1;
    }

    pub fn compress(&self, level: CompressionLevel) -> String {
        match level {
            CompressionLevel::None => self.content.clone(),
            CompressionLevel::Light => compress_light(&self.content),
            CompressionLevel::Medium => compress_medium(&self.content),
            CompressionLevel::Heavy => compress_heavy(&self.content),
            CompressionLevel::Minimal => compress_minimal(&self.content),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompressedContext {
    pub start_fragments: Vec<ContextFragment>,
    pub middle_fragments: Vec<ContextFragment>,
    pub end_fragments: Vec<ContextFragment>,
    pub archived_ids: Vec<String>,
}

impl CompressedContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_start(&mut self, frag: ContextFragment) {
        self.start_fragments.push(frag);
    }

    pub fn add_middle(&mut self, frag: ContextFragment) {
        self.middle_fragments.push(frag);
    }

    pub fn add_end(&mut self, frag: ContextFragment) {
        self.end_fragments.push(frag);
    }

    pub fn total_tokens(&self) -> usize {
        self.start_fragments.iter().map(|f| f.tokens).sum::<usize>()
            + self
                .middle_fragments
                .iter()
                .map(|f| f.tokens)
                .sum::<usize>()
            + self.end_fragments.iter().map(|f| f.tokens).sum::<usize>()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# Essential Context\n\n");
        for frag in &self.start_fragments {
            out.push_str(&frag.content);
            out.push_str("\n\n");
        }
        if !self.middle_fragments.is_empty() {
            out.push_str("---\n# Main Context\n\n");
            for frag in &self.middle_fragments {
                out.push_str(&frag.content);
                out.push_str("\n\n");
            }
        }
        out.push_str("---\n# End Context\n\n");
        for frag in &self.end_fragments {
            out.push_str(&frag.content);
            out.push_str("\n\n");
        }
        out
    }
}

pub struct ContextCompressor {
    history: VecDeque<CompressionRecord>,
}

#[derive(Debug, Clone)]
struct CompressionRecord {
    original_tokens: usize,
    compressed_tokens: usize,
    level: CompressionLevel,
}

impl ContextCompressor {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
        }
    }

    pub fn compress_for_budget(
        &self,
        fragments: &[ContextFragment],
        budget: &ContextBudget,
    ) -> CompressedContext {
        let mut result = CompressedContext::new();
        let mut total = 0;

        for frag in fragments
            .iter()
            .filter(|f| f.tier == ContentTier::Essential)
        {
            if total + frag.tokens <= budget.reserved_essentials {
                result.add_start(frag.clone());
                total += frag.tokens;
            }
        }

        for frag in fragments.iter().filter(|f| f.tier == ContentTier::Standard) {
            if total + frag.tokens <= budget.comprehensive_limit {
                result.add_middle(frag.clone());
                total += frag.tokens;
            } else if total + frag.tokens <= budget.limit {
                let compressed = frag.compress(CompressionLevel::Medium);
                let mut c = frag.clone();
                c.content = compressed;
                c.tokens = estimate_tokens(&c.content);
                result.add_middle(c);
                total += frag.tokens;
            }
        }

        for frag in fragments
            .iter()
            .filter(|f| f.tier == ContentTier::Compressible)
        {
            if total + frag.tokens <= budget.limit {
                result.add_end(frag.clone());
                total += frag.tokens;
            }
        }

        for frag in fragments.iter().filter(|f| f.tier == ContentTier::Archive) {
            result.archived_ids.push(frag.id.clone());
        }

        result
    }

    pub fn suggest_compression(&self, budget: &ContextBudget) -> CompressionLevel {
        let usage = budget.used as f64 / budget.limit as f64;
        if usage > 0.95 {
            CompressionLevel::Minimal
        } else if usage > 0.85 {
            CompressionLevel::Heavy
        } else if usage > 0.70 {
            CompressionLevel::Medium
        } else if usage > 0.50 {
            CompressionLevel::Light
        } else {
            CompressionLevel::None
        }
    }
}

impl Default for ContextCompressor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ContextMonitor {
    budget: ContextBudget,
    history: VecDeque<UsageSnapshot>,
    alerts: Vec<ContextAlert>,
}

#[derive(Debug, Clone)]
struct UsageSnapshot {
    timestamp: i64,
    tokens: usize,
}

#[derive(Debug, Clone)]
pub struct ContextAlert {
    pub alert_type: AlertType,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertType {
    HighUsage,
    CompressionNeeded,
    ContextOverflow,
}

impl ContextMonitor {
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            budget,
            history: VecDeque::new(),
            alerts: Vec::new(),
        }
    }

    pub fn record(&mut self, tokens: usize) {
        self.budget.update(tokens);
        self.history.push_back(UsageSnapshot {
            timestamp: now_ts(),
            tokens,
        });
        if self.history.len() > 100 {
            self.history.pop_front();
        }
        self.check_alerts();
    }

    fn check_alerts(&mut self) {
        self.alerts.clear();
        if self.budget.is_critical() {
            self.alerts.push(ContextAlert {
                alert_type: AlertType::ContextOverflow,
                message: format!(
                    "Context critical: {}/{}",
                    self.budget.used, self.budget.limit
                ),
            });
        } else if self.budget.needs_compression() {
            self.alerts.push(ContextAlert {
                alert_type: AlertType::CompressionNeeded,
                message: format!(
                    "Compression needed: {} excess",
                    self.budget.compression_needed()
                ),
            });
        }
    }

    pub fn get_alerts(&self) -> &[ContextAlert] {
        &self.alerts
    }
    pub fn get_budget(&self) -> &ContextBudget {
        &self.budget
    }
}

fn estimate_tokens(text: &str) -> usize {
    (text.len() / 4).max(1)
}

fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("frag_{:x}", ts)
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn compress_light(text: &str) -> String {
    let mut r = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("```") {
            r.push_str(line);
            r.push('\n');
        } else {
            r.push_str(t);
            r.push(' ');
        }
    }
    r.trim().to_string()
}

fn compress_medium(text: &str) -> String {
    let paras: Vec<&str> = text.split("\n\n").collect();
    if paras.len() <= 2 {
        return compress_light(text);
    }
    let mut r = paras.first().unwrap_or(&"").to_string();
    r.push_str("\n\n[SUMMARY: ");
    for p in paras.iter().skip(1).take(2) {
        if let Some(l) = p.lines().next() {
            r.push_str(l);
            r.push_str(" | ");
        }
    }
    r.push_str("]\n");
    if let Some(l) = paras.last() {
        r.push_str(l);
    }
    r
}

fn compress_heavy(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let headers: String = lines
        .iter()
        .filter(|l| l.trim().starts_with('#'))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    let first = lines.first().map(|l| l.trim()).unwrap_or("");
    let last = lines.last().map(|l| l.trim()).unwrap_or("");
    format!(
        "{}\n{}\n... [compressed {} lines] ...\n{}",
        headers,
        first,
        lines.len(),
        last
    )
}

fn compress_minimal(text: &str) -> String {
    let first = text.chars().take(50).collect::<String>();
    format!("[COMPRESSED: {}...]", first)
}
