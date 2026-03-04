# Changelog

All notable changes to this project will be documented in this file.

## [1.4.0] - 2026-03-04

### 🚀 New Features

- **VirusTotal Integration**: Complete implementation of VirusTotal API v3 for malware detection
  - Automatic URL scanning with 70+ antivirus engines
  - Real-time alerts for malicious and suspicious links
  - Configurable via `VIRUSTOTAL_API_KEY` environment variable
  - Comprehensive documentation in `docs/VIRUSTOTAL.md`
- **Enhanced URL Detection**: Fixed critical bug in URL entity detection
  - URLs are now correctly identified in both private chats and groups
  - Improved message processing pipeline with detailed logging

### 🐛 Bug Fixes

- **Critical**: Fixed `has_urls` flag not being set correctly, causing URLs to be skipped
  - URLs with `MessageEntityKind::Url` or `MessageEntityKind::TextLink` now properly detected
  - Commands no longer incorrectly trigger URL processing
- **Logging**: Added comprehensive debug logging throughout message processing pipeline
  - "Messaggio ricevuto" logs with user_id, chat_id, and text length
  - "Nessun URL trovato" vs "URL candidati trovati" for better debugging
  - "Invio risposta con URL puliti" for tracking successful responses

### 🔧 Technical Improvements

- **Dependencies**: Added `base64 = "0.22"` for VirusTotal URL encoding
- **Security**: VirusTotal requests use proper base64 URL-safe encoding without padding
- **Performance**: 10-second timeout for VirusTotal API calls to prevent blocking
- **Code Quality**: Improved error handling and logging consistency

### 📚 Documentation

- **New**: Complete VirusTotal integration guide (`docs/VIRUSTOTAL.md`)
  - Setup instructions with API key acquisition
  - Rate limits and free tier details (4 req/min, 500/day)
  - Privacy considerations and self-hosted alternatives
  - Troubleshooting and examples
- **Updated**: README.md with VirusTotal feature details
- **Updated**: ARCHITECTURE.md with VirusTotal integration section
- **Updated**: All documentation reflects current codebase state

### ⚠️ Breaking Changes

None - all changes are backward compatible.

### 🔒 Security

- VirusTotal integration is fully optional and disabled by default
- URLs sent to VirusTotal become public in their database (documented)
- No sensitive data logged or transmitted

## [1.3.0] - 2026-02-24

### 🛠 Migliorie principali

- Gestione errori esplicita e logging avanzato
- Modularità estesa: funzioni di sanitizzazione e validazione in moduli dedicati
- Test automatizzati aggiunti per validazione input/output
- Ottimizzazione performance con cache
- Sicurezza input/output rafforzata
- Internazionalizzazione dinamica tramite file JSON
- Documentazione aggiornata

All notable changes to this project will be documented in this file.

## [1.2.0] - 2026-01-20

### 🚀 Major Modernization Release

- **Rust Toolchain Update**: Updated to Rust 1.92+ with MSRV 1.75 for modern language features and performance improvements
- **Podman Migration**: Complete migration from Docker to Podman for enhanced security and rootless container support
- **Build Optimization**: Optimized Cargo.toml with LTO, single codegen unit, and improved release profile settings
- **Performance Improvements**: Enhanced build times and runtime performance through compiler optimizations

### 🔄 Breaking Changes

- **Container Runtime**: Switched from Docker to Podman (Docker still supported but deprecated)
- **Containerfile**: Replaced Dockerfile with Podman-compatible Containerfile
- **Deployment Scripts**: New `podman-deploy.sh` script replacing Docker-based deployment

### 🛠️ Code Quality & Modernization

- **Deprecated API Fixes**: Fixed all deprecated teloxide method calls (`msg.from()` → `msg.from`)
- **Modern Async Patterns**: Updated to use `ReplyParameters` for modern teloxide API
- **Memory Safety**: Replaced `LazyLock` with `once_cell::Lazy` for MSRV compatibility
- **Dependency Updates**: Updated to latest stable versions across all dependencies

### 📦 Container & Deployment Enhancements

- **Security Improvements**: Rootless Podman operation with proper SELinux labeling
- **Pod Networking**: Implemented Podman pod architecture for better network isolation
- **Resource Management**: Enhanced resource limits and monitoring capabilities
- **Multi-stage Builds**: Optimized container image size and security

### 📚 Documentation Overhaul

- **Comprehensive README**: Complete rewrite with modern deployment options and examples
- **Architecture Guide**: Updated with detailed Podman and performance optimization sections
- **Contributing Guide**: Expanded with development workflows, testing guidelines, and code standards
- **Deployment Guide**: New comprehensive deployment documentation covering all scenarios

### 🧪 Testing & Quality Assurance

- **Clippy Compliance**: All clippy warnings resolved with proper code quality standards
- **Build Verification**: Automated build testing for multiple Rust versions
- **Code Formatting**: Consistent rustfmt configuration across the project

## [1.1.0] - 2026-01-10

### Added

- **Smart Language Detection**: Automatically detects language of incoming messages (English/Italian) and replies in the corresponding language.
- **Supabase Integration**: Compatibility with Supabase PostgreSQL for persistent cloud storage.
- **WASM URL Cleaner**: High-performance Rust-compiled WebAssembly module for client-side sanitization.
- **Advanced Observability**: Robust logging system using `tracing` with JSON output support for production and colored pretty-logs for development.
- **Multi-Database Support**: Implemented dynamic backend detection (SQLite/Postgres) using `sqlx::Any`.

### Changed

- Refactored project structure into a modular library (`src/lib.rs`) and binary (`src/main.rs`).
- Upgraded all dependencies to latest major versions (`teloxide 0.17`, `axum 0.8`, `sqlx 0.8`).
- Improved documentation with detailed architecture and observability guides.
- Hardened web dashboard with Axum 0.8 compatibility and enhanced route safety.

### Fixed

- Deprecated `teloxide` method calls and updated to new `reply_parameters` API.
- Fixed `reqwest` TLS feature naming conflicts in version 0.13.
- **Zero-Panic Core**: Eliminated all `unwrap()` calls in favor of graceful error handling and descriptive status codes.
- **Bot Command Handling**: Fixed `/start` command compatibility in group chats and with bot handles.

### Removed

- **WASM Module**: Removed WebAssembly functionality to focus on core bot features and reduce complexity (reverted in 1.2.0 modernization)

---

## Migration Guide

### From 1.1.x to 1.2.0

#### Container Migration

```bash
# Old Docker way (deprecated)
docker-compose up

# New Podman way
./podman-deploy.sh start
# or
podman-compose -f podman-compose.yml up
```

#### Development Setup

```bash
# Update Rust toolchain
rustup update stable

# Rebuild with new optimizations
cargo clean
cargo build --release
```

#### Configuration Changes

- No configuration changes required
- Environment variables remain the same
- Database schema unchanged

### Breaking Changes Summary

- Docker support deprecated (still works but will be removed in future versions)
- Minimum Rust version increased to 1.75 (from 1.70)
- Container image names updated (clear_urls_bot vs previous naming)
- Some internal APIs changed for better performance

---

## Support

For help with migration or issues:

- [GitHub Issues](https://github.com/yourusername/clear_urls_bot/issues)
- [Discussions](https://github.com/yourusername/clear_urls_bot/discussions)
- [Documentation](docs/)
