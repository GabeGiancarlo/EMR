# ADR-001: Frontend Stack

- **Status:** Accepted
- **Date:** 2026-05-03

## Decision

Future Nexus frontend implementation should use React + TypeScript.  
Leptos should not be reintroduced in this reset commit.

## Rationale

- Better ecosystem fit for dashboards, forms, tables, and calendar-heavy workflows.
- Faster product iteration for the current team profile.
- Better alignment with current frontend experience and hiring availability.
- Previous Leptos frontend was removed as part of architecture reset.

## Consequences

- Frontend bootstrap should target a React/TypeScript workspace when scheduled.
- If this direction changes later, a superseding ADR is required.
