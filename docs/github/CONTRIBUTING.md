# Contributing to Synapsis

Thank you for your interest in contributing to Synapsis! This document provides guidelines and instructions for contributing.

## 🚀 Getting Started

### Prerequisites

- Rust 1.75 or later
- SQLite 3.35+
- Git

### Setting Up Development Environment

```bash
# Clone the repository
git clone https://github.com/methodwhite/synapsis.git
cd synapsis

# Install dependencies
cargo build

# Run tests
cargo test

# Run with all features
cargo test --all-features
```

## 📝 Development Workflow

1. **Fork the repository**
2. **Create a feature branch** (`git checkout -b feature/amazing-feature`)
3. **Make your changes**
4. **Run tests** (`cargo test`)
5. **Run clippy** (`cargo clippy -- -D warnings`)
6. **Format code** (`cargo fmt`)
7. **Commit your changes** (`git commit -m 'Add amazing feature

Co-authored-by: Qwen-Coder <qwen-coder@alibabacloud.com>'`)
8. **Push to your fork** (`git push origin feature/amazing-feature`)
9. **Open a Pull Request**

## 🔒 Security Contributions

For security-related contributions:

1. **DO NOT** disclose vulnerabilities publicly
2. Email security@synapsis.dev directly
3. Allow 90 days for patch development
4. Coordinate disclosure

## 🧪 Testing Guidelines

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generation() {
        let session_id = generate_session_id("qwen", b"secret");
        assert_eq!(session_id.len(), 64); // SHA256 hex
    }
}
```

### Integration Tests

```bash
# Run integration tests
cargo test --test integration

# Run with coverage
cargo tarpaulin --out Html
```

## 📖 Documentation

- Update README.md if you change public APIs
- Add doc comments for public functions
- Update SECURITY.md for security-related changes

## 🎯 Code Style

- Follow Rust API Guidelines
- Use `cargo clippy` for linting
- Use `cargo fmt` for formatting
- Write meaningful commit messages

## 🤝 Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Welcome newcomers

## 📬 Questions?

Open an issue or email methodwhite@proton.me (primary) · methodwhite.developer@gmail.com (enterprise)

---

Thank you for contributing to Synapsis! 🦀
