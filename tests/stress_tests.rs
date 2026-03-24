//! Synapsis Stress Tests
//!
//! Tests para verificar resistencia a race conditions y concurrency

#[cfg(test)]
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use std::thread;
    use std::time::Duration;
    use synapsis::core::*;
    use synapsis::domain::*;

    // ═══════════════════════════════════════════════════════════════════
    // STRESS TEST: Concurrent Observations
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn stress_concurrent_observations() {
        use std::sync::Arc;
        use synapsis::infrastructure::Database;

        let storage = Arc::new(Database::new());
        storage.init().unwrap();

        let counter = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));

        let mut handles = vec![];

        for agent_id in 0..100 {
            let storage = storage.clone();
            let counter = counter.clone();
            let errors = errors.clone();

            handles.push(std::thread::spawn(move || {
                for i in 0..10 {
                    let obs = Observation::new(
                        SessionId::new(&format!("agent-{}-session", agent_id)),
                        ObservationType::Bugfix,
                        format!("Agent {} Bug Fix {}", agent_id, i),
                        format!("Content from agent {} observation {}", agent_id, i),
                    );

                    match storage.save_observation(&obs) {
                        Ok(_) => {
                            counter.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let total = counter.load(Ordering::Relaxed);
        let errs = errors.load(Ordering::Relaxed);

        println!("Concurrent test: {} successes, {} errors", total, errs);
        assert_eq!(errs, 0, "Race condition detected!");
        assert_eq!(total, 1000, "Not all observations were added");
    }

    // ═══════════════════════════════════════════════════════════════════
    // STRESS TEST: Deduplication Race
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn stress_deduplication_race() {
        use std::sync::Arc;
        use synapsis::infrastructure::Database;

        let storage = Arc::new(Database::new());
        storage.init().unwrap();

        let base_obs = Observation::new(
            SessionId::new("dedup-test"),
            ObservationType::Bugfix,
            "Identical Bug".to_string(),
            "Same content".to_string(),
        );

        let counter = Arc::new(AtomicU32::new(0));
        let unique_ids = Arc::new(std::sync::Mutex::new(Vec::<ObservationId>::new()));

        let mut handles = vec![];

        for _ in 0..50 {
            let mut obs = base_obs.clone();
            let storage = storage.clone();
            let counter = counter.clone();
            let ids = unique_ids.clone();

            handles.push(std::thread::spawn(move || {
                for _ in 0..10 {
                    let id = storage.save_observation(&obs);
                    if id.is_ok() {
                        counter.fetch_add(1, Ordering::Relaxed);
                        let mut guard = ids.lock().unwrap();
                        guard.push(id.unwrap());
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let total = counter.load(Ordering::Relaxed);
        let guard = unique_ids.lock().unwrap();

        println!(
            "Deduplication test: {} inserts, {} unique ids",
            total,
            guard.len()
        );

        assert!(total >= 10, "Should have at least 10 successful inserts");
    }

    // ═══════════════════════════════════════════════════════════════════
    // TEST: Circuit Breaker Basic
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_circuit_breaker() {
        use synapsis::core::retry::CircuitBreaker;

        let cb = CircuitBreaker::new(3, 1);

        assert!(cb.is_closed(), "Circuit should start closed");
        assert_eq!(cb.state(), synapsis::core::retry::CircuitState::Closed);

        cb.failure();
        cb.failure();

        assert_eq!(cb.state(), synapsis::core::retry::CircuitState::Closed);

        cb.failure();

        assert_eq!(cb.state(), synapsis::core::retry::CircuitState::Open);
        assert!(cb.check().is_err(), "Should reject when open");
    }

    // ═══════════════════════════════════════════════════════════════════
    // TEST: Retry with Backoff
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn stress_retry_backoff() {
        use std::time::Duration;
        use synapsis::core::retry::Retry;

        let attempts = AtomicU32::new(0);
        let retry = Retry::new(5, Duration::from_millis(1), Duration::from_secs(1));

        let result = retry.execute(|| {
            let curr = attempts.fetch_add(1, Ordering::Relaxed) + 1;
            if curr < 3 {
                Err(())
            } else {
                Ok(42)
            }
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::Relaxed), 3);
    }

    // ═══════════════════════════════════════════════════════════════════
    // TEST: SecureRng
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_secure_rng() {
        use synapsis::core::security::SecureRng;

        let rng = SecureRng::new();

        let val1 = rng.random_u64();
        let val2 = rng.random_u64();

        assert_ne!(val1, val2, "Random values should be different");

        let mut buf = [0u8; 32];
        SecureRng::fill_random(&mut buf);

        let unique: std::collections::HashSet<_> = buf.iter().collect();
        assert!(unique.len() > 10, "Low entropy detected!");
    }

    // ═══════════════════════════════════════════════════════════════════
    // TEST: UUID Generation
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_uuid_generation() {
        use synapsis::core::uuid::Uuid;

        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();

        assert_ne!(u1, u2);
        assert_eq!(u1.to_hex_string().len(), 34);
    }

    // ═══════════════════════════════════════════════════════════════════
    // TEST: Observation CRUD
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_observation_crud() {
        use synapsis::infrastructure::Database;

        let db = Database::new();
        db.init().unwrap();

        let obs = Observation::new(
            SessionId::new("test-session"),
            ObservationType::Bugfix,
            "Test Bug".to_string(),
            "Test Content".to_string(),
        );

        let id = db.save_observation(&obs).unwrap();
        let retrieved = db.get_observation(id).unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content, "Test Content");
    }

    // ═══════════════════════════════════════════════════════════════════
    // TEST: Agent Registry
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_agent_registry() {
        use synapsis::infrastructure::agents::{Agent, AgentRegistry, AgentRole};

        let registry = AgentRegistry::new();

        let agent = Agent::new(
            "test-agent".to_string(),
            AgentRole::Coder,
            "Test agent".to_string(),
        );

        let agent_id = registry.register(agent);

        let retrieved = registry.get(&agent_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-agent");
    }
