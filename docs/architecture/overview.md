# Architecture Overview

## Current State

- Active implementation is the Rust API in `api/`.
- `api/` currently exposes scaffold-level endpoints and includes mock behavior.
- `core/` exists as a domain-layer seed, not a complete domain model.

## Planned State

- Backend evolves into clear route/handler/service/repository boundaries.
- Postgres + migration discipline becomes the persistence baseline.
- A React + TypeScript frontend is planned for role-based workspaces.

## Parked Areas

- `fhir/` is parked for future interoperability work.
- `jobs/` is parked for future background automation/runtime work.

## Removed Areas

- Static `demo/` has been removed.
- Previous Leptos/Rust `web/` frontend has been removed.
- Legacy script-based development paths were removed or retired.

## How the Areas Relate

- `api/` defines current HTTP behavior and integration points.
- `core/` should provide reusable domain concepts independent of transport/storage.
- Future frontend should consume backend APIs and align to role workspaces.
- `infra/` contains environment and deployment references, but still needs production hardening.
- `fhir/` and `jobs/` should remain isolated until clear activation milestones are met.
