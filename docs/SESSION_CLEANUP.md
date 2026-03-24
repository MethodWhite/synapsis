# 🧹 Session Cleanup & Heartbeat System

## Overview

Synapsis now includes **automatic session cleanup** to prevent zombie sessions and resource leaks.

### Problem Solved

**Before:**
- Sessions could stay "active" forever after agent disconnects
- No automatic cleanup of stale sessions
- Manual intervention required to clean up
- Resources (locks, tasks) held by dead sessions

**After:**
- ✅ Automatic heartbeat monitoring
- ✅ Auto-cleanup of stale sessions (configurable timeout)
- ✅ Background job runs every 60 seconds
- ✅ Automatic release of locks and cancellation of tasks

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│              Agent Sessions                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Agent 1  │  │ Agent 2  │  │ Agent 3  │              │
│  │ ❤️ HB:30s│  │ ❤️ HB:30s│  │ ❌ Dead  │              │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘              │
│       │             │             │                     │
│       └─────────────┼─────────────┘                     │
│                     ▼                                   │
│       ┌─────────────────────────┐                       │
│       │  Session Cleanup Job     │                       │
│       │  - Runs every 60s       │                       │
│       │  - Timeout: 300s (5min) │                       │
│       │  - Auto-end sessions    │                       │
│       └─────────────────────────┘                       │
└─────────────────────────────────────────────────────────┘
```

---

## Configuration

### Default Settings

| Parameter | Default | Description |
|-----------|---------|-------------|
| `session_timeout_secs` | 300 (5 min) | Sessions without heartbeat for this long are stale |
| `cleanup_interval_secs` | 60 (1 min) | How often cleanup job runs |
| `require_heartbeat` | true | Agents must send heartbeats |
| `auto_end_sessions` | true | Automatically end stale sessions |

### Custom Configuration

```rust
use synapsis_core::core::session_cleanup::{SessionCleanupConfig, SessionCleanupJob};

let config = SessionCleanupConfig {
    session_timeout_secs: 600,      // 10 minutes
    cleanup_interval_secs: 120,     // 2 minutes
    require_heartbeat: true,
    auto_end_sessions: true,
};

let cleanup_job = SessionCleanupJob::new(db, config);
cleanup_job.start();
```

---

## Heartbeat Protocol

### Agent Requirements

Agents **MUST** send heartbeats to stay active:

```rust
use synapsis_core::core::session_cleanup::update_heartbeat;

// Send heartbeat every 30 seconds
update_heartbeat(&db, &session_id)?;
```

### Recommended Heartbeat Interval

| Agent Type | Recommended Interval |
|------------|---------------------|
| CLI/TUI | 30 seconds |
| MCP Server | 30 seconds |
| Background Worker | 60 seconds |
| Long-running Task | 30 seconds |

---

## Cleanup Actions

When a stale session is detected, the cleanup job:

1. **Marks session as inactive**
   ```sql
   UPDATE agent_sessions SET is_active = 0 WHERE session_id = ?
   ```

2. **Ends the session** (if `auto_end_sessions` is true)
   ```sql
   UPDATE sessions SET ended_at = ? WHERE id = ? AND ended_at IS NULL
   ```

3. **Cancels pending tasks**
   ```sql
   UPDATE task_queue 
   SET status = 'cancelled', error = 'Session terminated: agent unresponsive'
   WHERE agent_session_id = ? AND status IN ('pending', 'running')
   ```

4. **Releases locks**
   ```sql
   DELETE FROM active_locks WHERE agent_session_id = ?
   ```

---

## Manual Cleanup

### Run Cleanup Once

```rust
use synapsis_core::core::session_cleanup::{SessionCleanupJob, SessionCleanupConfig};

let config = SessionCleanupConfig::default();
let cleanup_job = SessionCleanupJob::new(db, config);

// Run cleanup manually
let stats = cleanup_job.run_once().await?;

println!(
    "Cleaned: {} sessions, {} tasks, {} locks",
    stats.cleaned,
    stats.tasks_cancelled,
    stats.locks_released
);
```

### Check Session Status

```rust
use synapsis_core::core::session_cleanup::is_session_active;

let is_active = is_session_active(&db, &session_id, 300)?;

if is_active {
    println!("Session is active");
} else {
    println!("Session is stale or doesn't exist");
}
```

---

## Monitoring

### Cleanup Statistics

The cleanup job logs statistics:

```
[SessionCleanup] Started: timeout=300s, interval=60s
[SessionCleanup] Cleaned 2 sessions, 5 tasks, 3 locks
[SessionCleanup] Stale agent detected: agent-123 (type: qwen, heartbeat: 420s ago)
```

### Session Table Schema

```sql
CREATE TABLE agent_sessions (
    session_id TEXT PRIMARY KEY,
    agent_type TEXT NOT NULL,
    is_active INTEGER DEFAULT 1,
    last_heartbeat INTEGER,  -- UNIX timestamp
    created_at INTEGER NOT NULL,
    ended_at INTEGER
);

