#!/bin/bash

# Nexus devcontainer post-create script.
# Keeps setup intentionally lightweight for the current backend-focused phase.

set -euo pipefail

echo "Setting up Nexus development container..."

echo "Installing Rust components..."
rustup component add rustfmt clippy llvm-tools-preview

echo "Installing optional cargo tooling..."
cargo install --locked cargo-nextest || true
cargo install --locked cargo-audit || true

echo "Preparing workspace defaults..."
mkdir -p /workspace/logs /workspace/tmp

if [ ! -f /workspace/.env ]; then
    cp /workspace/infra/environment.template /workspace/.env
    echo "Created .env from infra/environment.template"
fi

echo "Pre-fetching cargo dependencies..."
cd /workspace
cargo fetch --locked

echo ""
echo "Nexus devcontainer ready."
echo "Run one of:"
echo "  just dev    # start API"
echo "  just check  # cargo check"
echo "  just test   # cargo test"
