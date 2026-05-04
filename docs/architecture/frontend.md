# Frontend Direction

## Current Status

There is no active frontend implementation in this repository at the moment.

## Decision

Nexus frontend direction is React + TypeScript unless changed by a future ADR.

## Context

The previous Leptos/Rust frontend was removed during the architecture reset and should not be reintroduced in this commit.

## Rationale

React + TypeScript is currently preferred for this team because it better supports:

- dashboard-heavy experiences
- form-heavy workflows
- table/grid-heavy data operations
- scheduling/calendar interactions
- faster product iteration with broad ecosystem support
