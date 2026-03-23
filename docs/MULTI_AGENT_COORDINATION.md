# Synapsis Multi-Agent Coordination Guide

## Architecture

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│  qwen-code  │◄───────►│   TCP       │◄───────►│  opencode   │
│  (Synapsis) │  :7438  │   Server    │  MCP    │  (Claude)   │
└─────────────┘         └─────────────┘         └─────────────┘
       │                       │                       │
       └───────────────────────┼───────────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │   SQLite Database   │
                    │   - observations    │
                    │   - task_queue      │
                    │   - agent_sessions  │
                    │   - active_locks    │
                    └─────────────────────┘
```

## Agent Roles

### qwen-code (Synapsis MCP)
- Multi-agent coordination
- TCP server management
- Event bus (WebSocket)
- Plugin ecosystem
- Testing & documentation

### opencode (PQC Core)
- Dilithium-4 implementation
- TPM integration
- Auth challenge-response
- Secure vault
- Key recycling

## Communication Patterns

### 1. Task Queue Coordination

```bash
# Create task
echo '{"method":"task_create","params":{"arguments":{"project":"synapsis","task_type":"build","payload":"Fix PQC build","priority":10}}}' | nc 127.0.0.1 7438

# Claim task
echo '{"method":"task_claim","params":{"arguments":{"session_id":"agent-session","task_type":"build"}}}' | nc 127.0.0.1 7438

# Complete task (implicit via heartbeat)
echo '{"method":"agent_heartbeat","params":{"arguments":{"session_id":"agent-session","task":"build-completed"}}}' | nc 127.0.0.1 7438
```

### 2. Distributed Locking

```bash
# Acquire lock before build
echo '{"method":"lock_acquire","params":{"arguments":{"session_id":"agent-session","lock_key":"synapsis-build","ttl":300}}}' | nc 127.0.0.1 7438

# Release lock after completion
echo '{"method":"lock_release","params":{"arguments":{"lock_key":"synapsis-build"}}}' | nc 127.0.0.1 7438
```

### 3. Context Sharing

```bash
# Save context
echo '{"method":"chunk_create","params":{"arguments":{"project":"synapsis","title":"Build Status","content":"Build completed successfully"}}}' | nc 127.0.0.1 7438

# Read context
echo '{"method":"chunk_get","params":{"arguments":{"project":"synapsis"}}}' | nc 127.0.0.1 7438
```

## Best Practices

1. **Heartbeat Frequency**: Every 25-30 seconds
2. **Lock TTL**: 300 seconds for builds, 60 seconds for quick operations
3. **Task Priority**: 10 (critical), 7-9 (high), 4-6 (medium), 1-3 (low)
4. **Context Size**: Keep chunks under 50KB for performance

## Troubleshooting

### Agent Not Responding
```bash
# Check active agents
echo '{"method":"agents_active","params":{}}' | nc 127.0.0.1 7438

# Re-register agent
echo '{"method":"session_register","params":{"arguments":{"agent_type":"agent-name","project":"synapsis"}}}' | nc 127.0.0.1 7438
```

### Lock Stuck
```bash
# Wait for TTL expiration (automatic)
# Or manually release if you have the key
echo '{"method":"lock_release","params":{"arguments":{"lock_key":"stuck-lock"}}}' | nc 127.0.0.1 7438
```

### Task Not Claimed
```bash
# Check task queue
sqlite3 ~/.local/share/synapsis/synapsis.db "SELECT * FROM task_queue WHERE status='pending' ORDER BY priority DESC;"

# Manually assign (if needed)
sqlite3 ~/.local/share/synapsis/synapsis.db "UPDATE task_queue SET status='running', agent_session_id='agent-session' WHERE task_id='task-id';"
```
