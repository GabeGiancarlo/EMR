# api

## Purpose

Active Rust API backend foundation for Nexus.

## Current Status

Active, with prototype/scaffold behavior still present in parts of the API.

## What Belongs Here

- Actix app setup and route registration
- HTTP handlers
- service-layer business logic
- repository-layer persistence logic
- middleware, config, database wiring, and error handling

## What Does Not Belong Here

- frontend UI code
- ad hoc scripts that bypass typed backend boundaries
- domain logic that should live in `core/`

## Notes

Current endpoints include mock responses while Postgres-backed workflows are implemented in upcoming milestones.
