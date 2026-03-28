# 📁 Synapsis Project Structure

This document explains the organization of files and directories in the Synapsis repository.

---

## 🏗️ Root Directory Structure

```
synapsis/
├── src/                    # Source code (Rust)
├── docs/                   # Documentation
├── tests/                  # Integration tests
├── examples/               # Usage examples
├── plugins/                # Dynamic plugins (.so/.dylib)
├── scripts/                # Helper scripts
├── systemd/                # Systemd service files
├── .github/                # GitHub workflows and templates
├── Cargo.toml              # Rust package manifest
├── rust-toolchain.toml     # Rust version specification
├── rustfmt.toml            # Code formatting configuration
├── .clippy.toml            # Clippy linting configuration
├── README.md               # Main documentation (START HERE)
├── CHANGELOG.md            # Version history
├── SECURITY.md             # Security policy
├── LICENSE                 # Business Source License 1.1
└── .gitignore              # Git ignore rules
```

---

## 📂 Source Code (`src/`)

```
src/
├── main.rs                 # Main binary entry point (TCP server)
├── lib.rs                  # Library root
├── bin/                    # Additional binaries
│   ├── mcp.rs              # MCP server (stdio bridge)
│   ├── server.rs           # HTTP REST API
│   ├── ollama.rs           # Ollama integration
│   ├── http.rs             # HTTP client tools
│   └── bench.rs            # Performance benchmarks
├── domain/                 # Domain layer (business logic)
│   ├── entities/           # Core entities (Observation, Session, Task)
│   ├── types/              # Type definitions
│   ├── errors/             # Error types
│   ├── crypto/             # Cryptography traits
│   └── plugin_loader.rs    # Dynamic plugin loading
├── core/                   # Core layer (application logic)
│   ├── auth/               # Authentication (PQC, challenge-response)
│   ├── orchestrator.rs     # Multi-agent coordination
│   ├── session_manager.rs  # Session lifecycle
│   ├── rate_limiter.rs     # Rate limiting
│   ├── recycle/            # Soft-delete recycle bin
│   └── vault/              # Encrypted storage
├── infrastructure/         # Infrastructure layer
│   ├── database/           # SQLite with SQLCipher
│   ├── network/            # Network utilities
│   ├── plugin.rs           # Plugin system
│   └── shared_state.rs     # Shared state management
├── presentation/           # Presentation layer
│   ├── mcp/                # MCP protocol implementation
│   │   ├── tcp.rs          # TCP server
│   │   └── secure_tcp.rs   # Secure TCP with PQC
│   ├── api/                # REST API endpoints
│   └── cli.rs              # Command-line interface
├── tools/                  # MCP tools implementation
│   ├── memory_tools.rs     # mem_save, mem_search, etc.
│   ├── web_tools.rs        # web_research, cve_search
│   └── security_tools.rs   # security_classify
├── config.rs               # Configuration management
├── session_cleanup.rs      # Session cleanup module
└── updater.rs              # Auto-update mechanism
```

---

## 📚 Documentation (`docs/`)

```
docs/
├── README.md               # Documentation index
├── CLI_GUIDE.md            # Command-line interface guide
├── SECURITY.md             # Security model (10-star system)
├── MCP.md                  # MCP protocol details
├── ARCHITECTURE.md         # Architecture deep-dive
├── MULTI-AGENT.md          # Multi-agent coordination
├── API.md                  # API reference
├── PLUGIN_SYSTEM_GUIDE.md  # Plugin development
├── ENGRAM_VS_SYNAPSIS.md   # Comparison with Engram
└── internal/               # Internal development docs
    ├── PROJECT_MANAGEMENT.md
    ├── TODO.md
    ├── SECURITY_FIXES.md
    └── ... (other internal docs)
```

---

## 🧪 Tests (`tests/`)

```
tests/
├── synapsis_database_tests.rs    # Database integration tests
├── synapsis_pqc_integration.rs   # PQC cryptography tests
├── stress_tests.rs               # Load and stress tests
├── mcp_integration_tests.rs      # MCP protocol tests
├── synapsis_skills_tests.rs      # Skills system tests
├── orchestration_tests.rs        # Multi-agent orchestration
└── multi-agent-orchestrator_test.sh  # Shell script test
```

---

## 🔧 Configuration Files

### `Cargo.toml`
Rust package manifest with dependencies and features.

### `rust-toolchain.toml`
```toml
[toolchain]
channel = "1.88.0"
components = ["rustfmt", "clippy", "rust-analyzer"]
```

Ensures all developers use the same Rust version.

### `rustfmt.toml`
Code formatting rules for consistent style.

### `.clippy.toml`
Clippy linting configuration with security-focused rules.

---

## 📝 Documentation Hierarchy

### Public-Facing (Root)
- **README.md** - Start here for installation and usage
- **CHANGELOG.md** - Version history
- **SECURITY.md** - Security policy and disclosures
- **LICENSE** - BUSL-1.1 license terms

### Technical Documentation (`docs/`)
- **CLI_GUIDE.md** - Complete CLI reference
- **SECURITY.md** - Security model details
- **MCP.md** - MCP protocol specification
- **ARCHITECTURE.md** - System architecture

### Internal Documentation (`docs/internal/`)
Development notes, security reports, and planning documents.

---

## 🎯 Quick Navigation

| I want to... | Go to... |
|-------------|----------|
| Install Synapsis | `README.md` → Quick Start |
| Learn CLI commands | `docs/CLI_GUIDE.md` |
| Understand security | `docs/SECURITY.md` |
| Develop plugins | `docs/PLUGIN_SYSTEM_GUIDE.md` |
| Run tests | `tests/` directory |
| Check API | `docs/API.md` |
| View internal docs | `docs/internal/` |

---

## 📦 Binary Outputs

After building (`cargo build --release`):

```
target/release/
├── synapsis              # Main TCP server (4.5MB)
├── synapsis-mcp          # MCP bridge (7.1MB)
├── synapsis-server       # HTTP REST API
├── synapsis-ollama       # Ollama integration
└── synapsis-http         # HTTP client tools
```

---

## 🔒 License

**BUSL-1.1** - Personal, educational, and research use only.

Commercial use requires separate license: methodwhite@proton.me

---

**Last Updated:** 2026-03-27
**Version:** 0.1.0
