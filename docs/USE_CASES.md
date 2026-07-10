# Synapsis Use Cases

Real-world workflows demonstrating Synapsis cross-platform memory, session continuity,
and multi-agent coordination.

---

## Use Case 1: Cross-Platform Memory

**Save from OpenCode, retrieve from Cursor.**

### Scenario

A developer works in OpenCode during a morning session, saving key observations.
After lunch, they open Cursor on the same project and need those observations without
any manual export/import.

### Step-by-Step

```bash
# ── In OpenCode (morning session) ──

# Start a session
mem_session_start project=my-app

# Save observations during work
mem_save title="API design decision" content="Use REST over GraphQL for user service" project=my-app
mem_save title="Bug found" content="Null pointer in auth middleware when JWT is expired" project=my-app type=bugfix

# End session
mem_session_end summary="Reviewed API design, fixed auth bug"

# ── In Cursor (afternoon session) ──

# Search for past observations — they appear immediately
mem_search query="auth middleware"

# Get recent context — includes morning's OpenCode session
mem_context

# Timeline shows both sessions chronologically
mem_timeline
```

### Technical Architecture

```
OpenCode ──→ MCP stdio ──→ Synapsis Server (port 7438)
                                    │
                              Session Bridge
                                    │
                              Shared SQLite DB
                                    │
Cursor ────→ MCP HTTP/SSE ──→ Synapsis Server (port 7438)
```

**Components involved:** Memory Engine, Session Bridge, SQLite+FTS5, MCP Transport

### Expected Output

```
> mem_search query="auth middleware"
[1] ID: 42 | "Bug found" | type: bugfix | 2026-07-10T09:15:00Z
    Null pointer in auth middleware when JWT is expired
    Source: opencode session my-app-abc123
```

---

## Use Case 2: Session Continuity

**Start in Gemini CLI, continue in VS Code.**

### Scenario

A developer starts a debugging session in Gemini CLI on a headless server, then
opens VS Code on their workstation to continue working. Both tools share the
same session ID and all observations are collected under one session.

### Step-by-Step

```bash
# ── In Gemini CLI (server) ──

mem_session_start project=my-app

# Investigate and save findings
mem_save title="Memory leak detected" content="Connection pool not releasing idle connections" project=my-app

# ── In VS Code (workstation) ──

# VS Code detects the existing active session
shared_sessions_by_project project=my-app
# Returns: session-id-abc123, started 10:30, 3 observations

# Continue under the same session
mem_session_start project=my-app session_id=session-id-abc123

# Add more observations
mem_save title="Fix applied" content="Added .releaseIdle() call after each query" project=my-app

# End from either tool — summary includes ALL observations
mem_session_end session_id=session-id-abc123 summary="Fixed memory leak in connection pool"
```

### Technical Architecture

```
Gemini CLI ──→ MCP HTTP ──→ Synapsis Session Bridge
                                    │
                            shared_sessions_by_project
                                    │
VS Code ────→ MCP HTTP ──→ Synapsis Session Bridge
```

**Components involved:** Session Bridge, Session Manager, `shared_sessions_by_project`

### Expected Output

```
> shared_sessions_by_project project=my-app
Active sessions for "my-app":
  session-id-abc123
    Agent: gemini-cli (host: server-01)
    Started: 2026-07-10T10:30:00Z
    Observations: 3
    Status: active

> mem_session_end session_id=session-id-abc123 summary="Fixed memory leak"
Session ended: session-id-abc123
  Agent: gemini-cli, vscode
  Started: 2026-07-10T10:30:00Z
  Ended:   2026-07-10T11:45:00Z
  Duration: 1h 15m
  Observations: 4
  Summary: Fixed memory leak in connection pool
```

---

## Use Case 3: Auto-Discovery + Auto-Config

**Plug and play: new tool detected automatically.**

### Scenario

A developer installs a new AI coding tool (e.g., Cline). Synapsis detects the new
binary, generates the MCP configuration automatically, and registers it in the
Session Bridge — all without manual setup.

### Step-by-Step

```bash
# ── Before installing new tool ──

# Start the auto-config watcher (runs in background)
synapsis-autoconfig --apply --watch

# ── User installs Cline ──
# (Cline binary becomes available in PATH)

# Auto-config detects it and generates MCP config:
# [synapsis-autoconfig] Detected new tool: cline
# [synapsis-autoconfig] Writing ~/.config/cline/mcp.json
# [synapsis-autoconfig] Configuring Cline ...
# [synapsis-autoconfig] ok Cline configured

# Verify all configured tools
synapsis-autoconfig --apply
# Synapsis MCP Auto-Config
#   Detected tools: opencode claude cursor windsurf gemini cline
#   Configured: OpenCode, Claude Code, Cursor, Windsurf, Gemini CLI, Cline

# Register new tool with Session Bridge
scripts/session-autolink.sh --once
# [synapsis-autolink] Detected agents:
#   - cline
# [synapsis-autolink] Registered: cline on my-app (session: ...)
```

