# 🖥️ Synapsis CLI Guide

Complete guide for using Synapsis command-line interface.

---

## 📦 Installation

### Quick Install (Linux/macOS)

```bash
# Clone and build
git clone https://github.com/methodwhite/synapsis.git
cd synapsis
cargo build --release

# Optional: Install to local bin
cargo install --path .
# Or manually copy binary
cp target/release/synapsis ~/.local/bin/
```

### Windows (PowerShell)

```powershell
# Clone repository
git clone https://github.com/methodwhite/synapsis.git
cd synapsis

# Build release
cargo build --release

# Binary location
.\target\release\synapsis-mcp.exe
```

---

## 🚀 Quick Start

### 1. Start MCP Server (Standard Input/Output)

```bash
# Local MCP server (for IDE integration)
./target/release/synapsis-mcp
```

### 2. Start TCP Server (Multi-Agent Mode)

```bash
# Start TCP server on port 7438
./target/release/synapsis --tcp 7438

# Start with secure mode (PQC authentication)
./target/release/synapsis --tcp 7438 --secure

# Custom bind address
./target/release/synapsis --tcp 7438 --addr 0.0.0.0
```

### 3. Connect MCP Client

```bash
# Connect to TCP server
./target/release/synapsis-mcp --tcp-addr localhost:7438

# Connect with secure mode
./target/release/synapsis-mcp --tcp-addr localhost:7438 --secure
```

---

## 📋 Command Reference

### Main Binary (`synapsis`)

| Command | Description | Example |
|---------|-------------|---------|
| `--tcp <port>` | Start TCP server | `synapsis --tcp 7438` |
| `--secure` | Enable PQC security | `synapsis --tcp 7438 --secure` |
| `--addr <ip>` | Bind address | `synapsis --tcp 7438 --addr 0.0.0.0` |
| `--help` | Show help | `synapsis --help` |
| `--version` | Show version | `synapsis --version` |

### MCP Binary (`synapsis-mcp`)

| Command | Description | Example |
|---------|-------------|---------|
| `--tcp-addr <addr>` | Connect to TCP server | `synapsis-mcp --tcp-addr localhost:7438` |
| `--secure` | Use PQC secure connection | `synapsis-mcp --tcp-addr :7438 --secure` |
| `--help` | Show help | `synapsis-mcp --help` |

---

## 🔧 Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SYNAPSIS_DATA_DIR` | Data directory | `~/.local/share/synapsis` |
| `SYNAPSIS_LOG_LEVEL` | Log level (debug, info, warn, error) | `info` |
| `SYNAPSIS_TCP_PORT` | Default TCP port | `7438` |
| `SYNAPSIS_SECURE_MODE` | Enable secure mode | `false` |
| `XDG_DATA_HOME` | XDG data directory | `~/.local/share` |

### Configuration Files

#### Data Directory Structure

```
~/.local/share/synapsis/
├── synapsis.db          # SQLite database
├── synapsis.db-shm      # SQLite shared memory
├── synapsis.db-wal      # SQLite write-ahead log
├── sessions.json        # Active sessions
├── skills.json          # Agent skills
├── resource_limits.json # Resource management
├── vault/               # Encrypted vault
│   └── ...
└── logs/                # Application logs
    └── synapsis.log
```

#### Resource Limits Configuration

Create `~/.local/share/synapsis/resource_limits.json`:

```json
{
  "global": {
    "max_total_tasks": 20,
    "max_cpu_percent": 70.0,
    "max_memory_percent": 80.0,
    "high_load_threshold": 3.5,
    "enable_adaptive_throttling": true
  },
  "agent_limits": {
    "opencode": {
      "max_concurrent_tasks": 3,
      "max_cpu_percent": 50.0,
      "max_memory_mb": 2048,
      "priority": 8
    },
    "qwen": {
      "max_concurrent_tasks": 2,
      "max_cpu_percent": 40.0,
      "max_memory_mb": 1536,
      "priority": 9
    },
    "claude": {
      "max_concurrent_tasks": 2,
      "max_cpu_percent": 40.0,
      "max_memory_mb": 1536,
      "priority": 9
    }
  }
}
```

---

## 🤝 Multi-Agent Setup

### Scenario 1: Single Agent (Local)

```bash
# Start MCP server locally
./target/release/synapsis-mcp
```

### Scenario 2: Multiple Agents (TCP)

**Terminal 1 - Start Server:**
```bash
./target/release/synapsis --tcp 7438
```

**Terminal 2 - Agent 1 (Qwen):**
```bash
./target/release/synapsis-mcp --tcp-addr localhost:7438
```

**Terminal 3 - Agent 2 (OpenCode):**
```bash
./target/release/synapsis-mcp --tcp-addr localhost:7438
```

### Scenario 3: Secure Multi-Agent (PQC)

