# Database Strategy

## Direction

Nexus is moving toward Postgres with explicit migration discipline (SQLx planned).

## Why Postgres

Recovery EMR and billing workflows are relational and require strong consistency for:

- client and episode records
- billing entities and lifecycle transitions
- authorization and claim state tracking
- audit events and reporting

## Migration Approach

- Keep schema changes in versioned migrations.
- Require migration tests for non-trivial schema changes.
- Coordinate application-layer updates with schema updates in the same milestone.

## Likely Early Tables

These are planned starting points, not guaranteed current implementation:

- organizations
- facilities
- users
- roles
- clients
- episodes_of_care
- audit_events
- tasks
- sessions
- clinical_notes
- insurance_policies
- authorizations
- claims

## Current Gap

The repo currently has infrastructure and SQL reference material, but the complete production schema and migration flow are still in progress.
