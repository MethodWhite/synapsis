# 🤝 Multi-Agent Coordination Guide

## Overview

Synapsis supports seamless multi-agent coordination with automatic session management, distributed locking, and task queues.

## Supported Agents

| Agent | Status | Connection Method |
|-------|--------|-------------------|
| Qwen Code | ✅ Active | MCP stdio |
| Claude Code | ✅ Supported | MCP stdio |
| Cursor | ✅ Supported | MCP bridge |
| Windsurf | ✅ Supported | MCP bridge |
| VS Code + Copilot | ✅ Supported | MCP extension |
| Gemini CLI | ✅ Supported | MCP bridge |
| OpenCode | ✅ Active | MCP stdio |

## Architecture

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Qwen 1    │  │  Claude     │  │   Cursor    │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘
       │                │                │
       └────────────────┴────────────────┘
                        │
               ┌────────▼────────┐
               │  Synapsis MCP   │
               │     Bridge      │
               └────────┬────────┘
                        │
               ┌────────▼────────┐
               │  Synapsis Core  │
               │  (SQLite + FTS5)│
               └─────────────────┘
```

## Session Management

### Auto-Reconnect

Synapsis automatically reconnects sessions that were inactive for < 30 minutes:

```python
# Agent sends heartbeat
agent_heartbeat(session_id="my-session", status="idle")

# Synapsis detects inactive session and auto-reconnects
# Returns: reconnected=True if session was restored
```

### Session Lifecycle

1. **Register**: `mem_session_start(id="unique-id", project="my-project")`
2. **Heartbeat**: `agent_heartbeat(session_id="unique-id", status="working")`
3. **Complete**: `mem_session_end(id="unique-id", summary="...")`

## Distributed Locking

### Acquire Lock

```python
mem_lock_acquire(lock_key="file:src/main.rs", ttl=60)
# Returns: {"acquired": true, "expires_at": timestamp}
```

### Release Lock

```python
mem_lock_release(lock_key="file:src/main.rs")
# Returns: {"released": true}
```

### Lock Verification

Synapsis verifies:
- Session exists
- Session is active
- Session owns the lock

## Task Queue

### Create Task

```python
task_create(
    project="my-project",
    task_type="code_review",
    payload="Review PR #123",
    priority=8
)
# Auto-assigns to idle agent if available
```

### Claim Task

```python
task_claim(session_id="my-session")
# Returns: {"task": {...}} or {"task": null}
```

## Agent Coordination Patterns

### Pattern 1: Parallel Work

```
Agent 1: Creates task → Task Queue
Agent 2: Claims task → Works on task
Agent 3: Monitors → Coordinates
```

### Pattern 2: Resource Sharing

```
Agent 1: Acquires lock → Edits file → Releases lock
Agent 2: Waits for lock → Acquires → Edits → Releases
```

### Pattern 3: Context Sharing

```
Agent 1: Saves memory → Synapsis DB
Agent 2: Searches memory → Gets context
Agent 3: Updates memory → Shared knowledge
```

## Best Practices

1. **Always send heartbeats** every 30 seconds
2. **Release locks promptly** after use
3. **Use descriptive task payloads**
4. **Set appropriate TTLs** for locks
5. **Handle reconnection gracefully**

## Troubleshooting

### Issue: Lock not releasing

**Solution:** Check if session is still active. Inactive sessions have locks auto-released.

### Issue: Task not assigned

**Solution:** Ensure agent sent heartbeat with status="idle" within last 60 seconds.

### Issue: Session disconnected

**Solution:** Reconnect with same session_id within 30 minutes for auto-reconnect.

---

**Last Updated:** 2026-03-22
