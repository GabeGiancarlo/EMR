# EMR Platform - Development Commands
# ===================================

# Start the complete development environment
dev:
    @echo "🏥 Starting EMR Platform Development Environment..."
    @chmod +x scripts/dev-start.sh
    @./scripts/dev-start.sh

# Start just the API server
api:
    @echo "🚀 Starting API Server..."
    cd api && cargo run

# Stop the development environment
dev-stop:
    @echo "🛑 Stopping development servers..."
    @if [ -f .api_pid ]; then kill `cat .api_pid` && rm .api_pid; fi
    @pkill -f "cargo run" || true
    @echo "✅ Development servers stopped"

# Open the demo interface
demo:
    @echo "🌐 Opening EMR Platform Demo..."
    @open demo/index.html

# Test the API endpoints
api-test:
    @echo "🧪 Testing API endpoints..."
    @echo "📋 Health check:"
    @curl -s http://localhost:8080/healthz | jq '.' || echo "❌ API not running"
    @echo "\n👥 Patients list:"
    @curl -s http://localhost:8080/api/patients | jq '.patients[] | {name, email, status}' || echo "❌ API not running"

# Building commands
# =================

# Build all components
build:
    @echo "🔨 Building EMR Platform..."
    cargo build

# Build in release mode
build-release:
    @echo "🚀 Building EMR Platform (Release)..."
    cargo build --release

# Build just the API
build-api:
    @echo "🔨 Building API..."
    cd api && cargo build

# Testing commands
# ================

# Run all tests
test:
    @echo "🧪 Running tests..."
    cargo test

# Run tests with coverage
test-coverage:
    @echo "📊 Running tests with coverage..."
    cargo test --coverage

# API tests
test-api:
    @echo "🧪 Running API tests..."
    cd api && cargo test

# Linting and formatting
# ======================

# Run Clippy linter
lint:
    @echo "🔍 Running Clippy..."
    cargo clippy -- -D warnings

# Fix lint issues automatically
lint-fix:
    @echo "🔧 Fixing lint issues..."
    cargo clippy --fix --allow-dirty

# Format code
fmt:
    @echo "🎨 Formatting code..."
    cargo fmt

# Check formatting
fmt-check:
    @echo "🎨 Checking code formatting..."
    cargo fmt --check

# Check code without building
check:
    @echo "✅ Checking code..."
    cargo check

# Development tools
# =================

# Setup development environment
setup:
    @echo "⚙️  Setting up development environment..."
    @echo "📦 Installing Rust components..."
    rustup component add clippy rustfmt
    rustup target add wasm32-unknown-unknown
    @echo "🔧 Installing cargo tools..."
    cargo install just || true
    @echo "✅ Development environment ready!"

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    cargo clean

# Check for security vulnerabilities
audit:
    @echo "🔒 Running security audit..."
    cargo audit || echo "Install cargo-audit: cargo install cargo-audit"

# Check for outdated dependencies
outdated:
    @echo "📅 Checking for outdated dependencies..."
    cargo outdated || echo "Install cargo-outdated: cargo install cargo-outdated"

# Tree view of dependencies
tree:
    @echo "🌳 Dependency tree..."
    cargo tree

# Check project status
status:
    @echo "📊 Project Status"
    @echo "================="
    @echo "🦀 Rust Version: $(rustc --version)"
    @echo "📦 Cargo Version: $(cargo --version)"
    @echo "🔧 Just Version: $(just --version)"
    @echo ""
    @echo "📁 Project Structure:"
    @find . -name "Cargo.toml" -not -path "./target/*" | head -10
    @echo ""
    @echo "🏃‍♂️ Running Processes:"
    @ps aux | grep -E "(cargo|emr)" | grep -v grep || echo "No EMR processes running"

# Git and GitHub commands
# =======================

# Initial push to GitHub
github-push:
    @echo "🚀 Setting up GitHub repository..."
    @chmod +x scripts/initial-push.sh
    @./scripts/initial-push.sh

# Development report
report:
    @echo "📋 EMR Development Report"
    @echo "========================="
    @echo "📅 Date: $(date)"
    @echo "🏗️  Build Status:"
    @cargo check --quiet && echo "✅ Code compiles" || echo "❌ Compilation errors"
    @echo "🧪 Test Status:"
    @cargo test --quiet && echo "✅ All tests pass" || echo "❌ Test failures"
    @echo "🔍 Lint Status:"
    @cargo clippy --quiet && echo "✅ No lint issues" || echo "⚠️  Lint warnings"
    @echo "📊 Lines of Code:"
    @find . -name "*.rs" -not -path "./target/*" | xargs wc -l | tail -1
    @echo "📁 API Endpoints:"
    @echo "   GET /healthz"
    @echo "   GET /api/patients"
    @echo "   GET /api/patients/{id}" 