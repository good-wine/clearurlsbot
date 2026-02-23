# Architecture Overview 🏗️

This project is designed with modularity, performance, and security in mind, supporting both containerized and local deployments with modern Rust practices and Podman optimization.

## 📦 Component Structure

### 1. Core Library (`src/lib.rs`)
The backbone of the application with a modular architecture:
- **Sanitizer Module** (`src/sanitizer/`):
  - `rule_engine.rs`: Regex-based URL processing engine with sensitive data redaction
  - `ai_engine.rs`: Optional LLM-powered deep-scan for complex tracking parameters
  - Includes URL expansion to uncover hidden trackers in shortened links
- **Database Module** (`src/db/`):
  - `implementation.rs`: Database abstraction layer using **sqlx::Any**
  - `models.rs`: Data models for users, chats, cleaned links, and custom rules
  - Dynamic backend detection supporting both **PostgreSQL** and **SQLite**
- **Bot Module** (`src/bot.rs`): 
  - Modern Telegram bot handler using Teloxide with ReplyParameters
  - Smart language detection and internationalization
  - Efficient async message processing with proper error handling
- **AI Module** (`src/ai_sanitizer.rs` → `src/sanitizer/ai_engine.rs`):
  - Integrated AI-powered URL sanitization
  - Configurable model selection and API endpoints
- **Configuration** (`src/config.rs`):
  - Environment-based configuration with validation
  - Secure handling of sensitive data
- **Internationalization** (`src/i18n.rs`):
  - Multi-language support for English and Italian
  - Context-aware language detection

### 2. Application Entry Point (`src/main.rs`)
Optimized initialization sequence:
1. Configuration loading and validation
2. Database initialization with migrations
3. Cache setup (Moka for performance)
4. Bot service startup with graceful shutdown handling

### 3. Module Organization (`src/`)
```
src/
├── lib.rs              # Library crate definition
├── main.rs             # Application entry point
├── bot.rs              # Telegram bot logic
├── config.rs           # Configuration management
├── i18n.rs            # Internationalization
├── db/                 # Database layer
│   ├── mod.rs
│   ├── implementation.rs
│   └── models.rs
└── sanitizer/          # URL processing engines
    ├── mod.rs
    ├── rule_engine.rs
    └── ai_engine.rs
```

## 🔄 Data Flow Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Telegram API   │───▶│   Bot Handler   │───▶│  URL Detection  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                        │
                                                        ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Audit Log      │◀───│  Database       │◀───│ URL Sanitization│
└─────────────────┘    └─────────────────┘    └─────────────────┘
                              ▲                        │
                              │                        ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Statistics    │◀───│  User Config   │◀───│   AI Engine     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

### Processing Pipeline
1. **Message Reception**: Telegram updates processed via long polling
2. **URL Detection**: Entity-based detection + regex fallback
3. **Context Analysis**: Language detection, user/chat configuration lookup
4. **Sanitization**: Rule engine → AI engine (optional) → final result
5. **Persistence**: Audit logging, statistics tracking, user preferences
6. **Response**: Formatted response with cleaned URLs

## 📊 Database Schema & Architecture

### Supported Backends
- **SQLite**: Default for development and small deployments
- **PostgreSQL**: Recommended for production with high concurrency

### Core Tables
```sql
user_configs     -- User-specific settings and preferences
chat_configs     -- Group/chat specific configurations  
cleaned_links   -- Comprehensive audit log of all processed URLs
custom_rules     -- User-defined regex patterns for custom sanitization
```

### Connection Strategy
- **Dynamic Detection**: Automatic backend selection based on connection string
- **Connection Pooling**: Optimized for both SQLite and PostgreSQL
- **Migrations**: Automatic schema management with SQLx migrations

## 🐳 Container Architecture (Podman)

### Security-First Design
- **Rootless Operation**: Full support for rootless Podman
- **Non-root User**: Container execution as dedicated `clearurls` user
- **SELinux Integration**: Proper file labeling for enhanced security
- **Minimal Attack Surface**: Slim base image with only required dependencies

### Multi-stage Build Optimization
```dockerfile
# Stage 1: Build
FROM rust:1.92-slim as builder  # Optimized Rust toolchain
# Stage 2: Runtime  
FROM debian:bookworm-slim          # Minimal runtime base
```

### Resource Management
- **Memory Limit**: 512MB (configurable)
- **CPU Limit**: 0.5 cores (configurable)
- **Restart Policy**: Unless-stopped for reliability
- **Health Checks**: Container health monitoring

### Volume Strategy
- **SQLite**: Host-mounted database file with proper SELinux context
- **PostgreSQL**: Network connection to external database
- **Logs**: Structured logging with rotation to prevent disk exhaustion

### Pod Networking
```bash
# Pod creation for network isolation
podman pod create --name clear_urls_bot_pod -p 3000:3000
# Container joins pod for shared networking
podman run --pod clear_urls_bot_pod clear_urls_bot
```

## 🚀 Performance Optimizations

### Build Optimizations
- **LTO (Link Time Optimization)**: Better binary optimization across crate boundaries
- **Single Codegen Unit**: Maximum optimization potential
- **Panic = Abort**: Smaller binary size, faster startup
- **Opt-level 3**: Maximum performance optimizations

### Runtime Optimizations
- **Async I/O**: Non-blocking operations throughout
- **Connection Pooling**: Database connection reuse
- **Caching**: Multi-layer caching strategy (Moka for hot data)
- **Zero-copy**: Minimize data copying in hot paths
- **Efficient Regex**: Compiled patterns with sensitive data protection

### Memory Management
- **String Interning**: Reduce allocations for common strings
- **Arc/Mutex**: Safe shared state with minimal contention
- **Buffer Management**: Reuse buffers where possible

## 🛡️ Reliability & Stability

### Error Handling Philosophy
- **Result Types**: Graceful error propagation throughout
- **No Panics**: Core logic avoids `unwrap()` and `expect()`
- **Fallback Strategies**: Multiple levels of fallback for robustness
- **Structured Logging**: Comprehensive tracing for debugging

### Configuration Management
- **Environment-based**: All configuration via environment variables
- **Validation**: Automatic configuration validation on startup
- **Secure Defaults**: Secure defaults for all settings
- **Hot Reload**: Configuration changes without restart where possible

### Observability
- **Structured Logging**: JSON-formatted logs with correlation IDs
- **Metrics**: Built-in performance and usage metrics
- **Health Checks**: Application health monitoring
- **Tracing**: Distributed tracing for request flow analysis

## 🔧 Development Architecture

### Toolchain Requirements
- **Minimum Rust**: 1.75 (MSRV)
- **Recommended Rust**: 1.92+ (tested)
- **Edition**: Rust 2021 with modern features

### Code Organization Principles
- **Single Responsibility**: Each module has a clear, focused purpose
- **Dependency Injection**: Testable architecture with trait abstractions
- **Async/Await**: Consistent async patterns throughout
- **Error Handling**: Comprehensive error types and recovery strategies

### Testing Strategy
- **Unit Tests**: Comprehensive test coverage for core logic
- **Integration Tests**: End-to-end testing with real databases
- **Property Tests**: Generative testing for edge cases
- **Benchmarks**: Performance regression testing

This architecture provides a solid foundation for a production-ready, secure, and maintainable URL sanitization service.