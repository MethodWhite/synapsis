//! Synapsis Stress Tests
//!
//! Tests para verificar resistencia a race conditions y concurrency

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use synapsis::domain::*;

    /// Serialize Database creation + init across the parallel test threads.
    ///
    /// This is a TEST-side guard, not a fix. The underlying defect is in
    /// production code: `run_migrations` has no concurrency guard, so two
    /// threads (or two processes) calling `init()` on the same fresh database
    /// race and one dies with "duplicate column name: prev_hash" or a PRIMARY
    /// KEY violation. On the live database the race is invisible because the
    /// migrations are already applied and there is nothing left to run.
    ///
    /// It only became visible once these tests were pointed at a fresh
    /// temporary directory instead of the live coordination database. The
    /// production-side fix (a migration lock) is still outstanding; without
    /// this guard the suite is order-dependent and fails intermittently.
    fn init_lock() -> std::sync::MutexGuard<'static, ()> {
        use std::sync::{Mutex, OnceLock};
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let m = LOCK.get_or_init(|| Mutex::new(()));
        m.lock().unwrap_or_else(|e| e.into_inner())
    }
    ///
    /// `Database::new()` falls back to `dirs::data_local_dir()/synapsis` when
    /// SYNAPSIS_DATA_DIR is unset, which is the LIVE coordination database. These
    /// stress tests were writing into it on every `cargo test` run. Set once per
    /// binary because the environment is process-global; per-test values would
    /// race across the parallel test threads.
    fn isolate_data_dir() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let dir =
                std::env::temp_dir().join(format!("synapsis-test-stress-{}", std::process::id()));
            std::fs::create_dir_all(&dir).ok();
            // SAFETY: test-scoped env change, applied once before any Database::new().
            unsafe {
                std::env::set_var("SYNAPSIS_DATA_DIR", &dir);
            }
        });
    }

    // ═══════════════════════════════════════════════════════════════════
    // STRESS TEST: Concurrent Observations
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn stress_concurrent_observations() {
        use std::sync::Arc;
        use synapsis::infrastructure::Database;

        let _guard = init_lock();
        isolate_data_dir();
        let storage = Arc::new(Database::new());
        storage.init().unwrap();

        let counter = Arc::new(AtomicU64::new(0));
        let errors = Arc::new(AtomicU64::new(0));

        let mut handles = vec![];

        for agent_id in 0..20 {
            let storage = storage.clone();
            let counter = counter.clone();
            let errors = errors.clone();

            handles.push(std::thread::spawn(move || {
                for i in 0..10 {
                    let obs = Observation::new(
                        SessionId::new(format!("agent-{}-session", agent_id)),
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
        assert_eq!(total, 200, "Not all observations were added");
    }

    // ═══════════════════════════════════════════════════════════════════
    // STRESS TEST: Deduplication Race
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn stress_deduplication_race() {
        use std::sync::Arc;
        use synapsis::infrastructure::Database;

        let _guard = init_lock();
        isolate_data_dir();
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

        for _ in 0..15 {
            let obs = base_obs.clone();
            let storage = storage.clone();
            let counter = counter.clone();
            let ids = unique_ids.clone();

            handles.push(std::thread::spawn(move || {
                for _ in 0..10 {
                    if let Ok(saved_id) = storage.save_observation(&obs) {
                        counter.fetch_add(1, Ordering::Relaxed);
                        let mut guard = ids.lock().unwrap();
                        guard.push(saved_id);
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

        assert!(
            (1..=3).contains(&total),
            "Deduplication should result in ~1-2 inserts (sync_id may vary)"
        );
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
            if curr < 3 { Err(()) } else { Ok(42) }
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

        let _guard = init_lock();
        isolate_data_dir();
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
}
