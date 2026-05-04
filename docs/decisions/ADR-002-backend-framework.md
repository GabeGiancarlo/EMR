# ADR-002: Backend Framework

- **Status:** Accepted
- **Date:** 2026-05-03

## Decision

Keep Actix Web as the backend framework for the current Nexus API foundation.

## Rationale

- Existing API scaffolding already uses Actix.
- Actix is a production-capable Rust web framework.
- The main current weakness is incomplete business/persistence implementation, not framework mismatch.
- Framework migration is deferred until a concrete technical reason exists.

## Consequences

- Near-term backend work should focus on architecture and domain correctness.
- Any future framework migration requires a separate ADR with migration cost analysis.