CREATE INDEX idx_agent_sessions_active ON agent_sessions(is_active);
CREATE INDEX idx_agent_sessions_heartbeat ON agent_sessions(last_heartbeat);
```

---

## Best Practices

### For Agent Developers

1. **Send heartbeats regularly**
   ```rust
   tokio::spawn(async move {
       let mut interval = interval(Duration::from_secs(30));
       loop {
           interval.tick().await;
           update_heartbeat(&db, &session_id).ok();
       }
   });
   ```

2. **Handle cleanup gracefully**
   ```rust
   // Check if session is still valid before long operations
   if !is_session_active(&db, &session_id, 300)? {
       return Err("Session terminated".into());
   }
   ```

3. **End sessions properly**
   ```rust
   // When agent shuts down
   update_heartbeat(&db, &session_id).ok();  // Final heartbeat
   // Then end session
   session_manager.end_session(&session_id, Some("Shutdown")).ok();
   ```

### For Administrators

1. **Monitor cleanup logs**
   ```bash
   tail -f ~/.local/share/synapsis/logs/synapsis.log | grep SessionCleanup
   ```

2. **Adjust timeout for your environment**
   - Development: 600s (10 min) - agents may be idle longer
   - Production: 300s (5 min) - tighter control
   - High-security: 120s (2 min) - aggressive cleanup

3. **Manual cleanup for emergencies**
   ```bash
   # Via CLI (if available)
   synapsis session cleanup --timeout=0
   ```

---

## Troubleshooting

### Sessions Not Cleaning Up

**Check:**
1. Is cleanup job running?
   ```rust
   assert!(cleanup_job.is_running());
   ```

2. Are heartbeats being sent?
   ```sql
   SELECT session_id, last_heartbeat, 
          (strftime('%s', 'now') - last_heartbeat) as seconds_ago
   FROM agent_sessions
   WHERE is_active = 1;
   ```

3. Is timeout configured correctly?
   ```rust
   println!("Timeout: {}s", config.session_timeout_secs);
   ```

### Too Many False Positives

**Solution:** Increase timeout
```rust
let config = SessionCleanupConfig {
    session_timeout_secs: 600,  // Increase from 300 to 600
    ..Default::default()
};
```

### Agents Disconnecting Frequently

**Check:**
1. Network stability
2. Agent crash logs
3. Resource exhaustion (memory, CPU)

**Solution:** Implement reconnection logic
```rust
// Agent reconnection
if !is_session_active(&db, &session_id, 300)? {
    // Create new session
    session_id = session_manager.start_session(project, directory)?;
}
```

---

## API Reference

### `SessionCleanupConfig`

```rust
pub struct SessionCleanupConfig {
    pub session_timeout_secs: u64,
    pub cleanup_interval_secs: u64,
    pub require_heartbeat: bool,
    pub auto_end_sessions: bool,
}
```

### `SessionCleanupJob`

```rust
impl SessionCleanupJob {
    pub fn new(db: Arc<Database>, config: SessionCleanupConfig) -> Self;
    pub fn start(&self);                    // Start background job
    pub fn stop(&self);                     // Stop job
    pub fn is_running(&self) -> bool;       // Check if running
    pub async fn run_once(&self) -> Result<CleanupStats, String>;  // Manual run
}
```

### `CleanupStats`

```rust
pub struct CleanupStats {
    pub cleaned: usize,              // Sessions cleaned
    pub tasks_cancelled: usize,      // Tasks cancelled
    pub locks_released: usize,       // Locks released
    pub contexts_archived: usize,    // Contexts archived
}
```

### Helper Functions

```rust
// Update heartbeat
pub fn update_heartbeat(db: &Arc<Database>, session_id: &str) -> Result<(), String>;

// Check session status
pub fn is_session_active(db: &Arc<Database>, session_id: &str, timeout_secs: u64) -> Result<bool, String>;
```

---

## Security Considerations

### Session Hijacking Prevention

The cleanup system helps prevent session hijacking:

1. **Short timeout** - Stale sessions are quickly terminated
2. **Heartbeat requirement** - Attacker must maintain connection
3. **Auto-end sessions** - Proper cleanup prevents reuse

### Resource Exhaustion Protection

Prevents DoS via session exhaustion:

1. **Automatic cleanup** - Frees resources from dead sessions
2. **Task cancellation** - Prevents task queue flooding
3. **Lock release** - Prevents lock starvation

---

## Performance Impact

| Metric | Impact |
|--------|--------|
| Memory | <1MB for cleanup job |
| CPU | <0.1% (runs every 60s) |
| Database | 1-5 queries per cleanup |
| Latency | Negligible (async) |

---

## Future Enhancements

- [ ] Session persistence across restarts
- [ ] Graceful period before cleanup (30s warning)
- [ ] Session revival (if agent reconnects quickly)
- [ ] Per-agent-type timeouts
- [ ] Metrics export (Prometheus, etc.)

---

**Last Updated:** 2026-03-24  
**Author:** MethodWhite  
**Contact:** methodwhite@proton.me
