# EMR Platform Development Commands
# 
# This Justfile provides convenient commands for development, testing, and deployment
# of the HIPAA-grade EMR platform.
#
# Usage: just <command>
# Example: just dev

# Default recipe
default:
    @just --list

# Development commands
# ===================

# Start development environment (builds and runs all services)
dev:
    @echo "🚀 Starting EMR development environment..."
    docker-compose -f infra/docker-compose.yml up --build

# Stop development environment
dev-stop:
    @echo "🛑 Stopping EMR development environment..."
    docker-compose -f infra/docker-compose.yml down

# Restart development environment
dev-restart:
    @echo "🔄 Restarting EMR development environment..."
    docker-compose -f infra/docker-compose.yml restart

# Build commands
# ==============

# Build all crates
build:
    @echo "🔨 Building all crates..."
    cargo build --workspace

# Build for release
build-release:
    @echo "🔨 Building all crates for release..."
    cargo build --workspace --release

# Build API only
build-api:
    @echo "🔨 Building API crate..."
    cargo build -p emr-api

# Build web frontend only
build-web:
    @echo "🔨 Building web frontend..."
    cargo build -p emr-web --features ssr

# Build jobs worker only
build-jobs:
    @echo "🔨 Building jobs worker..."
    cargo build -p emr-jobs

# Testing commands
# ================

# Run all tests
test:
    @echo "🧪 Running all tests..."
    cargo test --workspace

# Run tests with coverage
test-coverage:
    @echo "🧪 Running tests with coverage..."
    cargo tarpaulin --workspace --out html --output-dir target/coverage

# Run specific crate tests
test-core:
    cargo test -p emr-core

test-api:
    cargo test -p emr-api

test-fhir:
    cargo test -p emr-fhir

test-jobs:
    cargo test -p emr-jobs

test-web:
    cargo test -p emr-web

# Linting and formatting
# ======================

# Run Clippy lints
lint:
    @echo "🔍 Running Clippy lints..."
    cargo clippy --workspace -- -D warnings

# Fix Clippy issues automatically
lint-fix:
    @echo "🔧 Fixing Clippy issues..."
    cargo clippy --workspace --fix

# Format code
fmt:
    @echo "✨ Formatting code..."
    cargo fmt --all

# Check formatting
fmt-check:
    @echo "🔍 Checking code formatting..."
    cargo fmt --all --check

# Run all checks (format, lint, test)
check: fmt-check lint test
    @echo "✅ All checks passed!"

# Docker commands
# ===============

# Build Docker images
docker-build:
    @echo "🐳 Building Docker images..."
    docker-compose -f infra/docker-compose.yml build

# Start services with Docker
docker-up:
    @echo "🐳 Starting services with Docker..."
    docker-compose -f infra/docker-compose.yml up -d

# Stop Docker services
docker-down:
    @echo "🐳 Stopping Docker services..."
    docker-compose -f infra/docker-compose.yml down

# View Docker logs
docker-logs service="":
    @echo "📋 Viewing Docker logs for {{service}}..."
    @if [ "{{service}}" = "" ]; then \
        docker-compose -f infra/docker-compose.yml logs -f; \
    else \
        docker-compose -f infra/docker-compose.yml logs -f {{service}}; \
    fi

# Database commands
# =================

# Setup database
db-setup:
    @echo "🗄️  Setting up database..."
    # TODO: Add database setup commands
    @echo "Database setup not yet implemented"

# Run database migrations
db-migrate:
    @echo "🗄️  Running database migrations..."
    # TODO: Add migration commands
    @echo "Database migrations not yet implemented"

# Reset database
db-reset:
    @echo "🗄️  Resetting database..."
    # TODO: Add database reset commands
    @echo "Database reset not yet implemented"

# Git and GitHub commands
# =======================

# Initial push to GitHub
github-push:
    @echo "🚀 Setting up GitHub repository..."
    @chmod +x scripts/initial-push.sh
    @./scripts/initial-push.sh

# Development tools
# =================

# Install development dependencies
setup:
    @echo "🛠️  Installing development dependencies..."
    cargo install cargo-leptos
    cargo install cargo-tarpaulin
    cargo install diesel_cli --no-default-features --features postgres
    @echo "✅ Development dependencies installed!"

# Generate API documentation
docs:
    @echo "📚 Generating documentation..."
    cargo doc --workspace --no-deps --open

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    cargo clean
    docker system prune -f

# Security and compliance
# =======================

# Run security audit
audit:
    @echo "🔒 Running security audit..."
    cargo audit

# Check for outdated dependencies
outdated:
    @echo "📋 Checking for outdated dependencies..."
    cargo outdated

# Check licenses
licenses:
    @echo "📄 Checking licenses..."
    cargo license

# Production commands
# ===================

# Deploy to staging
deploy-staging:
    @echo "🚀 Deploying to staging..."
    # TODO: Add staging deployment commands
    @echo "Staging deployment not yet implemented"

# Deploy to production
deploy-prod:
    @echo "🚀 Deploying to production..."
    # TODO: Add production deployment commands
    @echo "Production deployment not yet implemented"

# Health check
health:
    @echo "🏥 Checking service health..."
    curl -f http://localhost:8080/healthz || echo "❌ API health check failed"
    curl -f http://localhost:3000/api/health || echo "❌ Web health check failed"

# Utility commands
# ================

# Show workspace structure
tree:
    @echo "📁 Workspace structure:"
    tree -I 'target|node_modules|.git' -L 3

# Show project status
status:
    @echo "📊 Project Status"
    @echo "=================="
    @echo "Git status:"
    git status --short
    @echo ""
    @echo "Workspace info:"
    cargo tree --workspace --depth 1
    @echo ""
    @echo "Docker containers:"
    docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

# Generate project report
report:
    @echo "📊 Generating project report..."
    @echo "# EMR Platform Report" > report.md
    @echo "Generated on: $(date)" >> report.md
    @echo "" >> report.md
    @echo "## Test Results" >> report.md
    cargo test --workspace 2>&1 | tee -a report.md || true
    @echo "" >> report.md
    @echo "## Lint Results" >> report.md
    cargo clippy --workspace 2>&1 | tee -a report.md || true
    @echo "📊 Report generated: report.md" 