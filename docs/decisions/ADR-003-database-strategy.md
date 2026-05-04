# ADR-003: Database Strategy

- **Status:** Accepted
- **Date:** 2026-05-03

## Decision

Adopt Postgres with SQLx-style migration discipline for Nexus backend persistence.

## Rationale

- Clinical, billing, and audit data is relational and consistency-sensitive.
- Postgres is a strong fit for transactional workflows and reporting workloads.
- SQLx aligns with async Rust and explicit migration/versioning workflows.

## Consequences

- Data access should converge on repository patterns with explicit SQL and typed models.
- Schema evolution should be tracked in migration files with test coverage.
