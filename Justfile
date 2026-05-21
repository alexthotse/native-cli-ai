# NCA Justfile - Build Automation
# Replace all shell scripts with this comprehensive justfile

[variables]
CARGO := cargo
JUST := just

# Default target
default:
    @echo "🚀 Native CLI AI (NCA)"
    @echo "Usage: just <recipe>"
    @echo ""
    @echo "Development:"
    @echo "  just dev          - Build and run in development mode"
    @echo "  just build        - Build release version"
    @echo "  just test         - Run all tests"
    @echo "  just lint         - Run linter"
    @echo "  just fix          - Auto-fix linting issues"
    @echo "  just check        - Check code without building"
    @echo ""
    @echo "Running:"
    @echo "  just              - Run main NCA binary"
    @echo "  just run-provider <provider> - Run with specific provider"
    @echo "  just run-model <model>       - Run with specific model"
    @echo ""
    @echo "AI Components:"
    @echo "  just agents       - Run Agent OS demo"
    @echo "  just rag          - Test RAG pipeline"
    @echo "  just aider        - Run Aider Rust"
    @echo "  just harness      - Run self-healing harness"
    @echo "  just swarm        - Run multi-agent swarm"
    @echo "  just heal <file>  - Auto-repair specific file"
    @echo "  just evolve       - Run continuous improvement mode"
    @echo ""
    @echo "Project Management:"
    @echo "  just new-crate <name> - Create new crate"
    @echo "  just docs         - Generate documentation"
    @echo "  just clean        - Clean build artifacts"
    @echo ""
    @echo "Release:"
    @echo "  just release      - Create release build"
    @echo "  just publish      - Publish to crates.io"

# Development recipes
dev:
    @echo "🔨 Building in development mode..."
    {{CARGO}} build --workspace

build:
    @echo "🏗️  Building release version..."
    {{CARGO}} build --release --workspace

test:
    @echo "🧪 Running all tests..."
    {{CARGO}} test --workspace

test-harness:
    @echo "🧪 Running harness tests..."
    {{CARGO}} test -p nca-harness

lint:
    @echo "🔍 Running linter..."
    {{CARGO}} clippy --workspace -- -D warnings

fix:
    @echo "🔧 Auto-fixing linting issues..."
    {{CARGO}} clippy --workspace --fix --allow-staged

check:
    @echo "✅ Checking code..."
    {{CARGO}} check --workspace

# Running recipes
run: build
    @echo "🚀 Running NCA..."
    ./target/release/nca

run-provider:
    @if [ -z "{{provider}}" ]; then \
        echo "❌ Error: provider argument required"; \
        echo "Usage: just run-provider <provider>"; \
        exit 1; \
    fi
    @echo "🚀 Running NCA with provider: {{provider}}"
    {{CARGO}} run --provider {{provider}}

run-model:
    @if [ -z "{{model}}" ]; then \
        echo "❌ Error: model argument required"; \
        echo "Usage: just run-model <model>"; \
        exit 1; \
    fi
    @echo "🚀 Running NCA with model: {{model}}"
    {{CARGO}} run --model {{model}}

# AI Component recipes
agents:
    @echo "🤖 Running Agent OS demo..."
    {{CARGO}} run -p agent_os

rag:
    @echo "🔍 Testing RAG pipeline..."
    {{CARGO}} test -p nca-rag -- --nocapture

aider:
    @echo "💻 Running Aider Rust..."
    {{CARGO}} run -p aider_rs

harness:
    @echo "⚙️  Running self-healing harness..."
    {{CARGO}} run -p nca-harness

swarm:
    @echo "🐝 Running multi-agent swarm..."
    {{CARGO}} test -p nca-harness swarm -- --nocapture

heal:
    @if [ -z "{{file}}" ]; then \
        echo "❌ Error: file argument required"; \
        echo "Usage: just heal <file>"; \
        exit 1; \
    fi
    @echo "🩹 Auto-repairing file: {{file}}"
    {{CARGO}} run -p nca-harness -- --heal {{file}}

evolve:
    @echo "🔄 Running continuous improvement mode..."
    {{CARGO}} run -p nca-harness -- --evolve

