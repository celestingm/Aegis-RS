# Aegis-RS Makefile

.PHONY: all build check test clean run fmt lint ci

# Default target
all: check test build

# Build the project
build:
	cargo build --release

# Check for compilation errors
check:
	cargo check

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean

# Run the application
run:
	cargo run --release

# Format code
fmt:
	cargo fmt

# Lint code
lint:
	cargo clippy -- -D warnings

# Run full CI suite locally (Format, Lint, Test, Build)
ci: fmt lint test build
	@echo "All CI checks passed! ✅"
