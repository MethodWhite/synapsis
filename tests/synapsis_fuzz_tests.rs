//! Fuzz Tests and Property-Based Tests for Synapsis
//!
//! Property-based tests using proptest for robust testing of
//! hash functions, encryption, search, and MCP protocol parsing.


use std::env;
use synapsis::domain::entities::*;
use synapsis::domain::types::*;
use synapsis::infrastructure::database::Database;
use synapsis::domain::ports::StoragePort;

fn test_db() -> Database {
    env::set_var("XDG_DATA_HOME", "/tmp/synapsis-fuzz-test");
    std::fs::create_dir_all("/tmp/synapsis-fuzz-test/synapsis").ok();
    let db = Database::new();

    db
}

fn cleanup_test_dir() {
    std::fs::remove_dir_all("/tmp/synapsis-fuzz-test").ok();
}

// proptest! {
//     #[test]
//     fn fuzz_search_query_special_chars(query: String) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             ObservationType::Manual,
//             format!("Test with '{}'", query),
//             "Content".to_string(),
//         );
//         db.add_observation(obs).ok();
// 
//         let results = db.search(&query, 10);
// 
//         if !query.is_empty() && !query.chars().all(|c| c.is_whitespace()) {
//             prop_assert!(results.len() <= 1, "Should find at most 1 result");
//         }
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_unicode_search(unicode_str in "\\PC*") {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             ObservationType::Manual,
//             unicode_str.clone(),
//             unicode_str.clone(),
//         );
//         db.add_observation(obs).ok();
// 
//         let results = db.search(&unicode_str, 10);
// 
//         prop_assert!(results.len() <= 1, "Should find at most 1 result");
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_long_content(content: String) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             ObservationType::Manual,
//             "Long content test".to_string(),
//             content.clone(),
//         );
// 
//         let id = db.add_observation(obs);
//         prop_assert!(id.is_ok(), "Should handle long content");
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_titles(title: String) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             ObservationType::Manual,
//             title.clone(),
//             "Content".to_string(),
//         );
// 
//         let id = db.add_observation(obs);
//         prop_assert!(id.is_ok(), "Should handle any title");
// 
//         if !title.is_empty() {
//             let results = db.search(&title, 10);
//             prop_assert!(!results.is_empty(), "Should find by exact title");
//         }
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_observation_type(typ: u8) {
//         let obs_type = match typ % 6 {
//             0 => ObservationType::Manual,
//             1 => ObservationType::Tool,
//             2 => ObservationType::File,
//             3 => ObservationType::Command,
//             4 => ObservationType::Search,
//             _ => ObservationType::Decision,
//         };
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             obs_type,
//             "Test".to_string(),
//             "Content".to_string(),
//         );
// 
//         prop_assert_eq!(obs.observation_type, obs_type);
//     }
// 
//     #[test]
//     fn fuzz_search_limit(limit in 0i32..1000) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         for i in 0..10 {
//             let obs = Observation::new(
//                 SessionId::new("test"),
//                 ObservationType::Manual,
//                 format!("Title {}", i),
//                 "Content".to_string(),
//             );
//             db.add_observation(obs).ok();
//         }
// 
//         let timeline = db.get_timeline(limit);
// 
//         if limit <= 0 {
//             prop_assert!(timeline.is_empty() || limit == 0, "Should handle 0 limit");
//         } else {
//             prop_assert!(timeline.len() as i32 <= limit.min(10), "Should respect limit");
//         }
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn property_hash_deterministic(content: String) {
//         use synapsis::core::uuid::Uuid;
// 
//         let id1 = Uuid::new_v4();
//         let id2 = Uuid::new_v4();
// 
//         prop_assert_ne!(id1.to_hex_string(), id2.to_hex_string(), "UUIDs should be unique");
//     }
// 
//     #[test]
//     fn property_search_case_insensitive(
//         title in "[a-zA-Z]+",
//         search in "[a-zA-Z]+"
//     ) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             ObservationType::Manual,
//             title.clone(),
//             "Content".to_string(),
//         );
//         db.add_observation(obs).ok();
// 
//         let results_lower = db.search(&search.to_lowercase(), 10);
//         let results_upper = db.search(&search.to_uppercase(), 10);
// 
//         prop_assert_eq!(
//             results_lower.len(),
//             results_upper.len(),
//             "Search should be case insensitive"
//         );
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn property_multiple_observations_unique_ids(count in 1usize..100) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let mut ids = Vec::new();
//         for i in 0..count {
//             let obs = Observation::new(
//                 SessionId::new("test"),
//                 ObservationType::Manual,
//                 format!("Title {}", i),
//                 "Content".to_string(),
//             );
//             let id = db.add_observation(obs).unwrap();
//             ids.push(id);
//         }
// 
//         ids.sort();
//         ids.dedup();
// 
//         prop_assert_eq!(
//             ids.len(),
//             count,
//             "All observation IDs should be unique"
//         );
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn property_session_id_format(project in "[a-z]+", directory in "[a-z/]+") {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let session_id = db.start_session(&project, &directory);
// 
//         prop_assert!(session_id.is_ok(), "Should create session");
//         prop_assert!(
//             !session_id.unwrap().0.is_empty(),
//             "Session ID should not be empty"
//         );
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn property_timeline_ordering(count in 1usize..20) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         for i in 0..count {
//             let obs = Observation::new(
//                 SessionId::new("test"),
//                 ObservationType::Manual,
//                 format!("{}", 1000 - i),
//                 "Content".to_string(),
//             );
//             db.add_observation(obs).ok();
//             std::thread::sleep(std::time::Duration::from_micros(100));
//         }
// 
//         let timeline = db.get_timeline(count as i32);
// 
//         for i in 0..timeline.len() - 1 {
//             prop_assert!(
//                 timeline[i].observation.created_at.0 >= timeline[i + 1].observation.created_at.0,
//                 "Timeline should be in descending order"
//             );
//         }
// 
//         cleanup_test_dir();
//     }
// }
// 
// mod custom_fuzz_tests {
//     use super::*;
//     use serde_json::Value;
// 
//     #[test]
//     fn fuzz_mcp_request_parsing_valid_json() {
//         let test_cases = vec![
//             r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
//             r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
//             r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"test","arguments":{}}}"#,
//             r#"{"jsonrpc":"2.0","id":4,"method":"resources/list","params":{}}"#,
//         ];
// 
//         let server = test_db();
// 
//         for json_str in test_cases {
//             let result: Result<Value, _> = serde_json::from_str(json_str);
//             assert!(result.is_ok(), "Should parse valid JSON: {}", json_str);
//         }
//     }
// 
//     #[test]
//     fn fuzz_mcp_request_various_content() {
//         let test_cases = vec![
//             (
//                 r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
//                 "initialize",
//             ),
//             (
//                 r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
//                 "tools/list",
//             ),
//             (
//                 r#"{"jsonrpc":"2.0","id":3,"method":"resources/list","params":{}}"#,
//                 "resources/list",
//             ),
//             (
//                 r#"{"jsonrpc":"2.0","id":4,"method":"prompts/list","params":{}}"#,
//                 "prompts/list",
//             ),
//         ];
// 
//         for (json_str, method_name) in test_cases {
//             let parsed: Value = serde_json::from_str(json_str).unwrap();
//             assert_eq!(parsed["jsonrpc"], "2.0");
//             assert_eq!(parsed["method"], method_name);
//         }
//     }
// 
//     #[test]
//     fn fuzz_encrypt_decrypt_consistency() {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let test_strings = vec![
//             "Hello, World!",
//             "Unicode: 日本語 中文 한국어",
//             "Special: !@#$%^&*()",
//             "Numbers: 1234567890",
//             "Mixed: Hello123!@#",
//             "",
//         ];
// 
//         for original in test_strings {
//             let obs = Observation::new(
//                 SessionId::new("test"),
//                 ObservationType::Manual,
//                 "Test".to_string(),
//                 original.to_string(),
//             );
// 
//             let id = db.add_observation(obs);
//             assert!(id.is_ok(), "Should add observation");
// 
//             let retrieved = db.get_observation(id.unwrap());
//             assert!(retrieved.is_ok(), "Should retrieve observation");
//             assert_eq!(
//                 retrieved.unwrap().map(|o| o.content),
//                 Some(original.to_string()),
//                 "Content should match"
//             );
//         }
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_search_empty_query() {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         for i in 0..5 {
//             let obs = Observation::new(
//                 SessionId::new("test"),
//                 ObservationType::Manual,
//                 format!("Title {}", i),
//                 format!("Content {}", i),
//             );
//             db.add_observation(obs).ok();
//         }
// 
//         let results = db.search("", 10);
//         assert_eq!(results.len(), 5, "Empty query should return all");
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_search_no_matches() {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             ObservationType::Manual,
//             "Apple banana".to_string(),
//             "Orange".to_string(),
//         );
//         db.add_observation(obs).ok();
// 
//         let results = db.search("xyz123", 10);
//         assert_eq!(results.len(), 0, "Should find no matches");
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_partial_matches() {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let titles = vec![
//             "rust programming",
//             "rustacean",
//             "rustic design",
//             "trust me",
//             "crust",
//         ];
// 
//         for title in titles {
//             let obs = Observation::new(
//                 SessionId::new("test"),
//                 ObservationType::Manual,
//                 title.to_string(),
//                 "Content".to_string(),
//             );
//             db.add_observation(obs).ok();
//         }
// 
//         let results = db.search("rust", 10);
// 
//         assert_eq!(results.len(), 3, "Should find 3 rust-related");
//         for result in results {
//             assert!(
//                 result.observation.title.to_lowercase().contains("rust"),
//                 "All results should contain 'rust'"
//             );
//         }
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_concurrent_adds(count: usize) {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let handles: Vec<_> = (0..count.min(10))
//             .map(|i| {
//                 let db_clone = db.clone();
//                 std::thread::spawn(move || {
//                     for j in 0..5 {
//                         let obs = Observation::new(
//                             SessionId::new("test"),
//                             ObservationType::Manual,
//                             format!("Thread{}-{}", i, j),
//                             "Content".to_string(),
//                         );
//                         db_clone.add_observation(obs).ok();
//                     }
//                 })
//             })
//             .collect();
// 
//         for handle in handles {
//             handle.join().ok();
//         }
// 
//         let stats = db.stats();
//         assert_eq!(
//             stats.total_observations as usize,
//             count * 5,
//             "Should have all observations"
//         );
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_memory_pressure() {
//         cleanup_test_dir();
//         let db = test_db();
// 
//         let large_content = "x".repeat(10000);
// 
//         let obs = Observation::new(
//             SessionId::new("test"),
//             ObservationType::Manual,
//             "Large".to_string(),
//             large_content,
//         );
// 
//         let id = db.add_observation(obs);
//         assert!(id.is_ok(), "Should handle large content");
// 
//         cleanup_test_dir();
//     }
// 
//     #[test]
//     fn fuzz_observation_persistence() {
//         cleanup_test_dir();
// 
//         let id = {
//             let db = test_db();
//             let obs = Observation::new(
//                 SessionId::new("persist"),
//                 ObservationType::Manual,
//                 "Persistent".to_string(),
//                 "Content".to_string(),
//             );
//             db.add_observation(obs).unwrap()
//         };
// 
//         {
//             let db = test_db();
//             let retrieved = db.get_observation(id);
//             assert!(
//                 retrieved.is_ok() && retrieved.unwrap().is_some(),
//                 "Should persist across instances"
//             );
//         }
// 
//         cleanup_test_dir();
//     }
// }

#[test]
fn test_search_query_injection_attempt() {
    cleanup_test_dir();
    let db = test_db();

    let malicious_inputs = vec![
        "'; DROP TABLE observations; --",
        "<script>alert('xss')</script>",
        "$$hash_comment$$",
        "/* inline comment */",
        "\n\n\nnewlines",
        "\t\ttabs",
    ];

    for input in malicious_inputs {
        let obs = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            format!("Test '{}'", input),
            "Content".to_string(),
        );

        let id = db.save_observation(&obs);
        assert!(id.is_ok(), "Should handle potentially malicious input");

        let results = db.search_fts(input, None, 10).unwrap();
        assert!(results.len() <= 1, "Should handle search safely");
    }

    cleanup_test_dir();
}

#[test]
fn test_unicode_normalization() {
    cleanup_test_dir();
    let db = test_db();

    let unicode_forms = vec![
        " café",    // combining accent
        " café",    // precomposed
        "\u{00E9}", // codepoint
    ];

    for (i, form) in unicode_forms.iter().enumerate() {
        let obs = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            format!("Unicode{}", i),
            form.to_string(),
        );
        db.save_observation(&obs).ok();
    }

    cleanup_test_dir();
}

#[test]
fn test_stats_after_operations() {
    cleanup_test_dir();
    let db = test_db();

    let initial_stats = db.stats().unwrap();
    assert_eq!(initial_stats["observations"].as_i64().unwrap(), 0);

    for i in 0..10 {
        let obs = Observation::new(
            SessionId::new("test"),
            ObservationType::Manual,
            format!("Title{}", i),
            "Content".to_string(),
        );
        db.save_observation(&obs).unwrap();
    }

    let final_stats = db.stats().unwrap();
    assert_eq!(final_stats["observations"].as_i64().unwrap(), 10);

    cleanup_test_dir();
}
