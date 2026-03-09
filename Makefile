# Automated Makefile for building Rust Lambda functions
# Automatically discovers binaries and their required features from Cargo.toml
# No manual editing required - just add/remove binaries and update Cargo.toml

# Variables
RUST_TARGET=aarch64-unknown-linux-gnu
CARGO_OPTS=--release --target $(RUST_TARGET)

# Automatically discover binary names from src/bin directory
BINARIES := $(patsubst src/bin/%.rs,%,$(wildcard src/bin/*.rs))

# Parse Cargo.toml in-memory (no temp files)
define get_cargo_info
$(shell ./parse_cargo.py | grep "^$(1):")
endef

define get_cargo_name
$(shell echo "$(call get_cargo_info,$(1))" | cut -d: -f2)
endef

define get_features
$(shell echo "$(call get_cargo_info,$(1))" | cut -d: -f3)
endef

# Default target
.PHONY: help
help:
	@echo "🤖 Automated Rust Lambda Builder"
	@echo "================================="
	@echo "📦 Discovered binaries: $(BINARIES)"
	@echo ""
	@echo "🔧 Build Commands:"
	@echo "  make build           - Build all binaries"
	@echo "  make build-all       - Build all binaries (alias)"
	@echo "  make build-<name>    - Build specific binary (e.g., make build-checkEmail)"
	@echo ""
	@echo "📋 Info Commands:"
	@echo "  make list            - List all binaries and features (alias: list-binaries)"
	@echo "  make list-binaries   - List all binaries and features"
	@echo "  make info            - Show environment information"
	@echo ""
	@echo "🛠️ Utility Commands:"
	@echo "  make clean           - Clean build artifacts"
	@echo "  make check           - Check code"
	@echo "  make test            - Run tests"
	@echo "  make sizes           - Show binary sizes"

# Handle different build commands
.PHONY: build build-all build-cargo-only
build: build-all
build-all:
	@echo "🔨 Building all binaries with Cargo Lambda..."
	@$(foreach binary,$(BINARIES), $(MAKE) --no-print-directory build-cargo-$(binary);)
	@$(MAKE) --no-print-directory copy-to-sam
	@echo "✅ All binaries built and copied to SAM directory!"

build-cargo-only:
	@echo "🔨 Building all binaries with Cargo Lambda only..."
	@$(foreach binary,$(BINARIES), $(MAKE) --no-print-directory build-cargo-$(binary);)
	@$(MAKE) --no-print-directory copy-to-sam
	@echo "✅ All binaries built and copied!"

# List commands
.PHONY: list list-binaries
list: list-binaries
list-binaries:
	@echo "🔍 Discovered binaries and their features:"
	@echo "=========================================="
	@$(foreach binary,$(BINARIES), \
		CARGO_NAME=$$(echo "$(call get_cargo_info,$(binary))" | cut -d: -f2); \
		FEATURES=$$(echo "$(call get_cargo_info,$(binary))" | cut -d: -f3); \
		echo "📁 $(binary):"; \
		echo "   Cargo name: $$CARGO_NAME"; \
		echo "   Features: $$FEATURES"; \
		echo ""; \
	)

# Dynamic build targets for each binary (file-based names)
.PHONY: $(addprefix build-,$(BINARIES))
$(addprefix build-,$(BINARIES)): build-%:
	@$(MAKE) --no-print-directory build-cargo-$*
	@cp target/lambda/$*/bootstrap .aws-sam/build/$*/;
	@echo "✅ Copied $*";

# Cargo Lambda build targets for each binary
.PHONY: $(addprefix build-cargo-,$(BINARIES))
$(addprefix build-cargo-,$(BINARIES)): build-cargo-%:
	@echo "🔨 Building $* binary..."
	@CARGO_NAME=$$(echo "$(call get_cargo_info,$*)" | cut -d: -f2); \
	FEATURES=$$(echo "$(call get_cargo_info,$*)" | cut -d: -f3); \
	if [ -z "$$CARGO_NAME" ]; then \
		echo "❌ Binary $* not found in Cargo.toml"; \
		exit 1; \
	fi; \
	if [ -n "$$FEATURES" ]; then \
		echo "   Building $$CARGO_NAME with features: $$FEATURES"; \
		cargo lambda build $(CARGO_OPTS) --bin $$CARGO_NAME --features "$$FEATURES"; \
	else \
		echo "   Building $$CARGO_NAME with no features"; \
		cargo lambda build $(CARGO_OPTS) --bin $$CARGO_NAME --no-default-features; \
	fi
	@echo "✅ $* binary built successfully!"

# Automatically copy all built binaries to SAM build directory
.PHONY: copy-to-sam
copy-to-sam:
	@echo "📁 Copying binaries to SAM build directory..."
	@$(foreach binary,$(BINARIES), \
		CARGO_NAME=$$(echo "$(call get_cargo_info,$(binary))" | cut -d: -f2); \
		if [ -n "$$CARGO_NAME" ] && [ -f target/lambda/$$CARGO_NAME/bootstrap ]; then \
			mkdir -p .aws-sam/build/$$CARGO_NAME; \
			cp target/lambda/$$CARGO_NAME/bootstrap .aws-sam/build/$$CARGO_NAME/; \
			echo "✅ Copied $$CARGO_NAME"; \
		fi; \
	)

# Show binary sizes automatically
.PHONY: sizes
sizes:
	@echo "📊 Binary sizes for all discovered binaries:"
	@echo "==========================================="
	@$(foreach binary,$(BINARIES), \
		CARGO_NAME=$$(echo "$(call get_cargo_info,$(binary))" | cut -d: -f2); \
		if [ -f target/lambda/$$CARGO_NAME/bootstrap ]; then \
			ls -lh target/lambda/$$CARGO_NAME/bootstrap | awk '{print "📦 '$$CARGO_NAME': " $$5}'; \
		else \
			echo "❌ $$CARGO_NAME: not built"; \
		fi; \
	)
	@echo ""
	@echo "💡 Run 'make build' to build missing binaries"

# Clean build artifacts
.PHONY: clean
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf .aws-sam/build
	@echo "✅ Clean completed!"

# Run tests
.PHONY: test
test:
	@echo "🧪 Running tests..."
	cargo test --features dev
	@echo "✅ Tests completed!"

# Check code with all features enabled
.PHONY: check
check:
	@echo "🔍 Checking code with all features..."
	cargo check --features dev
	@echo "✅ Check completed!"

# Auto-discover and show environment info
.PHONY: info
info:
	@echo "🔍 Environment Information:"
	@echo "=========================="
	@echo "Rust version: $$(rustc --version)"
	@echo "Cargo Lambda version: $$(cargo lambda --version 2>/dev/null || echo 'Not installed')"
	@echo "SAM CLI version: $$(sam --version 2>/dev/null || echo 'Not installed')"
	@echo "Target architecture: $(RUST_TARGET)"
	@echo ""
	@echo "📦 Auto-discovered binaries:"
	@$(foreach binary,$(BINARIES), \
		CARGO_NAME=$$(echo "$(call get_cargo_info,$(binary))" | cut -d: -f2); \
		FEATURES=$$(echo "$(call get_cargo_info,$(binary))" | cut -d: -f3); \
		echo "   $(binary) -> $$CARGO_NAME [$$FEATURES]"; \
	)

# Validate Cargo.toml configuration
.PHONY: validate-config
validate-config:
	@echo "🔍 Validating Cargo.toml configuration..."
	@$(foreach binary,$(BINARIES), \
		CARGO_NAME=$$(echo "$(call get_cargo_info,$(binary))" | cut -d: -f2); \
		if [ -n "$$CARGO_NAME" ]; then \
			echo "✅ $(binary) -> $$CARGO_NAME configured"; \
		else \
			echo "❌ $(binary) not found in Cargo.toml [[bin]] sections"; \
		fi; \
	)
	@echo "✅ Configuration validation completed!"

# Development workflow
.PHONY: dev
dev: validate-config check test build sizes
	@echo "✅ Automated development workflow completed!"

# Utility commands
.PHONY: fmt clippy watch install-tools
fmt:
	@echo "📝 Formatting code..."
	cargo fmt
	@echo "✅ Formatting completed!"

clippy:
	@echo "📎 Running clippy..."
	cargo clippy --features dev --all-targets -- -D warnings
	@echo "✅ Clippy completed!"

watch:
	@echo "👀 Watching for changes..."
	cargo watch -x "make build"

install-tools:
	@echo "🛠️ Installing required tools..."
	cargo install cargo-lambda
	cargo install cargo-watch
	@echo "✅ Tools installed!"
