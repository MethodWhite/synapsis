//! Database Tests for Synapsis
//!
//! Unit tests for the Database struct covering CRUD operations,
//! search functionality, timeline ordering, and session management.

use std::env;
use std::io::Write;
use synapsis::domain::entities::*;
use synapsis::domain::types::*;
use synapsis::infrastructure::database::Database;
use synapsis::domain::ports::StoragePort;
use synapsis::domain::ports::SessionPort;

fn test_db() -> Database {
    println!("[TEST_DB] Setting XDG_DATA_HOME to /tmp/synapsis-test");
    let _ = std::io::stdout().flush();
    env::set_var("XDG_DATA_HOME", "/tmp/synapsis-test");
    std::fs::create_dir_all("/tmp/synapsis-test/synapsis").ok();
    println!("[TEST_DB] Creating Database instance");
    let _ = std::io::stdout().flush();
    let db = Database::new();
    println!("[TEST_DB] Calling init");
    let _ = std::io::stdout().flush();
    db.init().unwrap();
    println!("[TEST_DB] Returning db");
    let _ = std::io::stdout().flush();
    db
}

fn cleanup_test_dir() {
    println!("[CLEANUP] Removing /tmp/synapsis-test");
    let _ = std::io::stdout().flush();
    std::fs::remove_dir_all("/tmp/synapsis-test").ok();
    println!("[CLEANUP] Done");
    let _ = std::io::stdout().flush();
}

mod database_tests {
    use super::*;

    #[test]
    fn test_add_and_retrieve_observation() {
        eprintln!("[TEST] Starting test_add_and_retrieve_observation");
        cleanup_test_dir();
        let db = test_db();

        let obs = Observation::new(
            SessionId::new("test-session"),
            ObservationType::Manual,
            "Test Title".to_string(),
            "Test content".to_string(),
        );

        let id = db.save_observation(&obs).unwrap_or_else(|e| panic!("Save failed: {:?}", e));
        assert!(id.0 > 0, "ID should be positive");

        let retrieved = db
            .get_observation(id)
            .expect("Should retrieve observation")
            .expect("Observation should exist");

        assert_eq!(retrieved.title, "Test Title");
        assert_eq!(retrieved.content, "Test content");
        assert_eq!(retrieved.observation_type, ObservationType::Manual);

        cleanup_test_dir();
    }

    #[test]
    fn test_add_multiple_observations_increments_id() {
        cleanup_test_dir();
        let db = test_db();

        for i in 0..5 {
            let obs = Observation::new(
                SessionId::new("test-session"),
                ObservationType::Manual,
                format!("Title {}", i),
                format!("Content {}", i),
            );
            db.save_observation(&obs).expect("Should add observation");
        }

        let stats = db.stats().unwrap();
        assert_eq!(stats["observations"].as_i64().unwrap(), 5, "Should have 5 observations");

        cleanup_test_dir();
    }

