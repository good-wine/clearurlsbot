.PHONY: check build release clean clippy fmt test help build-fast check-fast

# Variables
BINARY_NAME=clear_urls_bot
RELEASE_BINARY=target/release/$(BINARY_NAME)
BINARY_SIZE=$(shell ls -lh $(RELEASE_BINARY) 2>/dev/null | awk '{print $$5}')

help:
	@echo "🚀 ClearURLs Bot - Build Commands"
	@echo ""
	@echo "Development:"
	@echo "  make check-fast     - Quick syntax check (2-3s)"
	@echo "  make build-fast     - Fast debug build with optimizations"
	@echo "  make test           - Run tests"
	@echo "  make clippy         - Run linter"
	@echo "  make fmt            - Format code"
	@echo ""
	@echo "Production:"
	@echo "  make release        - Full optimized release build"
	@echo "  make release-strip  - Release with stripped symbols"
	@echo ""
	@echo "Maintenance:"
	@echo "  make clean          - Remove build artifacts"
	@echo "  make size           - Show binary size"
	@echo "  make all            - Build all (check + build + test)"

# Fast syntax check (lowest level)
check-fast:
	@echo "🔍 Running quick syntax check..."
	cargo check --release

# Fast build for development
build-fast:
	@echo "⚡ Building debug (optimized)..."
	cargo build

# Full release build with all optimizations
release:
	@echo "🏗️  Building optimized release..."
	time cargo build --release
	@echo ""
	@echo "✅ Release binary: $(RELEASE_BINARY)"
	@echo "📦 Size: $(shell ls -lh $(RELEASE_BINARY) 2>/dev/null | awk '{print $$5}')"

# Release with debug symbols (for profiling)
release-debug:
	@echo "🔐 Building release with debug info..."
	cargo build --profile release-with-debug
	@echo "✅ Binary with debug: target/release-with-debug/$(BINARY_NAME)"

# Strip binary to reduce size
release-strip: release
	@echo "✂️  Stripping debug symbols..."
	strip $(RELEASE_BINARY)
	@echo "📦 Stripped size: $(shell ls -lh $(RELEASE_BINARY) | awk '{print $$5}')"

# Check code quality
clippy:
	@echo "🔎 Running Clippy linter..."
	cargo clippy --release -- -D warnings

# Format code
fmt:
	@echo "💅 Formatting code..."
	cargo fmt -- --check
	@echo "✅ Code format check passed"

fmt-fix:
	@echo "💅 Auto-formatting code..."
	cargo fmt

# Run tests
test:
	@echo "🧪 Running tests..."
	cargo test --release

# Full build pipeline
all: check-fast build-fast clippy test
	@echo "✅ All checks passed!"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf target/
	@echo "✅ Clean complete"

# Show binary size
size:
	@if [ -f "$(RELEASE_BINARY)" ]; then \
		echo "📦 Binary size analysis:"; \
		ls -lh $(RELEASE_BINARY) | awk '{print "  Release: " $$5 " (" $$9 ")"}'; \
		file $(RELEASE_BINARY); \
		echo ""; \
		echo "💾 Total target dir size:"; \
		du -sh target/; \
	else \
		echo "❌ Release binary not found. Run: make release"; \
	fi

# Development workflow (fast)
dev: check-fast fmt-fix
	@echo "🎯 Development build ready"

# CI/CD workflow (strict)
ci: clean check-fast clippy test
	@echo "✅ CI checks passed"

# Rebuild everything
rebuild: clean release
	@echo "🔄 Rebuild complete"

# Show compiler version
version:
	@echo "🦀 Rust Toolchain:"
	rustc --version
	cargo --version
	@echo ""
	@echo "📦 Cargo features:"
	cargo tree --depth=1 2>/dev/null || cargo tree | head -20

.DEFAULT_GOAL := help
