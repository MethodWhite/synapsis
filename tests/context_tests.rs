//! # Context Module Tests

use synapsis::infrastructure::context::{
    AccessLevel, Context, ContextId, ContextMetrics, ContextRef, ContextRegistry, ContextState,
    ContextType, ContextValue, Priority,
};

#[test]
fn test_context_id_new() {
    let id1 = ContextId::new();
    let id2 = ContextId::new();
    assert_ne!(id1.0, id2.0);
    assert!(!id1.0.is_empty());
}

#[test]
fn test_context_id_default() {
    let id: ContextId = Default::default();
    assert!(!id.0.is_empty());
}

#[test]
fn test_context_creation() {
    let ctx = Context::new("test".to_string(), ContextType::Session);
    assert_eq!(ctx.name, "test");
    assert_eq!(ctx.context_type, ContextType::Session);
    assert_eq!(ctx.state, ContextState::Hot);
    assert_eq!(ctx.priority, Priority::Normal);
    assert!(ctx.variables.is_empty());
    assert!(ctx.connections.is_empty());
}

#[test]
fn test_context_variables() {
    let mut ctx = Context::new("test".to_string(), ContextType::Task);
    ctx.set_var("name", ContextValue::from("Alice"));
    ctx.set_var("age", ContextValue::from(30));
    ctx.set_var("active", ContextValue::from(true));

    assert_eq!(
        ctx.get_var("name").unwrap().as_string(),
        Some(&"Alice".to_string())
    );
    assert_eq!(ctx.get_var("age").unwrap().as_number(), Some(30.0));
    assert_eq!(ctx.get_var("active").unwrap().as_bool(), Some(true));
    assert!(ctx.get_var("missing").is_none());
}

#[test]
fn test_context_value_size() {
    assert_eq!(ContextValue::from("hello").estimated_size(), 5);
    assert_eq!(ContextValue::from(42i64).estimated_size(), 8);
    assert_eq!(ContextValue::from(true).estimated_size(), 1);
    assert_eq!(ContextValue::from(false).estimated_size(), 1);
    assert_eq!(ContextValue::Null.estimated_size(), 0);
}

#[test]
fn test_context_touch() {
    let mut ctx = Context::new("test".to_string(), ContextType::Project);
    ctx.state = ContextState::Warm;
    let initial_access = ctx.metrics.access_count;

    ctx.touch();

    assert_eq!(ctx.metrics.access_count, initial_access + 1);
    assert_eq!(ctx.state, ContextState::Hot);
}

#[test]
fn test_context_memory_size() {
    let mut ctx = Context::new("test".to_string(), ContextType::Task);
    ctx.set_var("key", ContextValue::from("value"));
    ctx.summary = "A test context".to_string();

    let size = ctx.memory_size();
    assert!(size > 0);
}

#[test]
fn test_context_summary() {
    let mut ctx = Context::new("mycontext".to_string(), ContextType::Task);
    assert!(ctx.generate_summary().contains("mycontext"));

    ctx.summary = "Custom summary".to_string();
    assert_eq!(ctx.generate_summary(), "Custom summary");
}

#[test]
fn test_context_metrics_new() {
    let metrics = ContextMetrics::new();
    assert_eq!(metrics.access_count, 0);
    assert!(metrics.hot_score > 0.0);
    assert!(metrics.last_access > 0);
}

#[test]
fn test_context_metrics_touch() {
    let mut metrics = ContextMetrics::new();
    let old_access = metrics.last_access;
    metrics.touch();
    assert_eq!(metrics.access_count, 1);
    assert!(metrics.last_access >= old_access);
}

#[test]
fn test_isolation_config() {
    use synapsis::infrastructure::context::IsolationConfig;

    let config = IsolationConfig::new();
    assert!(config.global_vars.is_empty());
    assert!(config.read_only_vars.is_empty());
    assert!(config.inherit_globals);
}

#[test]
fn test_context_types() {
    let session = Context::new("s".to_string(), ContextType::Session);
    let project = Context::new("p".to_string(), ContextType::Project);
    let task = Context::new("t".to_string(), ContextType::Task);

    assert_eq!(session.context_type, ContextType::Session);
    assert_eq!(project.context_type, ContextType::Project);
    assert_eq!(task.context_type, ContextType::Task);
}

