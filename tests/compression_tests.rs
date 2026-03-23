//! # Context Compression & Budget Tests

use synapsis::infrastructure::context::{
    AlertType, CompressedContext, CompressionLevel, ContentTier, ContextAlert, ContextBudget,
    ContextCompressor, ContextFragment, ContextMonitor,
};

#[test]
fn test_budget_new() {
    let budget = ContextBudget::new(1000);
    assert_eq!(budget.limit, 1000);
    assert_eq!(budget.used, 0);
    assert_eq!(budget.reserved_essentials, 200); // 20%
    assert_eq!(budget.comprehensive_limit, 750); // 75%
    assert_eq!(budget.safety_buffer, 100); // 10%
}

#[test]
fn test_budget_available() {
    let budget = ContextBudget::new(1000);
    assert_eq!(budget.available(), 750);

    let mut budget = ContextBudget::new(1000);
    budget.update(500);
    assert_eq!(budget.available(), 250);
}

#[test]
fn test_budget_needs_compression() {
    let mut budget = ContextBudget::new(1000);
    assert!(!budget.needs_compression());

    budget.update(800);
    assert!(budget.needs_compression());
}

#[test]
fn test_budget_is_critical() {
    let mut budget = ContextBudget::new(1000);
    assert!(!budget.is_critical());

    budget.update(950);
    assert!(budget.is_critical()); // 950 + 100 (buffer) > 1000
}

#[test]
fn test_budget_update() {
    let mut budget = ContextBudget::new(1000);
    budget.update(500);
    assert_eq!(budget.used, 500);
    budget.update(750);
    assert_eq!(budget.used, 750);
}

#[test]
fn test_budget_compression_needed() {
    let mut budget = ContextBudget::new(1000);
    assert_eq!(budget.compression_needed(), 0);

    budget.update(800);
    assert_eq!(budget.compression_needed(), 150); // 800 - 750 + 100
}

#[test]
fn test_compression_level_ratios() {
    assert_eq!(CompressionLevel::None.compression_ratio(), 1.0);
    assert_eq!(CompressionLevel::Light.compression_ratio(), 0.7);
    assert_eq!(CompressionLevel::Medium.compression_ratio(), 0.5);
    assert_eq!(CompressionLevel::Heavy.compression_ratio(), 0.3);
    assert_eq!(CompressionLevel::Minimal.compression_ratio(), 0.1);
}

#[test]
fn test_content_tier() {
    let tiers = vec![
        ContentTier::Essential,
        ContentTier::Standard,
        ContentTier::OnDemand,
        ContentTier::Compressible,
        ContentTier::Archive,
    ];
    assert_eq!(tiers.len(), 5);
}

#[test]
fn test_fragment_new() {
    let frag = ContextFragment::new("test content".to_string(), ContentTier::Essential);
    assert!(frag.id.starts_with("frag_"));
    assert_eq!(frag.content, "test content");
    assert_eq!(frag.tier, ContentTier::Essential);
    assert!(frag.tokens > 0);
    assert_eq!(frag.relevance, 0.5);
    assert_eq!(frag.access_count, 0);
}

#[test]
fn test_fragment_touch() {
    let mut frag = ContextFragment::new("test".to_string(), ContentTier::Standard);
    let old_access = frag.last_access;
    frag.touch();
    assert_eq!(frag.access_count, 1);
    assert!(frag.last_access >= old_access);
}

#[test]
fn test_fragment_compress_none() {
    let frag = ContextFragment::new("hello world".to_string(), ContentTier::Standard);
    assert_eq!(frag.compress(CompressionLevel::None), "hello world");
}

#[test]
fn test_fragment_compress_light() {
    let content = "line 1\nline 2\nline 3";
    let frag = ContextFragment::new(content.to_string(), ContentTier::Standard);
    let compressed = frag.compress(CompressionLevel::Light);
    assert!(compressed.len() <= content.len());
}