**Terminal 1 - Start Secure Server:**
```bash
./target/release/synapsis --tcp 7438 --secure
```

**Terminal 2 - Secure Agent:**
```bash
./target/release/synapsis-mcp --tcp-addr localhost:7438 --secure
```

---

## 🛠️ MCP Tools Usage

### Save Observation

```json
{
  "method": "mem_save",
  "params": {
    "arguments": {
      "title": "Security Vulnerability Found",
      "content": "SQL injection vulnerability in user login endpoint",
      "project": "web-audit",
      "observation_type": 1
    }
  }
}
```

### Search Memory

```json
{
  "method": "mem_search",
  "params": {
    "arguments": {
      "query": "SQL injection",
      "project": "web-audit",
      "limit": 10
    }
  }
}
```

### Get Context

```json
{
  "method": "mem_context",
  "params": {
    "arguments": {
      "query": "authentication bypass",
      "limit": 5
    }
  }
}
```

### Task Management

```json
{
  "method": "task_create",
  "params": {
    "arguments": {
      "title": "Review authentication module",
      "description": "Check for vulnerabilities in auth flow"
    }
  }
}
```

```json
{
  "method": "task_claim",
  "params": {
    "arguments": {
      "session_id": "qwen-abc123-1234567890"
    }
  }
}
```

### Distributed Locking

```json
{
  "method": "mem_lock_acquire",
  "params": {
    "arguments": {
      "resource": "database-migration",
      "timeout": 300
    }
  }
}
```

```json
{
  "method": "mem_lock_release",
  "params": {
    "arguments": {
      "resource": "database-migration"
    }
  }
}
```

### Web Research

```json
{
  "method": "web_research",
  "params": {
    "arguments": {
      "query": "CVE-2026 latest vulnerabilities"
    }
  }
}
```

### CVE Search

```json
{
  "method": "cve_search",
  "params": {
    "arguments": {
      "cve_id": "CVE-2026-12345"
    }
  }
}
```

### Security Classification

```json
{
  "method": "security_classify",
  "params": {
    "arguments": {
      "text": "Buffer overflow in parse_header function",
      "context": "security"
    }
  }
}
```

---

## 🧪 Testing

### Run All Tests

```bash
cargo test
```

### Run Specific Test Suite

```bash
# Database tests
cargo test --test synapsis_database_tests

# PQC tests
cargo test --test synapsis_pqc_integration

# Stress tests
cargo test --test stress_tests

# MCP integration tests
cargo test --test mcp_integration_tests
```

### Run with Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html
```

---

## 🐛 Troubleshooting

### Issue: "Address already in use"

```bash
# Kill process on port 7438
lsof -ti:7438 | xargs kill -9

# Or use different port
synapsis --tcp 7439
```

### Issue: "Database locked"

```bash
# Remove SQLite WAL files
rm ~/.local/share/synapsis/synapsis.db-*

# Restart server
```

### Issue: "PQC initialization failed"

```bash
# Rebuild with security feature
cargo build --release --features security

# Check dependencies
cargo audit
```

### Issue: High CPU usage

```bash
# Check resource limits
cat ~/.local/share/synapsis/resource_limits.json

# Enable adaptive throttling
# Edit resource_limits.json and set:
# "enable_adaptive_throttling": true
```

---

## 📊 Monitoring

### Check Active Sessions

```bash
# Via MCP tool
{
  "method": "mem_stats",
  "params": {}
}
```

### View Event Timeline

```bash
{
  "method": "mem_timeline",
  "params": {
    "arguments": {
      "limit": 20
    }
  }
}
```

### Agent Heartbeat

```bash
{
  "method": "agent_heartbeat",
  "params": {
    "arguments": {
      "session_id": "qwen-abc123-1234567890",
      "status": "busy",
      "task": "code-review"
    }
  }
}
```

---

## 🔐 Security Best Practices

1. **Always use secure mode in production:**
   ```bash
   synapsis --tcp 7438 --secure
   ```

2. **Restrict bind address for local-only:**
   ```bash
   synapsis --tcp 7438 --addr 127.0.0.1
   ```

3. **Set strong SQLCipher key:**
   ```bash
   export SYNAPSIS_DB_KEY="your-strong-password-here"
   ```

4. **Regular security audits:**
   ```bash
   cargo audit
   cargo clippy --features security -- -D warnings
   ```

---

## 📚 Additional Resources

- [Security Documentation](docs/SECURITY.md)
- [MCP Protocol Details](docs/MCP.md)
- [Architecture Overview](docs/ARCHITECTURE.md)
- [Multi-Agent Guide](docs/MULTI-AGENT.md)
- [Plugin System](docs/PLUGIN_SYSTEM_GUIDE.md)

---

**Last Updated:** 2026-03-26
**Version:** 0.1.0
