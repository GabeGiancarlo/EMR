# PHI Handling

## Purpose

Defines PHI handling expectations for future Nexus development.

## Development Guidance

- avoid logging PHI in application logs, traces, or error payloads
- audit sensitive reads/writes where possible
- avoid unnecessary PHI exposure in frontend/API responses
- keep internal domain models separate from response DTOs
- restrict role access by least privilege
- use safe, synthetic seed/test data by default
- document any workflow that touches PHI end-to-end

## Data Boundary Practices

- prefer explicit DTOs over returning internal structs directly
- include only fields required for the caller use case
- treat exported reports and files as PHI-bearing unless proven otherwise

## Status

These are implementation expectations, not proof of compliance.  
Additional controls, policy mapping, and validation are required before production use.
