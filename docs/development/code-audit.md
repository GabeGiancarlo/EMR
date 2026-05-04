# Code Audit

## Scope

Architecture reset audit for Nexus documentation, maintainability, and stale-reference cleanup.

## Active Code Paths

- `api/` is the active backend runtime path.
- Root development commands now target backend execution directly.

## Parked Folders

- `fhir/` - future interoperability layer.
- `jobs/` - future background worker runtime.

## Stale References Found And Addressed

- removed `demo/` routing and static serving from `api/src/main.rs`
- removed `scripts/` startup assumptions from `Makefile`, `justfile`, and `dev`
- removed `web` service assumptions from compose/devcontainer/workflow files where currently active
- replaced broad production/compliance claims in docs with planning-oriented language

## Commands Run

- `rg "demo|Leptos|leptos|web/|scripts/|HIPAA-compliant|production-ready|EMR Platform|Electronic Medical Record|mock|TODO|FIXME"`
- `cargo fmt`
- `cargo check`
- `cargo test`
- `cargo fmt && cargo check` (post-edit verification)
- `cargo test` (post-edit verification)
- `cargo check && cargo test` (post-dependency cleanup verification)

## Command Results

- `cargo fmt`: success.
- `cargo check`: initial sandboxed run failed with `zstd-sys` permission error (`Operation not permitted`), rerun outside sandbox succeeded.
- `cargo test`: success (workspace currently executes `api` tests; test runner reported `0` tests in `src/main.rs`).

## Risks / TODOs

- several non-workspace crates (`core`, `fhir`, `jobs`) still contain scaffold code not wired into the active build
- infra assets remain mixed between active backend needs and parked/future services
- auth/RBAC/audit controls remain partially scaffolded and require implementation before production use
- workspace test surface is currently minimal and needs meaningful async/unit/integration coverage

## Recommended Next Cleanup Steps

- split API into explicit route/handler/service/repository modules
- introduce SQLx migrations and replace mock patient flows
- define activation criteria and CI strategy for parked `fhir/` and `jobs` crates
- add integration tests for auth boundaries and audit-event behaviors
