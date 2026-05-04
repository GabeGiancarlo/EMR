# ADR-004: Product Architecture

- **Status:** Accepted
- **Date:** 2026-05-03

## Decision

Build Nexus around three role-based workspaces:

- Clinician
- Billing
- Executive / Leadership

## Rationale

- Matches the actual Nexus product direction for recovery-center operations.
- Aligns technical milestones with real operational users.
- Reduces drift back to generic patient CRUD demo patterns.

## Consequences

- Roadmap and API evolution should map to role-centered workflows.
- New feature proposals should identify the target role workspace.
