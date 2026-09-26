//! Cross-process registry tests for the agent registry.
//!
//! The registry file is shared by every `synapsis-mcp` process on the host, so
//! these tests exercise the failure mode that produced the original bug: two
//! independent registry instances writing the same file, where the second write
//! used to erase the first agent because it serialised its own in-memory map
//! over the whole file.
//!
//! Each test drives one registry at a time and uses `SYNAPSIS_DATA_DIR` to point
//! `AgentRegistry::new` at a scratch directory, standing in for separate
//! processes on the same host. That env var is process-global, so the tests
//! serialise on `ENV_LOCK` and pass at any thread count:
//!
//! ```text
//! cargo test --test registry_concurrency_tests                  # 6 passed
//! cargo test --test registry_concurrency_tests -- --test-threads=1
//! ```
//!
//! Without the lock, parallel tests overwrite each other's variable and end up
//! sharing one `agents.json`, which fails four of the six. The project's CI
//! already pins `--test-threads=1`, so the lock only matters for a bare
//! `cargo test` run locally.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use synapsis::infrastructure::agents::{Agent, AgentId, AgentRegistry, AgentRole};

/// `AgentRegistry::new` resolves its directory from `SYNAPSIS_DATA_DIR`, which
/// is process-global. Rust's test harness runs tests in parallel threads by
/// default, so without this guard each test overwrites the variable the others
/// are about to read and they end up sharing one `agents.json`.
///
/// Holding it for the test's lifetime serialises the file regardless of thread
/// count. The project's CI already runs `cargo test -- --test-threads=1`, so
/// this is belt and braces: it keeps a bare `cargo test` honest for whoever
/// runs it locally.
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct Scratch {
    root: PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Scratch {
    /// Point the registry at a fresh directory, serialised against the other
    /// tests in this binary.
    fn new(tag: &str) -> Self {
        let guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // `env::temp_dir` rather than a hardcoded /tmp: this crate is built on
        // Windows in CI, where "/tmp/..." resolves to the current drive's root
        // and may not be writable.
        let root = std::env::temp_dir().join(format!("synapsis-registry-{tag}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        // SAFETY: `_guard` serialises every other test in this binary, so no
        // other thread can be reading or writing this variable right now.
        unsafe { std::env::set_var("SYNAPSIS_DATA_DIR", &root) };
        Self {
            root,
            _guard: guard,
        }
    }

    fn agents_file(&self) -> PathBuf {
        self.root.join("agents").join("agents.json")
    }

    /// A registry instance holding only what *this* process knows, standing in
    /// for a separate MCP process that started before the others existed.
    ///
    /// `init` is deliberately not called: a real process loads the file once at
    /// startup and then only ever sees its own registrations, which is the
    /// condition that let a later write erase an earlier one.
    fn detached(&self) -> AgentRegistry {
        AgentRegistry::new()
    }

    /// A registry instance that has loaded the current on-disk state.
    fn open(&self) -> AgentRegistry {
        let registry = AgentRegistry::new();
        registry.init().unwrap();
        registry
    }

    fn register_as(&self, name: &str) -> String {
        self.detached()
            .register(Agent::new(
                name.to_string(),
                AgentRole::Coder,
                format!("{name} description"),
            ))
            .0
    }

    fn names(&self) -> Vec<String> {
        let data = std::fs::read_to_string(self.agents_file()).unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&data).unwrap_or(serde_json::Value::Null);
        let mut names: Vec<String> = parsed
            .as_object()
            .map(|map| {
                map.values()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Regression: a second registration must not erase the first.
///
/// Before the fix, `save()` wrote the whole in-memory map, so an instance that
/// had only ever seen itself replaced the file contents and the agent
/// registered by the previous instance disappeared.
#[test]
fn second_registration_preserves_the_first() {
    let scratch = Scratch::new("preserve");
    scratch.register_as("claude-code");
    scratch.register_as("codex-hsaq");
    scratch.register_as("opencode");

    assert_eq!(
        scratch.names(),
        vec!["claude-code", "codex-hsaq", "opencode"],
        "every registration must survive"
    );
}

/// A registry that started before the others existed must still see them.
///
/// `load()` used to run only at `init`, so a long-lived process kept reporting
/// its own agents forever.
#[test]
fn a_stale_instance_sees_later_registrations_after_refresh() {
    let scratch = Scratch::new("refresh");

    // This instance loads while the file does not exist yet.
    let early = scratch.open();
    assert!(early.list(None).is_empty(), "registry starts empty");

    scratch.register_as("codex-hsaq");
    scratch.register_as("claude-code");

    // Without a refresh the early instance still sees nothing, which is the
    // reported symptom; the refresh is what makes the shared view usable.
    early.refresh().unwrap();
    let mut names: Vec<String> = early.list(None).into_iter().map(|a| a.name).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["claude-code", "codex-hsaq"],
        "refresh must surface agents registered by other processes"
    );
}

/// Unregistering must not remove anybody else's agent.
#[test]
fn unregister_only_removes_its_own_entry() {
    let scratch = Scratch::new("unregister");
    let mine = scratch.register_as("opencode");
    scratch.register_as("codex-hsaq");

    let registry = scratch.open();
    assert!(
        registry.unregister(&AgentId(mine)).is_some(),
        "unregister must find its own agent"
    );
    assert_eq!(scratch.names(), vec!["codex-hsaq"]);
}

/// The published file must always be complete JSON, never a truncated prefix.
#[test]
fn published_registry_is_never_truncated() {
    let scratch = Scratch::new("atomic");
    for name in ["a", "b", "c", "d"] {
        scratch.register_as(name);
        let data = std::fs::read_to_string(scratch.agents_file()).unwrap();
        serde_json::from_str::<serde_json::Value>(&data)
            .unwrap_or_else(|e| panic!("registry must stay parseable after {name}: {e}"));
    }

    let leftovers = temp_leftovers(&scratch.root);
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
}

/// A corrupt registry file must not take the whole registry down.
#[test]
fn corrupt_file_does_not_panic_or_lose_new_registrations() {
    let scratch = Scratch::new("corrupt");
    std::fs::create_dir_all(scratch.root.join("agents")).unwrap();
    std::fs::write(scratch.agents_file(), b"{\"broken\": ").unwrap();

    scratch.register_as("opencode");
    assert!(
        scratch.names().contains(&"opencode".to_string()),
        "a fresh registration must still be published after a corrupt read"
    );
}

/// Interleaved writes from several instances must converge on the full set.
#[test]
fn interleaved_registrations_all_survive() {
    let scratch = Scratch::new("interleaved");
    let names = ["opencode", "claude-code", "codex-hsaq", "cursor", "aider"];
    for name in names {
        scratch.register_as(name);
    }

    let mut expected: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(scratch.names(), expected);
}

fn temp_leftovers(root: &Path) -> Vec<String> {
    std::fs::read_dir(root.join("agents"))
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".tmp"))
                .collect()
        })
        .unwrap_or_default()
}
