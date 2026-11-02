# Backend architecture

The active `emr-api` Rust crate uses Actix for HTTP and SQLx for parameterized PostgreSQL queries and embedded migrations. Responses use stored records; normal startup seeds no clinical or billing fixtures.

| Module | Responsibility |
| --- | --- |
| `main.rs` | Local startup, loopback bind, database pool and migrations |
| `lib.rs` | Routes, shared state and structured errors |
| `access.rs` | Actor resolution, role policies, exceptions and audit inserts |
| `records.rs` | Field allowlists, validation, scope and persistence |
| `routes.rs` | Setup, bootstrap, reads and guarded writes |
| `workflow.rs` | Clinical/financial transitions, discharge and offboarding |
| `clinical.rs` | Plans, goals, reviews, groups and attendance |
| `forms.rs` | Snapshots, typed answers, provenance and form lifecycle |
| `exports.rs` | Aggregate accounting and internal claims CSV |
| `readiness.rs` | Shared billing checklist and transition validation |
| `applicability.rs` | Organization form rules and episode-specific completion |
| `speech.rs` | Optional bounded local recognizer, consent and reviewed transcript output |

Mutations and audit records share a transaction. Critical transitions lock relevant rows; database checks, foreign keys and immutability triggers reinforce application validation. Errors do not return SQL or clinical narrative.

Real PostgreSQL/HTTP tests exercise persistence, permissions, immutable history, financial concurrency and exports. See [verification](../development/verification.md) for evidence and [the data model](data-model.md) for limitations. Application policies do not replace the deliberately deferred identity system.