#[test]
fn test_context_states() {
    let mut ctx = Context::new("test".to_string(), ContextType::Session);
    ctx.state = ContextState::Hot;
    assert_eq!(ctx.state, ContextState::Hot);

    ctx.state = ContextState::Warm;
    assert_eq!(ctx.state, ContextState::Warm);

    ctx.state = ContextState::Cold;
    assert_eq!(ctx.state, ContextState::Cold);
}

#[test]
fn test_context_priority() {
    let mut ctx = Context::new("test".to_string(), ContextType::Task);
    ctx.priority = Priority::Critical;
    assert_eq!(ctx.priority, Priority::Critical);

    ctx.priority = Priority::Low;
    assert_eq!(ctx.priority, Priority::Low);

    ctx.priority = Priority::Frozen;
    assert_eq!(ctx.priority, Priority::Frozen);
}

// ContextRegistry tests
#[test]
fn test_registry_create() {
    let mut registry = ContextRegistry::new();
    let id = registry.create("test".to_string(), ContextType::Session);

    let ctx = registry.get(&id);
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().name, "test");
}

#[test]
fn test_registry_create_child() {
    let mut registry = ContextRegistry::new();
    let parent_id = registry.create("parent".to_string(), ContextType::Project);
    let child_id = registry.create_child("child".to_string(), ContextType::Task, &parent_id);

    let child = registry.get(&child_id);
    assert!(child.is_some());
    assert_eq!(child.unwrap().parent, Some(parent_id));
}

#[test]
fn test_registry_get_mut() {
    let mut registry = ContextRegistry::new();
    let id = registry.create("test".to_string(), ContextType::Session);

    {
        let ctx = registry.get_mut(&id);
        assert!(ctx.is_some());
    }
}

#[test]
fn test_registry_touch() {
    let mut registry = ContextRegistry::new();
    let id = registry.create("test".to_string(), ContextType::Session);

    registry.touch(&id);

    let ctx = registry.get(&id);
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().metrics.access_count, 1);
}

#[test]
fn test_registry_global_vars() {
    let mut registry = ContextRegistry::new();
    registry.set_global("app_name", ContextValue::from("Synapsis"));
    registry.set_global("version", ContextValue::from(1.0));

    assert_eq!(
        registry.get_global("app_name").unwrap().as_string(),
        Some(&"Synapsis".to_string())
    );
    assert_eq!(
        registry.get_global("version").unwrap().as_number(),
        Some(1.0)
    );
    assert!(registry.get_global("missing").is_none());
}

#[test]
fn test_registry_search_by_name() {
    let mut registry = ContextRegistry::new();
    registry.create("Rust project".to_string(), ContextType::Project);
    registry.create("Python script".to_string(), ContextType::Task);
    registry.create("Rust API".to_string(), ContextType::Session);

    let results = registry.search("rust");
    assert_eq!(results.len(), 2);
    assert!(results
        .iter()
        .all(|r| r.name.to_lowercase().contains("rust")));
}

#[test]
fn test_registry_search_by_summary() {
    let mut registry = ContextRegistry::new();
    let id = registry.create("test".to_string(), ContextType::Project);
    {
        let ctx = registry.get_mut(&id);
        if let Some(c) = ctx {
            c.summary = "Important security audit".to_string();
        }
    }

    let results = registry.search("security");
    assert_eq!(results.len(), 1);
    assert!(results[0].relevance > 0.0);
}

#[test]
fn test_registry_search_with_tags() {
    let mut registry = ContextRegistry::new();
    let id = registry.create("test".to_string(), ContextType::Task);
    {
        let ctx = registry.get_mut(&id);
        if let Some(c) = ctx {
            c.tags.insert("urgent".to_string());
            c.tags.insert("bugfix".to_string());
        }
    }

    let results = registry.search("urgent");
    assert_eq!(results.len(), 1);
    assert!(results[0].relevance > 0.0);
}