    #[test]
    fn test_search_by_title() {
        cleanup_test_dir();
        let db = test_db();

        let obs1 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Rust Programming".to_string(),
            "Content about Rust".to_string(),
        );
        let obs2 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Python Scripting".to_string(),
            "Content about Python".to_string(),
        );
        let obs3 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Rust Web Development".to_string(),
            "More Rust content".to_string(),
        );

        db.save_observation(&obs1).ok();
        db.save_observation(&obs2).ok();
        db.save_observation(&obs3).ok();

        let results = db.search_fts("Rust", None, 10).unwrap();
        assert_eq!(results.len(), 2, "Should find 2 Rust-related observations");

        cleanup_test_dir();
    }

    #[test]
    fn test_search_by_content() {
        cleanup_test_dir();
        let db = test_db();

        let obs1 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Note 1".to_string(),
            "Important security vulnerability found".to_string(),
        );
        let obs2 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Note 2".to_string(),
            "Regular note without issues".to_string(),
        );

        db.save_observation(&obs1).ok();
        db.save_observation(&obs2).ok();

        let results = db.search_fts("security", None, 10).unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should find 1 security-related observation"
        );
        assert!(results[0]["content"].as_str().unwrap().contains("vulnerability"));

        cleanup_test_dir();
    }

    #[test]
    fn test_search_by_type() {
        cleanup_test_dir();
        let db = test_db();

        let obs1 = Observation::new(
            SessionId::new("test"),
            ObservationType::ToolUse,
            "Tool execution".to_string(),
            "Content 1".to_string(),
        );
        let obs2 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Manual note".to_string(),
            "Content 2".to_string(),
        );
        let obs3 = Observation::new(
            SessionId::new("test"),
            ObservationType::FileChange,
            "File operation".to_string(),
            "Content 3".to_string(),
        );

        db.save_observation(&obs1).ok();
        db.save_observation(&obs2).ok();
        db.save_observation(&obs3).ok();

        let mut params = SearchParams::new("");
        params.obs_type = Some(ObservationType::ToolUse);

        let results = db.search_observations(&params).expect("Search should work");
        assert_eq!(results.len(), 1, "Should find 1 Tool observation");
        assert_eq!(
            results[0].observation.observation_type,
            ObservationType::ToolUse
        );

        cleanup_test_dir();
    }

    #[test]
    fn test_timeline_chronological_order() {
        cleanup_test_dir();
        let db = test_db();

        let obs1 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "First".to_string(),
            "First content".to_string(),
        );
        let obs2 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Second".to_string(),
            "Second content".to_string(),
        );
        let obs3 = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Third".to_string(),
            "Third content".to_string(),
        );

        db.save_observation(&obs1).ok();
        std::thread::sleep(std::time::Duration::from_millis(10));
        db.save_observation(&obs2).ok();
        std::thread::sleep(std::time::Duration::from_millis(10));
        db.save_observation(&obs3).ok();

        let timeline = db.get_timeline(10).unwrap();
        assert_eq!(timeline.len(), 3, "Should have 3 timeline entries");

        assert_eq!(timeline[0].observation.title, "Third", "Most recent first");
        assert_eq!(timeline[2].observation.title, "First", "Oldest last");

        cleanup_test_dir();
    }

    #[test]
    fn test_timeline_limit() {
        cleanup_test_dir();
        let db = test_db();

        for i in 0..10 {
            let obs = Observation::new(
                SessionId::new("test"),
                ObservationType::Manual,
                format!("Title {}", i),
                format!("Content {}", i),
            );
            db.save_observation(&obs).ok();
        }

        let timeline = db.get_timeline(5).unwrap();
        assert_eq!(timeline.len(), 5, "Should limit to 5 entries");

        cleanup_test_dir();
    }

    #[test]
    fn test_session_create_and_list() {
        cleanup_test_dir();
        let db = test_db();

        let session_id = db
            .start_session("test-project", "/tmp")
            .expect("Should create session");

        assert!(!session_id.0.is_empty(), "Session ID should not be empty");

        let sessions = db.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1, "Should have 1 session");
        assert_eq!(sessions[0].project, "test-project");
        assert!(sessions[0].ended_at.is_none(), "Session should be active");

        cleanup_test_dir();
    }

    #[test]
    fn test_session_end() {
        cleanup_test_dir();
        let db = test_db();

        let session_id = db
            .start_session("test-project", "/tmp")
            .expect("Should create session");

        db.end_session(&session_id, Some("Test summary".to_string()))
            .expect("Should end session");

        let sessions = db.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1, "Should have 1 session");
        assert!(sessions[0].ended_at.is_some(), "Session should be ended");
        assert_eq!(sessions[0].summary.as_deref(), Some("Test summary"));

        cleanup_test_dir();
    }

    #[test]
    fn test_stats_accuracy() {
        cleanup_test_dir();
        let db = test_db();

        db.start_session("project1", "/tmp").ok();
        db.start_session("project2", "/tmp").ok();

        db.save_observation(&Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Title 1".to_string(),
            "Content 1".to_string(),
        ))
        .ok();

        db.save_observation(&Observation::new(
            SessionId::new("test"),
            ObservationType::ToolUse,
            "Title 2".to_string(),
            "Content 2".to_string(),
        ))
        .ok();

        let stats = db.stats().unwrap();
        assert_eq!(stats["observations"].as_i64().unwrap(), 2);
        assert_eq!(stats["agent_sessions"].as_i64().unwrap(), 2);
        assert_eq!(stats["active_agents"].as_i64().unwrap(), 2);

        cleanup_test_dir();
    }

    #[test]
    fn test_search_case_insensitive() {
        cleanup_test_dir();
        let db = test_db();

        let obs = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "RUST Programming".to_string(),
            "CONTENT here".to_string(),
        );
        db.save_observation(&obs).ok();

        let results_lower = db.search_fts("rust", None, 10).unwrap();
        let results_upper = db.search_fts("RUST", None, 10).unwrap();
        let results_mixed = db.search_fts("RuSt", None, 10).unwrap();

        assert_eq!(results_lower.len(), results_upper.len());
        assert_eq!(results_upper.len(), results_mixed.len());
        assert_eq!(results_lower.len(), 1, "Search should be case-insensitive");

        cleanup_test_dir();
    }

    #[test]
    fn test_search_empty_query_returns_all() {
        cleanup_test_dir();
        let db = test_db();

        for i in 0..3 {
            let obs = Observation::new(
                SessionId::new("test"),
                ObservationType::Manual,
                format!("Title {}", i),
                format!("Content {}", i),
            );
            db.save_observation(&obs).ok();
        }

        let results = db.search_fts("", None, 10).unwrap();
        assert_eq!(results.len(), 3, "Empty query should return all");

        cleanup_test_dir();
    }

    #[test]
    fn test_concurrent_save_observations() {
        cleanup_test_dir();
        let db = test_db();
        let db_clone = db.clone();

        let handle1 = std::thread::spawn(move || {
            for i in 0..10 {
                let obs = Observation::new(
                    SessionId::new("test1"),
                    ObservationType::Manual,
                    format!("Thread1-{}", i),
                    format!("Content {}", i),
                );
                db_clone.save_observation(&obs).ok();
            }
        });

        let db_clone2 = db.clone();
        let handle2 = std::thread::spawn(move || {
            for i in 0..10 {
                let obs = Observation::new(
                    SessionId::new("test2"),
                    ObservationType::Manual,
                    format!("Thread2-{}", i),
                    format!("Content {}", i),
                );
                db_clone2.save_observation(&obs).ok();
            }
        });

        handle1.join().expect("Thread 1 should complete");
        handle2.join().expect("Thread 2 should complete");

        let stats = db.stats().unwrap();
        assert_eq!(
            stats["observations"].as_i64().unwrap(), 20,
            "Should have 20 observations from both threads"
        );

        cleanup_test_dir();
    }

    #[test]
    fn test_persistence_across_instances() {
        cleanup_test_dir();

        {
            let db1 = test_db();
            let obs = Observation::new(
                SessionId::new("test"),
                ObservationType::Manual,
                "Persisted".to_string(),
                "Content".to_string(),
            );
            db1.save_observation(&obs).ok();
        }

        {
            let db2 = test_db();
            let timeline = db2.get_timeline(10).unwrap();
            assert!(
                timeline.iter().any(|e| e.observation.title == "Persisted"),
                "Observation should persist across instances"
            );
        }

        cleanup_test_dir();
    }

    #[test]
    fn test_search_with_special_characters() {
        cleanup_test_dir();
        let db = test_db();

        let obs = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "Test with 'quotes'".to_string(),
            "Content with \"double\" and 'single'".to_string(),
        );
        db.save_observation(&obs).ok();

        let results = db.search_fts("quotes", None, 10).unwrap();
        assert_eq!(results.len(), 1, "Should find with special chars");

        cleanup_test_dir();
    }

    #[test]
    fn test_search_with_unicode() {
        cleanup_test_dir();
        let db = test_db();

        let obs = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            "测试标题".to_string(),
            "日本語コンテンツ".to_string(),
        );
        db.save_observation(&obs).ok();

        let results = db.search_fts("测试", None, 10).unwrap();
        assert_eq!(results.len(), 1, "Should find unicode content");

        cleanup_test_dir();
    }
}