#[test]
fn test_fragment_compress_medium() {
    let content = "paragraph 1\n\nparagraph 2\n\nparagraph 3";
    let frag = ContextFragment::new(content.to_string(), ContentTier::Standard);
    let compressed = frag.compress(CompressionLevel::Medium);
    assert!(compressed.contains("[SUMMARY:"));
}

#[test]
fn test_fragment_compress_heavy() {
    let content = "# Header\nline 1\nline 2\nline 3\nlast line";
    let frag = ContextFragment::new(content.to_string(), ContentTier::Standard);
    let compressed = frag.compress(CompressionLevel::Heavy);
    assert!(compressed.contains("[compressed"));
}

#[test]
fn test_fragment_compress_minimal() {
    let frag = ContextFragment::new(
        "some long content here".to_string(),
        ContentTier::Compressible,
    );
    let compressed = frag.compress(CompressionLevel::Minimal);
    assert!(compressed.starts_with("[COMPRESSED:"));
}

#[test]
fn test_compressed_context_new() {
    let cc = CompressedContext::new();
    assert!(cc.start_fragments.is_empty());
    assert!(cc.middle_fragments.is_empty());
    assert!(cc.end_fragments.is_empty());
    assert!(cc.archived_ids.is_empty());
}

#[test]
fn test_compressed_context_add_fragments() {
    let mut cc = CompressedContext::new();
    let frag1 = ContextFragment::new("start".to_string(), ContentTier::Essential);
    let frag2 = ContextFragment::new("middle".to_string(), ContentTier::Standard);
    let frag3 = ContextFragment::new("end".to_string(), ContentTier::Compressible);

    cc.add_start(frag1.clone());
    cc.add_middle(frag2.clone());
    cc.add_end(frag3.clone());

    assert_eq!(cc.start_fragments.len(), 1);
    assert_eq!(cc.middle_fragments.len(), 1);
    assert_eq!(cc.end_fragments.len(), 1);
}

#[test]
fn test_compressed_context_total_tokens() {
    let mut cc = CompressedContext::new();
    cc.add_start(ContextFragment::new(
        "a".to_string(),
        ContentTier::Essential,
    ));
    cc.add_start(ContextFragment::new(
        "bb".to_string(),
        ContentTier::Essential,
    ));

    let tokens = cc.total_tokens();
    assert!(tokens > 0);
}

#[test]
fn test_compressed_context_render() {
    let mut cc = CompressedContext::new();
    cc.add_start(ContextFragment::new(
        "essential".to_string(),
        ContentTier::Essential,
    ));
    cc.add_middle(ContextFragment::new(
        "standard".to_string(),
        ContentTier::Standard,
    ));
    cc.add_end(ContextFragment::new(
        "compressible".to_string(),
        ContentTier::Compressible,
    ));

    let rendered = cc.render();
    assert!(rendered.contains("# Essential Context"));
    assert!(rendered.contains("# Main Context"));
    assert!(rendered.contains("# End Context"));
    assert!(rendered.contains("essential"));
    assert!(rendered.contains("standard"));
    assert!(rendered.contains("compressible"));
}

#[test]
fn test_compressor_new() {
    let compressor = ContextCompressor::new();
    assert_eq!(
        compressor.suggest_compression(&ContextBudget::new(1000)),
        CompressionLevel::None
    );
}

#[test]
fn test_compressor_suggest_compression() {
    let compressor = ContextCompressor::new();

    let mut budget = ContextBudget::new(1000);
    budget.update(100); // 10%
    assert_eq!(
        compressor.suggest_compression(&budget),
        CompressionLevel::None
    );

    budget.update(600); // 60%
    assert_eq!(
        compressor.suggest_compression(&budget),
        CompressionLevel::Light
    );

    budget.update(750); // 75%
    assert_eq!(
        compressor.suggest_compression(&budget),
        CompressionLevel::Medium
    );

    budget.update(900); // 90%
    assert_eq!(
        compressor.suggest_compression(&budget),
        CompressionLevel::Heavy
    );

    budget.update(980); // 98%
    assert_eq!(
        compressor.suggest_compression(&budget),
        CompressionLevel::Minimal
    );
}

