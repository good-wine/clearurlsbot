# Contributing to ClearURLs Bot 🛡️

Thank you for your interest in contributing! We welcome all contributions that help make this project more robust, feature-rich, and secure. This guide covers everything you need to know to contribute effectively.

## 🚀 Quick Start

### Prerequisites

**Required:**
- [Rust](https://www.rust-lang.org/tools/install) 1.75+ (MSRV), 1.92+ recommended
- [Git](https://git-scm.com/) for version control

**Optional (for container development):**
- [Podman](https://podman.io/getting-started/installation) for containerized development
- [Podman Compose](https://github.com/containers/podman-compose) for compose workflows
- [Docker](https://www.docker.com/) (alternative to Podman)

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/clear_urls_bot.git
cd clear_urls_bot

# Set up your development environment
cp .env.example .env
# Edit .env with your development configuration

# Install development dependencies
rustup component add rustfmt clippy
cargo install cargo-watch cargo-audit  # Optional but recommended
```

## 🛠️ Development Workflows

### Local Development

```bash
# Install dependencies and build
cargo build

# Run with auto-reload during development
cargo watch -x run

# Run tests
cargo test

# Check code quality
cargo fmt --check
cargo clippy --all-targets --all-features
```

### Container Development

```bash
# Build container for testing
./podman-deploy.sh build

# Run container locally
./podman-deploy.sh run

# View container logs
./podman-deploy.sh logs

# Stop container
./podman-deploy.sh stop
```

### Database Development

```bash
# Development with SQLite (default)
cargo run

# Development with PostgreSQL
export DATABASE_URL=postgresql://user:pass@localhost/clearurls_dev
cargo run

# Run database migrations
cargo sqlx migrate run --database-url sqlite:bot_dev.db
```

## 🧪 Testing & Quality Assurance

### Pre-commit Checklist

Before submitting any PR, ensure all of these pass:

```bash
# Code formatting
cargo fmt

# Linting and static analysis
cargo clippy --all-targets --all-features

# Security audit
cargo audit

# Run all tests
cargo test --all-features

# Build check for all targets
cargo check --all-targets --all-features

# Documentation check
cargo doc --no-deps --document-private-items
```

### Testing Guidelines

1. **Unit Tests**: Test individual functions and modules
   ```bash
   cargo test --lib
   ```

2. **Integration Tests**: Test component interactions
   ```bash
   cargo test --test '*'
   ```

3. **Documentation Tests**: Ensure examples in documentation work
   ```bash
   cargo test --doc
   ```

4. **Performance Tests**: Benchmark critical paths
   ```bash
   cargo bench  # If benchmarks exist
   ```

### Code Quality Standards

- **Clippy**: All clippy warnings must be addressed or explicitly allowed
- **Documentation**: All public functions and types must have `///` documentation
- **Error Handling**: Use `Result` types, avoid `unwrap()` in production code
- **Logging**: Use `tracing` for structured logging with appropriate levels

## 📝 Development Guidelines

### Code Style

We follow standard Rust conventions with these additional guidelines:

```rust
// ✅ Good: Use descriptive names and proper error handling
async fn process_message(
    bot: Bot, 
    msg: Message,
    db: Db
) -> ResponseResult<()> {
    // Implementation with proper error handling
}

// ❌ Bad: Vague names, unwrapping, no error context
async fn handle(b: Bot, m: Message, d: Db) {
    let result = some_operation().unwrap();
}
```

### Project Structure

```
src/
├── lib.rs              # Public API and module exports
├── main.rs             # Application entry point
├── bot.rs              # Telegram bot logic
├── config.rs           # Configuration management
├── i18n.rs            # Internationalization
├── db/                 # Database layer
│   ├── mod.rs
│   ├── implementation.rs
│   └── models.rs
└── sanitizer/          # URL processing
    ├── mod.rs
    ├── rule_engine.rs
    └── ai_engine.rs
```

### Adding New Features

1. **Database Changes**: 
   - Create migration in `migrations/` directory
   - Update models in `src/db/models.rs`
   - Add relevant tests

2. **New Dependencies**:
   - Add to `Cargo.toml` with specific version
   - Update `rust-toolchain.toml` if minimum Rust version changes
   - Document why the dependency is needed

3. **Configuration**:
   - Add to `src/config.rs` with validation
   - Update `.env.example` with new settings
   - Document in configuration section

## 📬 Pull Request Process

### Step-by-Step PR Submission

1. **Create Branch**: Use descriptive branch names
   ```bash
   git checkout -b feature/url-sanitization-enhancement
   # or
   git checkout -b fix/database-connection-leak
   ```

2. **Development Work**:
   ```bash
   # Make your changes
   # Commit frequently with descriptive messages
   git add .
   git commit -m "feat: add AI-powered URL detection
   
   - Implement LLM integration for complex tracking patterns
   - Add configuration options for AI model selection  
   - Update documentation with AI setup instructions"
   ```

3. **Quality Checks**:
   ```bash
   # Run full test suite
   ./scripts/check-all.sh  # If available, or run manually
   ```

4. **Create PR**: 
   - Use GitHub's web interface or CLI
   - Fill out PR template completely
   - Link relevant issues
   - Add screenshots for UI changes if applicable

### PR Requirements

- **Clear Title**: Use conventional commit format (`feat:`, `fix:`, `docs:`, etc.)
- **Description**: Explain what, why, and how
- **Testing**: Describe how you tested your changes
- **Documentation**: Update relevant docs
- **Breaking Changes**: Clearly label and provide migration guide

### Review Process

1. **Automated Checks**: CI/CD pipeline runs all tests
2. **Code Review**: At least one maintainer review required
3. **Testing**: Reviewer may request additional tests
4. **Approval**: PR approved after all checks pass

## 🔧 Development Tools & Scripts

### Useful Aliases

```bash
# Add to your shell config for convenience
alias cb="cargo build"
alias cr="cargo run"
alias ct="cargo test"
alias cc="cargo check"
alias cf="cargo fmt"
alias cl="cargo clippy"
alias cw="cargo watch -x run"
```

### Development Scripts

```bash
# Full quality check (create as scripts/check-all.sh)
#!/bin/bash
set -e
echo "🔍 Running full quality checks..."

cargo fmt
echo "✅ Formatting checked"

cargo clippy --all-targets --all-features
echo "✅ Clippy passed"

cargo test --all-features
echo "✅ Tests passed"

cargo audit
echo "✅ Security audit passed"

echo "🎉 All checks passed!"
```

## 🐛 Debugging & Troubleshooting

### Common Issues

1. **Build Failures**: 
   ```bash
   # Clean build
   cargo clean && cargo build
   
   # Update dependencies
   cargo update
   ```

2. **Database Issues**:
   ```bash
   # Reset database
   rm bot.db && cargo run
   
   # Check migrations
   cargo sqlx migrate info
   ```

3. **Container Issues**:
   ```bash
   # Rebuild container
   podman rmi clear_urls_bot
   ./podman-deploy.sh build
   ```

### Logging Configuration

```bash
# Enable debug logging
export RUST_LOG=debug
export RUST_LOG_STYLE=always
cargo run

# Specific module logging
export RUST_LOG=clear_urls_bot::sanitizer=trace,clear_urls_bot::bot=debug
cargo run
```

## 📋 Issue Reporting

### Bug Reports

Use the issue template with:
- **Environment**: Rust version, OS, database type
- **Steps to Reproduce**: Clear, minimal reproduction steps
- **Expected Behavior**: What should happen
- **Actual Behavior**: What actually happens
- **Logs**: Relevant log output

### Feature Requests

Provide:
- **Use Case**: Why this feature is needed
- **Proposed Solution**: How you envision it working
- **Alternatives Considered**: Other approaches you've thought of
- **Additional Context**: Any other relevant information

## 🏆 Recognition

Contributors are recognized in:
- **README.md**: Contributors section
- **CHANGELOG.md**: Credits for specific contributions
- **Releases**: Special thanks for major contributions

## ⚖️ Code of Conduct

We are committed to providing a welcoming and inclusive environment. Please:

- **Be Respectful**: Treat all community members with respect
- **Be Inclusive**: Welcome newcomers and help them learn
- **Be Constructive**: Provide helpful, constructive feedback
- **Be Professional**: Maintain professional conduct in all interactions

For full details, see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## 🤝 Getting Help

- **Discussions**: Use GitHub Discussions for questions
- **Issues**: For bugs and feature requests
- **Documentation**: Check existing docs first
- **Maintainers**: Tag maintainainers for urgent issues

Thank you for contributing to ClearURLs Bot! Every contribution helps make the project better. 🎉
