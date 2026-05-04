# Nexus backend-focused development commands

EMR_PORT := "8090"

default:
    @just --list

dev:
    @echo "Starting Nexus API on port {{EMR_PORT}}"
    PORT={{EMR_PORT}} cargo run -p emr-api

api:
    @just dev

dev-stop:
    @echo "Stopping local Nexus API processes"
    @pkill -f "emr-api" || true
    @pkill -f "cargo run -p emr-api" || true

open-health:
    @echo "Opening health endpoint"
    @open "http://localhost:{{EMR_PORT}}/healthz"

api-test:
    @echo "Testing /healthz"
    @curl -s "http://localhost:{{EMR_PORT}}/healthz" | jq '.'
    @echo ""
    @echo "Testing /api/patients"
    @curl -s "http://localhost:{{EMR_PORT}}/api/patients" | jq '.'

fmt:
    cargo fmt --all

check:
    cargo check --workspace

test:
    cargo test --workspace

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

status:
    @echo "Nexus repo status"
    @echo "Rust: $(rustc --version)"
    @echo "Cargo: $(cargo --version)"
    @echo "Just: $(just --version)"
