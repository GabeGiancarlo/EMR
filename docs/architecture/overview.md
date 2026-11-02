# Architecture overview

Nexus has a persistent local application: a custom React/TypeScript interface, a Rust/Actix API, and PostgreSQL with versioned SQLx migrations. The ordinary database starts empty; the earlier mock API has been replaced.

The same stored client, episode and session link care documentation to claims readiness. Signed records remain immutable; corrections preserve originals. Financial transitions reserve authorization units and serialize payments. Executive metrics come from stored records with clinical narrative restricted by role.

Authentication is deliberately deferred. Local operator selection exercises policies but does not establish identity. Startup requires development mode and a loopback bind. Facility relationships exist, but a complete facility ACL is not implemented.

The active build includes `api/` and `frontend/`. `core/`, `fhir/` and `jobs/` remain parked legacy material. Use root `compose.local.yml` or the portable database helper; older infrastructure files are not a complete deployment design.

See the [data model](data-model.md), [API contract](api-contract.md), [verification](../development/verification.md) and [delivery plan](../development/delivery-plan.md).