#[test]
fn test_compressor_compress_for_budget() {
    let compressor = ContextCompressor::new();
    let budget = ContextBudget::new(1000);

    let fragments = vec![
        ContextFragment::new("essential 1".to_string(), ContentTier::Essential),
        ContextFragment::new("standard 1".to_string(), ContentTier::Standard),
        ContextFragment::new("compressible 1".to_string(), ContentTier::Compressible),
        ContextFragment::new("archive 1".to_string(), ContentTier::Archive),
    ];

    let result = compressor.compress_for_budget(&fragments, &budget);
    assert!(!result.start_fragments.is_empty());
    assert!(!result.middle_fragments.is_empty());
    assert!(!result.end_fragments.is_empty());
    assert!(!result.archived_ids.is_empty());
}

#[test]
fn test_monitor_new() {
    let budget = ContextBudget::new(1000);
    let monitor = ContextMonitor::new(budget.clone());
    assert_eq!(monitor.get_budget().limit, 1000);
    assert!(monitor.get_alerts().is_empty());
}

#[test]
fn test_monitor_record() {
    let budget = ContextBudget::new(1000);
    let mut monitor = ContextMonitor::new(budget);

    monitor.record(500);
    assert_eq!(monitor.get_budget().used, 500);

    monitor.record(750);
    assert_eq!(monitor.get_budget().used, 750);
}

#[test]
fn test_monitor_alerts_critical() {
    let budget = ContextBudget::new(1000);
    let mut monitor = ContextMonitor::new(budget);

    monitor.record(950);
    let alerts = monitor.get_alerts();
    assert!(!alerts.is_empty());
    assert_eq!(alerts[0].alert_type, AlertType::ContextOverflow);
}

#[test]
fn test_monitor_alerts_compression_needed() {
    let budget = ContextBudget::new(1000);
    let mut monitor = ContextMonitor::new(budget);

    monitor.record(800);
    let alerts = monitor.get_alerts();
    assert!(!alerts.is_empty());
    assert_eq!(alerts[0].alert_type, AlertType::CompressionNeeded);
}

#[test]
fn test_monitor_alerts_none_when_ok() {
    let budget = ContextBudget::new(1000);
    let mut monitor = ContextMonitor::new(budget);

    monitor.record(500);
    assert!(monitor.get_alerts().is_empty());
}

#[test]
fn test_alert_types() {
    let types = vec![
        AlertType::HighUsage,
        AlertType::CompressionNeeded,
        AlertType::ContextOverflow,
    ];
    assert_eq!(types.len(), 3);
}

#[test]
fn test_context_alert() {
    let alert = ContextAlert {
        alert_type: AlertType::HighUsage,
        message: "Test alert".to_string(),
    };
    assert_eq!(alert.message, "Test alert");
    assert_eq!(alert.alert_type, AlertType::HighUsage);
}

#[test]
fn test_compression_chain() {
    let frag = ContextFragment::new(
        "line1\nline2\nline3\nline4\nline5".to_string(),
        ContentTier::Standard,
    );

    let light = frag.compress(CompressionLevel::Light);
    let medium = frag.compress(CompressionLevel::Medium);
    let heavy = frag.compress(CompressionLevel::Heavy);
    let minimal = frag.compress(CompressionLevel::Minimal);

    eprintln!("light={:?}", light);
    eprintln!("medium={:?}", medium);
    eprintln!("heavy={:?}", heavy);
    eprintln!("minimal={:?}", minimal);

    assert!(light.len() <= 30);
    assert!(minimal.starts_with("[COMPRESSED:"));
    assert!(heavy.contains("[compressed"));
    assert!(medium.len() <= light.len());
}

#[test]
fn test_compressed_context_empty_render() {
    let cc = CompressedContext::new();
    let rendered = cc.render();
    assert!(rendered.contains("# Essential Context"));
    assert!(rendered.contains("# End Context"));
    assert!(!rendered.contains("# Main Context")); // No middle fragments
}
