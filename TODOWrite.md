# 📋 Synapsis TODO List

## Priority Tasks (Auto-Assigned to Ollama Sub-Agents)

### 🔥 CRITICAL (Priority 10)

- [ ] **Security 10/10 Verification**
  - Assigned to: deepseek-r1-i1
  - Status: ⚠️ IN PROGRESS
  - Notes: 6/7 vulnerabilities mitigated. PQC implementation incomplete, RNG insecure, SQLCipher not integrated.

- [ ] **Implement PQC Cryptography**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Replace AES-256-GCM stub with real PQC (CRYSTALS-Kyber-512/Dilithium-4) or update documentation

- [ ] **Fix Insecure RNG**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Replace time-based RNG in security.rs and tpm.rs with cryptographic RNG (OsRng/getrandom)

- [ ] **Integrate SQLCipher Encryption**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Integrate EncryptedDB into Database or remove unused module

- [ ] **GitHub Repository Setup**
  - Assigned to: huihui-qwen-9b
  - Status: ⏳ IN PROGRESS
  - Notes: Documentation ready, pending git init

### ⚡ HIGH (Priority 8-9)

- [ ] **Multi-Agent Testing**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Test coordination between Qwen + Claude + Cursor

- [ ] **Performance Optimization**
  - Assigned to: deepseek-r1-i1
  - Status: ⏳ PENDING
  - Notes: Optimize SQLCipher overhead (<5% target)

- [ ] **API Documentation**
  - Assigned to: huihui-qwen-9b
  - Status: ⏳ PENDING
  - Notes: Complete MCP tools documentation

- [ ] **Integrate Rate Limiting**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Integrate rate_limiter.rs into TCP/MCP servers for DoS protection

- [ ] **Complete MCP Tools Implementation**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Implement missing MCP tools (web_research, cve_search, security_classify, etc.)

### 📝 MEDIUM (Priority 5-7)

- [ ] **Unit Tests**
  - Assigned to: deepseek-coder:1.3b
  - Status: ⏳ PENDING
  - Notes: 80% code coverage target

- [ ] **Integration Tests**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Multi-agent scenario tests

- [ ] **Benchmark Suite**
  - Assigned to: huihui-qwen-9b
  - Status: ⏳ PENDING
  - Notes: Compare with Engram baseline

- [ ] **Improve Audit Logging**
  - Assigned to: deepseek-coder:6.7b
  - Status: ⏳ PENDING
  - Notes: Implement persistent audit logging in database (replace stub)

- [ ] **Cleanup Dead Code**
  - Assigned to: deepseek-coder:1.3b
  - Status: ⏳ PENDING
  - Notes: Remove #[allow(dead_code)] and unused modules

### 🐛 LOW (Priority 1-4)

- [ ] **Code Cleanup**
  - Assigned to: deepseek-coder:1.3b
  - Status: ⏳ PENDING
  - Notes: Fix clippy warnings

- [ ] **Documentation Polish**
  - Assigned to: huihui-qwen-9b
  - Status: ⏳ PENDING
  - Notes: Add diagrams, examples

---

## Ollama Sub-Agent Status

| Agent | Model | Current Task | Status |
|-------|-------|--------------|--------|
| Agent 1 | huihui-qwen-9b | Documentation | 🟢 Available |
| Agent 2 | deepseek-r1-i1 | Security Analysis | 🟢 Available |
| Agent 3 | deepseek-coder:6.7b | Code Implementation | 🟢 Available |
| Agent 4 | deepseek-coder:1.3b | Unit Tests | 🟢 Available |

---

## Parallel Execution Commands

```bash
# Run all documentation tasks in parallel
./scripts/ollama-subagents.sh documentation

# Run all security tasks in parallel
./scripts/ollama-subagents.sh security

# Run all code tasks in parallel
./scripts/ollama-subagents.sh code

# Run general tasks with all agents
./scripts/ollama-subagents.sh general
```

---

## Progress Tracking

- **Total Tasks:** 17
- **Completed:** 0 (0%)
- **In Progress:** 2 (12%)
- **Pending:** 15 (88%)

**Last Updated:** 2026-03-23
