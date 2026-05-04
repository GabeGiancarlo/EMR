# Nexus

## Overview

Nexus is a recovery-focused EMR, billing, and executive operations platform designed around the daily workflows of clinicians, billing teams, and leadership.  
This repository currently provides an early Rust backend foundation and architecture scaffolding for that direction.

## Current Status

The repository is in an architecture reset.

- The old static demo has been removed.
- The old Leptos/Rust frontend has been removed.
- Legacy script-driven startup paths have been replaced with backend-first commands.
- The Rust API crate remains the active implementation surface.

Current code includes prototype/mock behavior and is not yet production-ready.

## Product Direction

Nexus is being designed around three workspaces:

- Clinician
- Billing
- Executive / Leadership

These role-based workflows are the core product direction and future implementation priority.

## Architecture at a Glance

- **Active now:** Rust API backend (`api/`) using Actix Web.
- **Domain seed:** `core/` contains early domain abstractions and contracts.
- **Planned frontend:** React + TypeScript (documented decision; no active frontend in this repo yet).
- **Persistence direction:** Postgres with migration discipline (SQLx planned).
- **Parked areas:** `fhir/` and `jobs/` are retained as future integration/automation zones.
- **Infrastructure:** `infra/` contains useful references, but should not be treated as production-ready.

## Repository Structure

- `api/` - active Rust API backend foundation.
- `core/` - domain-layer seed and shared domain contracts.
- `fhir/` - parked FHIR interoperability experiments/plans.
- `jobs/` - parked background worker/runtime area.
- `infra/` - infrastructure, container, and database reference assets.
- `.github/` - CI/CD workflow definitions (currently backend-focused).
- `.devcontainer/` - devcontainer configuration for local contributor setup.
- `.cursor/` - Cursor AI guidance rules and conventions.
- `docs/` - product, architecture, compliance, and development documentation.

## What Works Today

Based on the current repository state:

- `api` crate runs locally with Actix (`cargo run -p emr-api`).
- Health endpoint is available at `/healthz`.
- Prototype patient endpoints are available at:
  - `/api/patients`
  - `/api/patients/{id}`
- `cargo fmt`, `cargo check`, and `cargo test` are wired for the current workspace.

## What Is Not Built Yet

- Production authentication and RBAC.
- Real database-backed clinical and billing workflows.
- Enforced audit logging across sensitive reads/writes.
- Active React frontend implementation.
- Billing engine and billing work queue execution.
- Executive dashboards and leadership action workflows.
- Production FHIR integration pipeline.
- Active jobs/worker runtime integrated with production data flows.

## Development Setup

### Prerequisites

- Rust (stable toolchain)
- `just` (optional)

### Run the API

```bash
cargo run -p emr-api
```

or:

```bash
just dev
```

### Quick checks

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

### API smoke checks

```bash
curl http://127.0.0.1:8090/healthz
curl http://127.0.0.1:8090/api/patients
```

## Roadmap / Phases

### Phase 0 — Repo Reset and Architecture Alignment

- Remove stale demo/web/script assumptions.
- Document current repo truth.
- Define Nexus product direction.
- Decide active vs. parked folders.
- Establish documentation and coding standards.

### Phase 1 — Backend Foundation

- Refactor API into clear app/state/routes/modules.
- Add Postgres + SQLx migrations.
- Replace mock patient/client behavior with database-backed records.
- Add organization, facility, user, role, and client foundations.
- Add validation, structured errors, and async tests.
- Add initial audit-event model.

### Phase 2 — Security, Access, and Audit Foundation

- Authentication.
- Role-based access control.
- Organization/facility scoping.
- PHI-safe logging.
- Audit logging for sensitive reads/writes.
- Compliance-oriented documentation.

### Phase 3 — Clinician Workspace

- Clinician dashboard.
- Client chart.
- Calendar/sessions.
- Clinical notes.
- Treatment plans.
- Documents/forms.
- Tasks and alerts.

### Phase 4 — Billing Workspace

- Insurance policies.
- Authorizations.
- Claims.
- Denials and appeals.
- Payments and balances.
- Billing work queue.
- Claim/documentation readiness.

### Phase 5 — Executive / Leadership Workspace

- Census and capacity visibility.
- Revenue and claim-health summaries.
- Documentation compliance.
- Staff/task visibility.
- Outcomes and risk indicators.
- Executive action queue.

### Phase 6 — Integrations and Automation

- FHIR import/export.
- Kipu/QuickBooks migration strategy.
- Background jobs.
- Eligibility checks.
- Report generation.
- Claim status polling.

### Phase 7 — Production Readiness

- CI hardening.
- Docker/deployment cleanup.
- Secrets management.
- Monitoring and observability.
- Backup/restore strategy.
- Security review.
- Performance testing.
- Compliance readiness review.

## Current TODO

- Finalize API module boundaries (`routes`, `handlers`, `services`, `repositories`).
- Implement Postgres persistence with migrations for first role-aligned entities.
- Add auth bootstrap (identity, sessions/tokens, role model).
- Add audit-event model and PHI-safe logging policy in code.
- Replace mock patient responses with repository-backed DTOs.
- Define React/TypeScript frontend bootstrap plan and repo integration approach.
- Decide activation criteria for parked `fhir/` and `jobs/` folders.

## Documentation

See [`docs/README.md`](docs/README.md) for product, architecture, decision records, compliance planning notes, and development guidance.