### Technical Architecture

```
synapsis-autoconfig --apply --watch
        │
        ├── Detects new binaries via PATH/pgrep
        ├── Generates MCP JSON config files
        └── Registers agent in Agent Registry
                │
        Session Bridge ←→ Agent Registry
```

**Components involved:** Auto-Config, Platform Catalog, Agent Registry, Session Bridge, MCP Transport

### Expected Output

```
> synapsis-autoconfig --apply
Synapsis MCP Auto-Config
  Binary: /usr/local/bin/synapsis-mcp
  Mode:   APPLY

Detecting AI development tools...
  Detected: opencode, claude, cursor, windsurf, gemini, cline

Generating MCP configurations...
  ok OpenCode configured
  ok Claude Code configured
  ok Cursor configured
  ok Windsurf configured
  ok Gemini CLI configured
  ok Cline configured

Configuration complete.
```

---

## Use Case 4: Secure Production Deployment

**HTTPS + API keys for team use.**

### Scenario

A team deploys Synapsis as a shared memory server for their engineering org.
The admin enables TLS encryption and API key authentication, and sets up
the audit log for compliance.

### Step-by-Step

```bash
# ── Admin: Generate certs and keys ──

# Generate TLS certificate and key
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem \
  -days 365 -nodes -subj "/CN=synapsis.internal.example.com"

# Generate API keys
SYNAPSIS_API_KEYS="key-admin-$(openssl rand -hex 16),key-dev-$(openssl rand -hex 16)"

# Store API keys for clients
echo "$SYNAPSIS_API_KEYS" > ~/.synapsis_api_keys
chmod 600 ~/.synapsis_api_keys

# ── Admin: Start server ──

# Method 1: CLI flags
synapsis --tls-cert cert.pem --tls-key key.pem

# Method 2: Environment variables
export SYNAPSIS_TLS_CERT=cert.pem
export SYNAPSIS_TLS_KEY=key.pem
export SYNAPSIS_API_KEYS=key-admin-...,key-dev-...
export SYNAPSIS_PORT=7438
synapsis

# ── Client: Connect with API key ──

export SYNAPSIS_API_KEYS=key-dev-abc123
export SYNAPSIS_URL=https://synapsis.internal.example.com:7438

# All MCP tools work encrypted
mem_save title="Deploy config" content="Set max_connections=100 in prod" project=my-app

# ── Admin: Audit ──

# Check who saved what
audit_log
```

### Technical Architecture

```
                          ┌──────────────────┐
Client ──HTTPS/TLS──▶    │   Synapsis Server │
(API Key in header)      │                   │
                          │  TLS Termination  │
                          │  API Key Auth     │
                          │  Audit Log        │
                          │  Rate Limiter     │
                          └────────┬──────────┘
                                   │
                            Encrypted SQLite
                            (SQLCipher via SYNAPSIS_DB_KEY)
```

**Components involved:** TLS Transport, API Key Auth, Audit Log, Rate Limiter, SQLCipher

### Expected Output

```
╔══════════════════════════════════════════════════════════╗
║  Synapsis v0.x.x - Multi-Agent MCP Server               ║
║  Transport: HTTPS/SSE (port 7438)                       ║
║  Multi-Agent: enabled                                   ║
╚══════════════════════════════════════════════════════════╝
[Synapsis] TLS configured (cert: cert.pem)
```

---

## Use Case 5: MCP Discovery Network

**Find and connect to remote Synapsis nodes.**

### Scenario

Multiple Synapsis instances run on a team's local network. Using mDNS discovery,
they find each other and can share memory via the bridge, enabling a team-wide
collective memory.

### Step-by-Step

```bash
# ── Each team member starts their Synapsis instance ──
synapsis

# ── Discover other nodes on the network ──
discovery_scan

# Sample output:
# Found 3 Synapsis nodes:
#   alice-dev (192.168.1.50:7438) - 5 agents, 142 observations
#   bob-workshop (192.168.1.72:7438) - 3 agents, 89 observations
#   ci-runner (192.168.1.100:7438) - 2 agents, 312 observations

# ── Query remote node via MCP bridge ──
mcp_call server_url=http://192.168.1.72:7438 tool_name=mem_search arguments='{"query":"deployment"}'

# ── Remote observations appear in local context ──
# (Session Bridge syncs across discovered nodes automatically)
mem_context
```

### Technical Architecture

```
Node A (Alice)         Node B (Bob)          Node C (CI)
    │                       │                     │
    │  mDNS broadcast       │  mDNS broadcast     │
    └───────────────────────┴─────────────────────┘
                            │
                    Discovery Layer
                    (mdns-sd library)
                            │
                    Session Bridge
                            │
                    mcp_call (cross-node)
```

