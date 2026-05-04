# Local Setup

## Current Path

The active local development path is backend-first using the Rust API crate.

## Prerequisites

- Rust toolchain (stable)
- `cargo`
- optional: `just`

## Run API

```bash
cargo run -p emr-api
```

or:

```bash
just dev
```

## Verify Endpoints

```bash
curl http://127.0.0.1:8090/healthz
curl http://127.0.0.1:8090/api/patients
```

## Quality Checks

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

## Current Setup Gaps

- Database-backed workflows are not fully wired.
- Auth/RBAC/audit enforcement is not complete.
- No active frontend workspace is included yet.

## Next Setup Tasks

- add migration workflow for Postgres schema evolution
- add environment validation for required secrets/config keys
- add integration test harness with test database lifecycle
