# 🤝 Contributing to Synapsis

**Thank you for your interest in contributing to Synapsis!**

This document provides guidelines and instructions for contributing.

---

## 📋 Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [How to Contribute](#how-to-contribute)
5. [Pull Request Guidelines](#pull-request-guidelines)
6. [Coding Standards](#coding-standards)
7. [Testing](#testing)
8. [Documentation](#documentation)
9. [Security](#security)
10. [Recognition](#recognition)

---

## Code of Conduct

### Our Pledge

We are committed to providing a welcoming and inspiring community for all. Please be respectful and constructive in your interactions.

### Our Standards

**Expected Behavior:**
- ✅ Be respectful and inclusive
- ✅ Accept constructive criticism
- ✅ Focus on what's best for the community
- ✅ Show empathy towards others

**Unacceptable Behavior:**
- ❌ Harassment or discrimination
- ❌ Trolling or insulting comments
- ❌ Publishing others' private information
- ❌ Promoting illegal activities

### Enforcement

Report unacceptable behavior to: methodwhite@proton.me

---

## Getting Started

### Prerequisites

- **Rust:** 1.88+
- **Git:** For version control
- **Cargo:** Rust package manager
- **Text Editor:** VS Code, Neovim, etc.

### Fork and Clone

```bash
# Fork the repository on GitHub

# Clone your fork
git clone https://github.com/YOUR_USERNAME/synapsis.git
cd synapsis

# Add upstream remote
git remote add upstream https://github.com/MethodWhite/synapsis.git

# Verify remotes
git remote -v
```

### Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dependencies
rustup install 1.88
rustup default 1.88

# Install development tools
cargo install cargo-watch
cargo install cargo-audit
cargo install cargo-tarpaulin

# Build the project
cargo build --release

# Run tests
cargo test

# Run with hot-reload
cargo watch -x "test"
```

### Docker Development (Optional)

```bash
# Start development environment
docker-compose up -d

# Access container
docker-compose exec synapsis-dev bash

# Run tests in container
cargo test
```

---

## How to Contribute

### Ways to Help

1. **Report Bugs** - Open an issue
2. **Fix Bugs** - Submit a PR
3. **Add Features** - Discuss first, then implement
4. **Improve Docs** - Always welcome
5. **Write Tests** - Help improve coverage
6. **Review Code** - Help maintain quality
7. **Answer Questions** - Help the community

### Finding Issues

Look for issues labeled:
- 🐛 `good first issue` - Perfect for beginners
- 🔧 `help wanted` - Need community help
- 📚 `documentation` - Improve docs
- 🧪 `testing` - Write tests

---

## Pull Request Guidelines

### Before Submitting

1. **Fork the repo** and create your branch
2. **Discuss** major changes in an issue first
3. **Write tests** for new functionality
4. **Update docs** if needed
5. **Run tests** and ensure they pass
6. **Check formatting** with `cargo fmt`
7. **Run clippy** with `cargo clippy`

### PR Title Format

```
type: short description

Examples:
feat: Add new PQC algorithm support
fix: Resolve memory leak in session manager
docs: Update README with installation steps
test: Add tests for Kyber768
refactor: Improve error handling in MCP server
```

### PR Description Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] 🐛 Bug fix
- [ ] ✨ New feature
- [ ] 📚 Documentation
- [ ] 🧪 Tests
- [ ] 🔧 Refactor
- [ ] ⚡ Performance

## Testing
- [ ] Tests pass
- [ ] New tests added
- [ ] Manually tested

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] No new warnings
- [ ] Tests added/updated
```

### Review Process

1. **Automated Checks** - CI must pass
2. **Code Review** - Maintainer reviews
3. **Testing** - Verify functionality
4. **Approval** - At least 1 approval required
5. **Merge** - Squash and merge

---

## Coding Standards

### Rust Style

Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

**Example:**
```rust
// ✅ Good: Clear naming
pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
    
    pub fn create_session(&mut self, agent: &str) -> Result<String> {
        // Implementation
    }
}

// ❌ Bad: Unclear naming
pub struct SM {
    s: HashMap<String, S>,
}
```

### Error Handling

```rust
// ✅ Good: Descriptive errors
#[derive(Debug, thiserror::Error)]
pub enum SynapsisError {
    #[error("Failed to create session: {0}")]
    SessionCreationFailed(String),
    
    #[error("Invalid agent type: {0}")]
    InvalidAgentType(String),
}

// ✅ Good: Proper error propagation
pub fn create_session(&self, agent: &str) -> Result<Session> {
    let session = self.validate_agent(agent)
        .map_err(|e| SynapsisError::SessionCreationFailed(e.to_string()))?;
    Ok(session)
}
```

### Documentation

```rust
/// Create a new session for the specified agent
///
/// # Arguments
///
/// * `agent` - The agent type identifier
/// * `project` - Optional project name
///
/// # Returns
///
/// * `Ok(Session)` - Created session
/// * `Err(SynapsisError)` - Error creating session
///
/// # Example
///
/// ```
/// let manager = SessionManager::new();
/// let session = manager.create_session("qwen", Some("my-project"))?;
/// ```
pub fn create_session(&self, agent: &str, project: Option<&str>) -> Result<Session> {
    // Implementation
}
```

---

## Testing

### Running Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_kyber512

# With output
cargo test -- --nocapture

# Coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let manager = SessionManager::new();
        let result = manager.create_session("test-agent", None);
        
        assert!(result.is_ok());
        let session = result.unwrap();
        assert_eq!(session.agent_type, "test-agent");
    }

    #[test]
    fn test_invalid_agent() {
        let manager = SessionManager::new();
        let result = manager.create_session("", None);
        
        assert!(result.is_err());
    }
}
```

### Test Coverage Goals

| Component | Target | Current |
|-----------|--------|---------|
| Core | 90% | 🎯 |
| PQC | 100% | ✅ |
| API | 80% | 🎯 |
| Utils | 70% | 🎯 |

---

## Documentation

### Documentation Standards

1. **Public APIs** - Must have doc comments
2. **Complex Logic** - Add explanatory comments
3. **Examples** - Include usage examples
4. **Errors** - Document possible errors

### Building Docs

```bash
# Generate documentation
cargo doc --no-deps

# Open in browser
cargo doc --no-deps --open

# Build with private items
cargo doc --document-private-items
```

---

## Security

### Reporting Vulnerabilities

**DO NOT** open public issues for security vulnerabilities.

**DO** email: methodwhite@proton.me

Include:
- Description of vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Security Best Practices

1. **Never commit** secrets or API keys
2. **Use environment variables** for sensitive data
3. **Review dependencies** regularly
4. **Run cargo-audit** before submitting
5. **Follow secure coding** guidelines

---

## Recognition

### Contributors

We recognize all contributors in:
- README.md contributors section
- Release notes
- Annual contributor report

### Becoming a Maintainer

Active contributors may be invited to become maintainers:
- Consistent contributions
- Code review participation
- Community engagement
- Project alignment

---

## Questions?

### Getting Help

- **GitHub Issues:** For bugs and feature requests
- **GitHub Discussions:** For questions and ideas
- **Email:** methodwhite@proton.me

### Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Synapsis Documentation](docs/)

---

## License

By contributing, you agree that your contributions will be licensed under the BUSL-1.1 license.

---

**Thank you for contributing to Synapsis!** 🎉

Every contribution, no matter how small, makes a difference.