**Components involved:** Discovery Layer (mDNS), Session Bridge, MCP Call, Agent Registry

### Expected Output

```
> discovery_scan
Scanning network for Synapsis nodes...
  ✓ alice-dev (192.168.1.50:7438) — latency 2ms
  ✓ bob-workshop (192.168.1.72:7438) — latency 3ms
  ✓ ci-runner (192.168.1.100:7438) — latency 1ms

> mcp_call server_url=http://192.168.1.72:7438 tool_name=mem_stats
Observations on bob-workshop: 89
Sessions: 12
Agents: cursor, opencode, claude-code
Last activity: 2026-07-10T14:30:00Z
```

---

## Use Case 6: Chinese Platform Integration

**Use Synapsis with DeepSeek, Qwen, Kimi.**

### Scenario

A developer toggles between Western tools (OpenCode, Claude Code) and Chinese
AI platforms (DeepSeek, Qwen/通义, Kimi/月之暗面). Synapsis detects all platforms
via environment variables or auto-config and provides unified memory across them.

### Step-by-Step

```bash
# ── Set API keys for Chinese platforms ──
export DEEPSEEK_API_KEY=sk-...
export DASHSCOPE_API_KEY=sk-...          # Qwen/通义
export MOONSHOT_API_KEY=sk-...           # Kimi/月之暗面

# ── Synapsis detects them and registers the platforms ──
synapsis-autoconfig --apply
# Detected platforms: opencode, cursor, deepseek, qwen, kimi

# ── Save observations from DeepSeek ──
mem_save title="模型选择决策" content="使用 Qwen2.5-72B 用于代码生成，DeepSeek-V3 用于推理" project=my-app

# ── Retrieve in OpenCode (Western tool) ──
mem_search query="模型选择"
# Returns the observation saved from DeepSeek

# ── Save from Qwen, retrieve from Kimi ──
# (All platforms share the same Session Bridge)
mem_context
```

### Technical Architecture

```
┌──────────────── Western ────────────────┐
│  OpenCode  Claude Code  Cursor          │
└──────────────┬──────────────────────────┘
               │
        ┌──────▼──────┐
        │   Synapsis  │
        │   Session   │
        │   Bridge    │
        └──────┬──────┘
               │
┌──────────────▼──────────────────────────┐
│  DeepSeek  Qwen/通义  Kimi/月之暗面     │
└──────────────── Chinese ────────────────┘
```

**Components involved:** Platform Catalog (30+ platforms), Session Bridge, Memory Engine,
Auto-Config, MCP Transport

### Expected Output

```
> synapsis-autoconfig --apply
Synapsis MCP Auto-Config
  Detected tools: opencode, cursor, deepseek, qwen, kimi

> mem_search query="模型选择"
[1] ID: 17 | "模型选择决策" | 2026-07-10T16:00:00Z
    使用 Qwen2.5-72B 用于代码生成，DeepSeek-V3 用于推理
    Source: deepseek session
```

---

## Use Case 7: CI/CD Pipeline Memory

**Synapsis in GitHub Actions.**

### Scenario

A CI/CD pipeline uses Synapsis MCP to save observations at each workflow step.
After the run, developers review the memory timeline to understand build failures,
test results, and deployment status — with knowledge persisting across workflow runs.

### Step-by-Step

```yaml
# .github/workflows/ci.yml
name: CI with Synapsis Memory

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Synapsis
        run: |
          curl -sfL https://raw.githubusercontent.com/MethodWhite/synapsis/main/install.sh | bash
          echo "$HOME/.synapsis/bin" >> $GITHUB_PATH

      - name: Start Synapsis
        run: |
          synapsis-mcp &
          sleep 2

      - name: Build
        run: |
          cargo build --release
          # Save build result to memory
          curl -s -X POST http://127.0.0.1:7438/message \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"mem_save","arguments":{"title":"Build result","content":"Build succeeded","project":"synapsis","type":"discovery"}},"id":1}'

      - name: Test
        run: |
          cargo test
          curl -s -X POST http://127.0.0.1:7438/message \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"mem_save","arguments":{"title":"Test results","content":"All 142 tests passed","project":"synapsis","type":"discovery"}},"id":1}'

      - name: Deploy
        run: |
          curl -s -X POST http://127.0.0.1:7438/message \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"mem_save","arguments":{"title":"Deployment","content":"Deployed to staging","project":"synapsis","type":"discovery"}},"id":1}'

      - name: Upload memory artifacts
        uses: actions/upload-artifact@v4
        with:
          name: synapsis-memory
          path: ~/.local/share/synapsis/
```

