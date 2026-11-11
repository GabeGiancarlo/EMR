# Optional POSIX helpers. Windows contributors should use tools/start-api.ps1.

EMR_PORT := "8090"

default:
    @just --list

dev:
    @echo "Starting Nexus API on port {{EMR_PORT}}"
    NEXUS_LOCAL_DEV=1 HOST=127.0.0.1 PORT={{EMR_PORT}} cargo run -p emr-api

api:
    @just dev

dev-stop:
    @echo "Press Ctrl+C in the terminal that started this API. Other checkouts are not stopped."

open-health:
    @echo "Opening health endpoint"
    @open "http://localhost:{{EMR_PORT}}/healthz"

api-test:
    @echo "Testing /healthz"
    @curl -s "http://localhost:{{EMR_PORT}}/healthz" | jq '.'
    @echo ""
    @echo "Testing /api/bootstrap"
    @curl -fsS "http://localhost:{{EMR_PORT}}/api/bootstrap" | jq '.'

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
