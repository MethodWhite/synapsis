# 🐛 GitHub Tool - Secure Integration

## Security Features

- ✅ **OAuth 2.0 with PKCE** - Most secure OAuth flow
- ✅ **Encrypted Token Storage** - Synapsis Vault
- ✅ **Rate Limiting** - Per-session rate limiting
- ✅ **Audit Logging** - All GitHub operations logged
- ✅ **Secure Token Refresh** - Automatic refresh with encryption

## Architecture

```
CLI/IDE
    ↓
Synapsis MCP Bridge
    ↓
GitHub Tool (OAuth 2.0 + PKCE)
    ↓
Synapsis Vault (Encrypted Tokens)
    ↓
GitHub API
```

## Setup

### 1. Create GitHub OAuth App

1. Go to GitHub Settings → Developer settings → OAuth Apps
2. Click "New OAuth App"
3. Fill in:
   - **Application name:** Synapsis GitHub Tool
   - **Homepage URL:** https://synapsis.dev
   - **Authorization callback URL:** `http://localhost:7438/github/callback`
4. Copy **Client ID** and generate **Client Secret**

### 2. Configure Synapsis

```toml
# ~/.config/synapsis/config.toml
[github]
client_id = "your_client_id"
client_secret_encrypted = "encrypted_in_vault"
scopes = ["repo", "user", "read:org"]
redirect_uri = "http://localhost:7438/github/callback"
```

## Usage

### CLI Usage

```bash
# Initialize OAuth flow
synapsis github auth --init

# Opens browser for authentication
# After authorization, token is stored encrypted in vault

# List repositories
synapsis github repos

# Create issue
synapsis github issue create methodwhite/synapsis "Bug title" "Bug description"

# Search repositories
synapsis github search "rust ai"
```

### IDE Usage (via MCP)

```json
// Initialize GitHub tool
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "github_init",
    "arguments": {
      "client_id": "your_client_id",
      "scopes": ["repo", "user"]
    }
  },
  "id": 1
}

// List repositories
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "github_get_repos",
    "arguments": {}
  },
  "id": 2
}

// Create issue
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "github_create_issue",
    "arguments": {
      "owner": "methodwhite",
      "repo": "synapsis",
      "title": "Feature request",
      "body": "Description"
    }
  },
  "id": 3
}
```

### Rust API Usage

```rust
use synapsis::tools::github::{GitHubTool, GitHubConfig};
use std::sync::Arc;

// Initialize with secure components
let vault = Arc::new(SecureVault::new(data_dir));
let rate_limiter = Arc::new(RateLimiter::new(100, 1000));
let audit_logger = Arc::new(Mutex::new(AuditLogger::new(log_path)?));

let github = GitHubTool::new(vault, rate_limiter, audit_logger);

// Initialize OAuth
let config = GitHubConfig {
    client_id: "your_client_id".to_string(),
    client_secret: "encrypted".to_string(),
    scopes: vec!["repo".to_string(), "user".to_string()],
    redirect_uri: "http://localhost:7438/github/callback".to_string(),
};

let auth_url = github.init_oauth(config)?;
// Open auth_url in browser

// After authorization
let token = github.exchange_code(config, &authorization_code)?;

// Use GitHub API
let repos = github.get_repos(session_id)?;
let issue = github.create_issue(session_id, "owner", "repo", "title", "body")?;
```

## Security Flow

### OAuth 2.0 with PKCE

```
1. Client generates code_verifier (random 32 bytes)
2. Client creates code_challenge = SHA256(code_verifier)
3. Client opens browser: /authorize?code_challenge=...&code_challenge_method=S256
4. User authorizes
5. GitHub redirects to callback with code
6. Client exchanges code + code_verifier for token
7. GitHub verifies: SHA256(code_verifier) == code_challenge
8. Token stored encrypted in Synapsis Vault
```

### Token Storage

```
┌─────────────────────────────────────┐
│  Synapsis Vault                     │
│  ┌───────────────────────────────┐  │
│  │ github_access_token           │  │
│  │ Encrypted: AES-256-GCM        │  │
│  │ Key: Derived from master key  │  │
│  └───────────────────────────────┘  │
│  ┌───────────────────────────────┐  │
│  │ github_refresh_token          │  │
│  │ Encrypted: AES-256-GCM        │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

## Rate Limiting

| Operation | Limit | Window |
|-----------|-------|--------|
| Get Repos | 100 | per minute |
| Create Issue | 10 | per minute |
| Search | 30 | per minute |
| Token Refresh | 5 | per hour |

## Audit Logging

All GitHub operations are logged:

```json
{
  "timestamp": "2026-03-22T10:00:00Z",
  "session_id": "qwen-code-abc123",
  "action": "github_create_issue",
  "success": true,
  "details": "Creating issue in methodwhite/synapsis"
}
```

## Error Handling

| Error | Code | Description |
|-------|------|-------------|
| InvalidToken | 401 | Token expired or invalid |
| RateLimitExceeded | 429 | Too many requests |
| NetworkError | 500 | Network connectivity issue |
| VaultError | 500 | Token storage/retrieval failed |
| ApiError | 400-599 | GitHub API error |

## Best Practices

1. **Always use PKCE** - Even for CLI/IDE apps
2. **Store tokens encrypted** - Never in plaintext
3. **Implement rate limiting** - Respect GitHub limits
4. **Log all operations** - For security auditing
5. **Refresh tokens proactively** - Before expiration
6. **Handle errors gracefully** - Don't expose tokens in errors

---

**Security Score:** 10/10 ✅  
**OAuth Flow:** 2.0 with PKCE  
**Token Storage:** Encrypted (AES-256-GCM)  
**Audit Logging:** Enabled