#[test]
fn test_registry_stats() {
    let mut registry = ContextRegistry::new();
    registry.create("c1".to_string(), ContextType::Session);
    registry.create("c2".to_string(), ContextType::Project);

    let stats = registry.stats();
    assert_eq!(stats.hot, 2);
    assert_eq!(stats.warm, 0);
    assert!(stats.working_set >= 2);
}

#[test]
fn test_registry_list() {
    let mut registry = ContextRegistry::new();
    registry.create("test1".to_string(), ContextType::Session);
    registry.create("test2".to_string(), ContextType::Task);

    let list = registry.list();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|c| c.state == ContextState::Hot));
}

#[test]
fn test_registry_list_info() {
    let mut registry = ContextRegistry::new();
    let id = registry.create("mycontext".to_string(), ContextType::Project);

    let list = registry.list();
    let info = list.iter().find(|c| c.context_id == id).unwrap();
    assert_eq!(info.name, "mycontext");
    assert_eq!(info.context_type, ContextType::Project);
    assert_eq!(info.priority, Priority::Normal);
}

#[test]
fn test_registry_get_nonexistent() {
    let registry = ContextRegistry::new();
    let fake_id = ContextId::new();
    assert!(registry.get(&fake_id).is_none());
}

#[test]
fn test_context_value_from_conversions() {
    let s: ContextValue = "hello".into();
    assert!(matches!(s, ContextValue::String(_)));

    let n: ContextValue = 42i64.into();
    assert!(matches!(n, ContextValue::Number(42.0)));

    let f: ContextValue = 2.718f64.into();
    assert!(matches!(f, ContextValue::Number(2.718)));

    let b: ContextValue = true.into();
    assert!(matches!(b, ContextValue::Boolean(true)));
}

#[test]
fn test_context_ref() {
    use synapsis::infrastructure::context::ContextRef;

    let ref1 = ContextRef {
        id: ContextId::new(),
        access_level: AccessLevel::Full,
    };
    let ref2 = ContextRef {
        id: ContextId::new(),
        access_level: AccessLevel::Partial,
    };

    assert_ne!(ref1.id.0, ref2.id.0);
    assert_ne!(ref1.access_level, ref2.access_level);
}

#[test]
fn test_access_levels() {
    let levels = vec![
        AccessLevel::None,
        AccessLevel::MetadataOnly,
        AccessLevel::Summary,
        AccessLevel::Partial,
        AccessLevel::Full,
    ];

    assert_eq!(levels.len(), 5);
    for level in levels {
        let ctx = Context::new("test".to_string(), ContextType::Session);
        let _ref = ContextRef {
            id: ctx.id.clone(),
            access_level: level,
        };
    }
}

#[test]
fn test_context_with_metadata() {
    let mut ctx = Context::new("test".to_string(), ContextType::Task);
    ctx.metadata
        .insert("author".to_string(), "Alice".to_string());
    ctx.metadata
        .insert("version".to_string(), "1.0".to_string());

    assert_eq!(ctx.metadata.get("author"), Some(&"Alice".to_string()));
    assert_eq!(ctx.metadata.get("version"), Some(&"1.0".to_string()));
}

#[test]
fn test_context_with_tags() {
    let mut ctx = Context::new("test".to_string(), ContextType::Project);
    ctx.tags.insert("rust".to_string());
    ctx.tags.insert("web".to_string());

    assert!(ctx.tags.contains("rust"));
    assert!(ctx.tags.contains("web"));
    assert!(!ctx.tags.contains("python"));
}

#[test]
fn test_context_value_arrays() {
    let arr = ContextValue::Array(vec![
        ContextValue::from(1i64),
        ContextValue::from(2i64),
        ContextValue::from(3i64),
    ]);

    assert_eq!(arr.estimated_size(), 24); // 3 * 8 bytes
}

#[test]
fn test_context_value_objects() {
    let obj = ContextValue::Object(vec![
        ("name".to_string(), ContextValue::from("Alice")),
        ("age".to_string(), ContextValue::from(30)),
    ]);

    assert!(obj.estimated_size() > 10);
}