# Project management recipes
new-crate:
    @if [ -z "{{name}}" ]; then \
        echo "❌ Error: name argument required"; \
        echo "Usage: just new-crate <name>"; \
        exit 1; \
    fi
    @echo "📦 Creating new crate: {{name}}"
    mkdir -p crates/{{name}}/src
    cd crates/{{name}} && cargo init --lib

docs:
    @echo "📚 Generating documentation..."
    {{CARGO}} doc --workspace --no-deps

clean:
    @echo "🧹 Cleaning build artifacts..."
    {{CARGO}} clean

# Release recipes
release:
    @echo "📦 Creating release build..."
    {{CARGO}} build --release --workspace
    @echo "✅ Release build complete!"
    @ls -lh target/release/

publish:
    @echo "⚠️  Publishing to crates.io..."
    @echo "This will publish all workspace crates. Continue? (y/N)"
    @read confirm && [ "$$confirm" = "y" ] || exit 1
    {{CARGO}} publish -p nca-common
    {{CARGO}} publish -p nca-core
    {{CARGO}} publish -p nca-runtime
    {{CARGO}} publish -p nca-cli

# Provider-specific helpers
providers:
    @echo "📡 Available Providers:"
    @echo "  - minimax (default)"
    @echo "  - openai"
    @echo "  - anthropic"
    @echo "  - openrouter"
    @echo "  - nvidia"
    @echo "  - opencode"
    @echo "  - glm"
    @echo "  - kimi"
    @echo "  - kilocode"
    @echo "  - ollama"
    @echo "  - ollamacloud"
    @echo "  - groq"
    @echo "  - together"
    @echo "  - fireworks"
    @echo "  - deepseek"
    @echo "  - cohere"
    @echo "  - sambanova"
    @echo "  - replicate"
    @echo "  - anyscale"
    @echo "  - perplexity"
    @echo "  - mistral"
    @echo "  - ai21"
    @echo ""
    @echo "Set provider with: export NCA_DEFAULT_PROVIDER=<name>"

# Environment setup
setup:
    @echo "🔧 Setting up development environment..."
    rustup update
    {{CARGO}} install just
    {{CARGO}} install cargo-watch
    @echo "✅ Setup complete!"

watch:
    @echo "👀 Watching for changes..."
    {{CARGO}} watch

# Benchmark
bench:
    @echo "⚡ Running benchmarks..."
    {{CARGO}} bench --workspace

# Coverage (requires cargo-tarpaulin)
coverage:
    @echo "📊 Generating test coverage..."
    {{CARGO}} tarpaulin --workspace --out Html

# Format all code
fmt:
    @echo "✨ Formatting all code..."
    {{CARGO}} fmt --all

# Verify all providers compile
verify-providers:
    @echo "🔍 Verifying all provider configurations..."
    {{CARGO}} check --workspace --all-features

# Quick test for specific component
quick-test:
    @echo "⚡ Quick test (no network)..."
    {{CARGO}} test --workspace --lib

# Integration tests
integration:
    @echo "🔗 Running integration tests..."
    {{CARGO}} test --workspace --test '*' -- --ignored

# Debug build with symbols
debug:
    @echo "🐛 Building debug version with symbols..."
    {{CARGO}} build --workspace

# Profile build
profile:
    @echo "📈 Building with profiling enabled..."
    {{CARGO}} build --release --workspace
    @echo "Use with: cargo flamegraph or samply"

# Static analysis
analyze:
    @echo "🔬 Running static analysis..."
    {{CARGO}} miri setup
    {{CARGO}} +nightly miri test

# Security audit
audit:
    @echo "🔒 Running security audit..."
    {{CARGO}} audit

# Update dependencies
update:
    @echo "🔄 Updating dependencies..."
    {{CARGO}} update

# Show dependency tree
tree:
    @echo "🌳 Dependency tree:"
    {{CARGO}} tree

# List all binaries
binaries:
    @echo "📦 Available binaries:"
    {{CARGO}} metadata --format-version 1 | jq '.packages[] | select(.kind[] == "bin") | .name'