```bash
# ── Developer reviews memory after CI run ──

# View timeline of all CI runs
mem_timeline project=synapsis

# Search for failures across runs
mem_search query="test failed" project=synapsis

# Get recent context
mem_context

# Memory persists — next CI run builds on previous knowledge
```

### Technical Architecture

```
GitHub Actions Runner
        │
  ┌─────▼─────┐
  │ Synapsis  │  mem_save at each step
  │ MCP stdio │
  └─────┬─────┘
        │
  ┌─────▼─────┐
  │  Network  │
  │  Volume   │ ←── persistent across runs
  └───────────┘
        │
Developer ──→ mem_timeline / mem_search
```

**Components involved:** MCP stdio, Memory Engine, SQLite+FTS5, Session Bridge

### Expected Output

```
> mem_timeline project=synapsis
CI Run #142 (2026-07-10)
  ├─ Build result: Succeeded (2m 14s)
  ├─ Test results: All 142 tests passed
  └─ Deployment: Deployed to staging

CI Run #141 (2026-07-09)
  ├─ Build result: FAILED (linker error)
  └─ Test results: Skipped

CI Run #140 (2026-07-08)
  ├─ Build result: Succeeded (2m 10s)
  ├─ Test results: 140 passed, 2 failed
  └─ Deployment: NOT DEPLOYED
```

---

## Use Case 8: Backup and Disaster Recovery

**Encrypted backup with integrity verification.**

### Scenario

An admin maintains a Synapsis deployment and needs to ensure data durability.
They set up periodic encrypted backups, verify database integrity, prune old data,
and practice restore procedures — all via MCP tools from any connected client.

### Step-by-Step

```bash
# ── Periodic backup ──
db_backup path=/backup/synapsis-$(date +%Y%m%d).db

# ── Verify integrity ──
db_integrity
# ok database integrity check passed

# ── Health check ──
db_health
# Database: synapsis.db
# Size: 42 MB
# Observations: 1,234
# Sessions: 89
# WAL mode: active
# Integrity: PASSED

# ── Prune old data (soft-delete observations older than 90 days) ──
db_prune older_than_days=90
# Pruned 23 observations (older than 90 days)

# ── Reclaim space ──
db_vacuum
# Database size reduced from 42 MB to 38 MB

# ── Restore from backup (via shell) ──
cp /backup/synapsis-20260701.db ~/.local/share/synapsis/synapsis.db

# ── Verify restored data ──
db_integrity
db_health
mem_stats
# Observations: 1,211 (pre-prune state restored)
```

### Technical Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Any MCP    │────▶│  db_backup  │────▶│  Backup     │
│  Client     │     │  db_prune   │     │  File (.db) │
│             │     │  db_vacuum  │     │             │
│             │     │  db_health  │     │  Encrypted  │
│             │     │  db_integrity│    │  (optional) │
└─────────────┘     └──────┬──────┘     └─────────────┘
                           │
                    ┌──────▼──────┐
                    │  SQLite DB  │
                    │  (WAL mode) │
                    └─────────────┘
```

**Components involved:** Database Manager, Backup System, Integrity Checker, Prune Engine, Vacuum

### Expected Output

```
> db_backup path=/backup/synapsis-20260710.db
Backup created: /backup/synapsis-20260710.db
  Size: 38 MB
  Observations: 1,234
  Checksum: a1b2c3d4e5f6...

> db_integrity
Running PRAGMA integrity_check...
  ok database integrity check passed

> db_prune older_than_days=90
Pruning observations older than 90 days...
  Deleted 23 observations (soft-delete)
  Reclaimable space: ~4 MB
  Run db_vacuum to reclaim disk space
```

---

## Architecture Summary

All use cases share these core Synapsis components:

| Component | Role | Use Cases |
|-----------|------|-----------|
| **Memory Engine** | Save, search, retrieve observations | 1, 2, 6, 7 |
| **Session Bridge** | Cross-tool session sharing | 1, 2, 5, 6 |
| **Session Manager** | Session lifecycle (start/end/summary) | 2 |
| **Auto-Config** | Tool detection & MCP config generation | 3 |
| **Platform Catalog** | 30+ registered platforms (Western + Chinese) | 3, 6 |
| **Agent Registry** | Register, list, discover agents | 3, 5 |
| **Discovery Layer** | mDNS for network node discovery | 5 |
| **Security Layer** | TLS, API keys, PQC, audit log | 4 |
| **Database Manager** | Backup, integrity, prune, vacuum | 8 |
| **Watchdog** | File system integrity monitoring | 4, 8 |

---

> All commands assume Synapsis is running (`synapsis` for HTTP/SSE or `synapsis-mcp` for stdio).
> MCP tools can be called from any connected client (OpenCode, Cursor, VS Code, Gemini CLI, etc.).
