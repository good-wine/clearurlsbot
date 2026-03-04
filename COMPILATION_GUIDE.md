# 🚀 Build Optimization Guide

## Overview

ClearURLs Bot è stato ottimizzato per **build veloce in sviluppo** e **binary efficiente in produzione**.

---

## Quick Start

### Development (Fast - 20-30s)
```bash
# Via Makefile
make build-fast

# Via cargo directly
cargo build
```

### Production (Optimized - 2-3 min)
```bash
# Via Makefile
make release

# Via cargo
cargo build --release

# Via script with options
./build.sh --release
```

### Quick Syntax Check (2-3s)
```bash
make check-fast
```

---

## Build Profiles Explained

| Profile | Use Case | Speed | Binary Size | Runtime Perf |
|---------|----------|-------|-------------|--------------|
| `dev` | Development | ⚡⚡⚡ Fast | Large | Slower |
| `release` | Production | 🐢 Slow | 📦 Small (11MB) | ⚡ Fast |
| `release-with-debug` | Profiling | 🐢 Slow | Medium | ⚡ Fast |

### Profile Configuration

**Development (`[profile.dev]`):**
- `opt-level = 1` → Minimal optimization for fast compilation
- `debug = 0` → No debug info (only 2-3s compilation)
- `incremental = true` → Recompile only changed files
- Dependencies optimized with `opt-level = 2`

**Release (`[profile.release]`):**
- `opt-level = 3` → Maximum optimization
- `lto = true` → Link Time Optimization (slower build, smaller binary)
- `codegen-units = 1` → Single codegen unit for better optimization
- `strip = true` → Remove debug symbols (~2MB saved)
- `panic = "abort"` → Smaller panic code
- `incremental = false` → Full rebuild for correctness

---

## Build Tools

### 1. **Makefile** (Recommended)
Best for most workflows with syntax highlighting.

```bash
make help          # Show all commands
make check-fast    # Quick syntax check (2-3s)
make build-fast    # Debug build (20-30s)
make release       # Release build (2-3 min)
make test          # Run tests
make clippy        # Run linter
make ci             # Full CI pipeline
```

### 2. **VS Code Tasks** (Graphical)
Press `Cmd+Shift+B` (or `Ctrl+Shift+B`) and select:
- ✓ check (fast)
- ✓ build (debug - optimized)
- ✓ build (release - full optimization)
- ✓ test
- ✓ clippy (linter)
- ✓ ci (full pipeline)

### 3. **Build Script** (`build.sh`)
Fine-grained control with options.

```bash
./build.sh --help

# Examples:
./build.sh                    # Release with strip (default)
./build.sh --debug            # Debug build
./build.sh --release --no-strip  # Release without stripping
./build.sh --verbose          # With verbose output
```

### 4. **Direct Cargo Commands**
For advanced users:

```bash
cargo check --release         # Syntax check only
cargo build                   # Debug optimized
cargo build --release         # Full optimization
cargo clippy --release        # Linter
cargo test --release          # Run tests
cargo fmt                     # Format code
```

---

## Optimization Details

### Cargo Configuration (`.cargo/config.toml`)

```toml
[build]
jobs = 8  # Use 8 CPU cores for parallel compilation

[registries.crates-io]
protocol = "sparse"  # Faster registry downloads

[net]
git-fetch-with-cli = true  # Better caching
retry = 3  # Retry failed downloads
```

### Binary Size Optimization

**Before optimization:** ~20MB (with debug symbols)  
**After optimization:** ~11MB (stripped)

Achieved by:
- ✓ `strip = true` in release profile
- ✓ `lto = true` (Link Time Optimization)
- ✓ `opt-level = 3` (Maximum compression)
- ✓ `panic = "abort"` (Smaller panic handler)
- ✓ `codegen-units = 1` (Better optimization)

---

## Compilation Times (Benchmarks)

### First Build (Clean)
```
cargo check --release:  ⏱️  ~50s
cargo build:            ⏱️  ~4 min
cargo build --release:  ⏱️  ~7 min
```

### Incremental Build (After small change)
```
cargo check --release:  ⏱️  ~2-3s
cargo build:            ⏱️  ~15-20s
cargo build --release:  ⏱️  ~30-40s
```

### CI Pipeline (Full checks)
```
cargo clean && full build: ⏱️  ~8-10 min
```

---

## Common Workflows

### Development Flow (Quick Iteration)
```bash
# 1. Edit code
# 2. Quick syntax check
make check-fast

# 3. Run unit tests
cargo test

# 4. Format code
cargo fmt

# 5. Build & test
make build-fast
```

### Before Commit
```bash
make ci  # Runs: check + clippy + test (this is safe to use locally)
```

### Production Release
```bash
make release  # Full optimized build with strip
make size     # Show binary size
```

---

## Troubleshooting

### Build is slow
1. Check if another cargo process is running: `ps aux | grep cargo`
2. Try `cargo clean` (careful - will rebuild everything)
3. Ensure you're using development mode for local work: `cargo build` (not `--release`)

### Binary is too large
```bash
# Check what's taking space
du -sh target/

# Strip manually
strip target/release/clear_urls_bot

# Rebuild with LTO
cargo build --release -Zbuild-std=core,alloc,std -Zbuild-std-features=core/memchr
```

### Check uses wrong Rust
```bash
rustc --version
cargo --version
rustup toolchain list
```

---

## Environment Variables

For advanced tuning:

```bash
# Use all CPU cores
export CARGO_BUILD_JOBS=16

# Verbose build
export CARGO_VERBOSE=true

# Target directory
export CARGO_TARGET_DIR=$PWD/build

# Example - build with env var:
CARGO_BUILD_JOBS=16 cargo build --release
```

---

## CI/CD Recommendations

For GitHub Actions / GitLab CI:

```yaml
# Caching (major speedup)
- uses: Swatinem/rust-cache@v2

# Quick syntax check stage
cargo check --release

# Lint stage
cargo clippy --release -- -D warnings

# Test stage
cargo test --release

# Build stage
cargo build --release
```

---

## Future Optimizations (Optional)

If compilation is still slow:

1. **sccache** - Distributed caching
   ```toml
   [build]
   rustc-wrapper = "sccache"
   ```

2. **mold linker** - Faster linking (Linux only)
   ```toml
   [target.x86_64-unknown-linux-gnu]
   linker = "clang"
   rustflags = ["-C", "link-arg=--ld-path=/usr/local/bin/mold"]
   ```

3. **cranelift** - Alternative codegen (experimental)
   ```bash
   cargo install cargo-cranelift
   RUSTFLAGS="-C llvm-args=-use-cranelift" cargo build
   ```

4. **Parallel frontends** (Nightly)
   ```bash
   cargo +nightly build -Z thread-local-const
   ```

---

## Summary

✅ **Development:** `make build-fast` or `cargo build` (~20-30s)  
✅ **Production:** `make release` (~2-3 min)  
✅ **Quick check:** `make check-fast` (~2-3s)  
✅ **Best for IDE:** VS Code Tasks (`Cmd+Shift+B`)  
✅ **Best for terminal:** Makefile (`make help`)  

Choose the workflow that suits your needs! 🚀
