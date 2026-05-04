# Backend Architecture

## Current Foundation

The active backend is Actix Web in `api/`.  
Current endpoints and route wiring are operational, but parts of the behavior still return mock/scaffold data.

## Intended Structure

As implementation matures, backend code should follow this shape:

- **app setup**: configuration, startup wiring, middleware registration.
- **shared app state**: database pool, clients, and service dependencies.
- **routes**: endpoint registration by bounded area.
- **handlers**: thin HTTP adapters (parse request, call service, return DTO).
- **services**: business logic and policy decisions.
- **repositories**: database persistence logic.
- **DTOs**: request/response models decoupled from internal domain models.
- **domain types**: reused from `core/` when stable.
- **structured errors**: typed errors with clear mapping to HTTP status codes.
- **validation**: input validation near boundary layers and service invariants.
- **async tests**: handler/service/repository tests using tokio-based async tests.

## Current Gaps

- Handler/service/repository boundaries are only partially established.
- Persistence is not yet database-backed for key clinical/billing flows.
- Auth/RBAC and audit logging are scaffolded but not enforced.
