# Nexus API

Persistent local development API for the Nexus clinical, billing, and operations workspace. Actix Web serves JSON; SQLx applies versioned PostgreSQL migrations on startup. The database starts empty.

## Run

Start PostgreSQL using the repository development tooling. In PowerShell:

```powershell
$env:NEXUS_LOCAL_DEV = '1'
$env:DATABASE_URL = 'postgres://nexus:nexus_local@127.0.0.1:55432/nexus'
cargo run -p emr-api
```

The API defaults to 127.0.0.1:8090. Authentication is intentionally deferred. Startup refuses non-loopback binding and requires explicit local-development opt-in. `x-nexus-actor` selects an active staff record for policy testing and must never be treated as authentication. Public hosting requires a verified identity provider and a separate deployment review.

## Implemented

Workspace onboarding; persisted facilities, staff, clients, episodes, sessions, clinical notes, tasks, policies, authorizations, claims, payments, form templates, consent attestations, and permission exceptions; immutable signed notes and append-only amendments; clinical/billing role boundaries; clinician assignment scope; claim readiness checks and authorization-unit reservations; external-submission recording; concurrent overpayment protection; discharge; staff offboarding and assignment transfer; transactional audit history. Treatment plans support measurable goals, attestation, immutable definitions, reviewed progress snapshots, completion, and revision. Shared groups support capacity-controlled enrollment and attendance-linked individual sessions. Structured forms retain template snapshots, typed draft answers, source-excerpt provenance, review/sign/amend history, and deterministic draft-note creation from actual answers.

Routes and frontend field definitions are in [the API contract](../docs/architecture/api-contract.md). `lib.rs` configures routes, `routes.rs` adapts HTTP, `records.rs` validates and persists records, `access.rs` evaluates policies, and `workflow.rs` coordinates transactional state changes.

PATCH requires the loaded record's `_expected_updated_at`; stale writes return 409 without changing the record or its update audit. `readiness.rs` supplies both the claim checklist and transition validation. `applicability.rs` matches organization form rules against client stage and the relevant episode. `speech.rs` runs an optional local whisper.cpp process with bounded WAV input, consent/assignment checks, bounded output, timeout and temporary-file cleanup. Speech access is rechecked under locks in the same transaction as the completion audit.

## Validation

```powershell
cargo test --workspace
$env:TEST_DATABASE_URL = 'postgres://nexus:nexus_local@127.0.0.1:55432/nexus_test'
cargo test --test postgres -- --ignored
```

The explicitly ignored integration test resets only a database whose URL ends in `/nexus_test`. It verifies persistence and workflow boundaries against real PostgreSQL. Never point it at clinical data. All fixtures are confined to that disposable test database.

## Boundaries

No authentication, external claim transmission, eligibility lookup, payer adjudication, file storage, legal consent generation, or clinical decision support is represented as implemented. Group encounters have real participant rosters; each attendance record owns an individual session used by notes and billing. Form answers are entered or explicitly mapped from reviewed source text, including optional local speech recognition. Recognition does not interpret clinical meaning or attest to accuracy. Claims cover one session/service line each, not complete X12 claim transactions. Payment adjustments reduce a balance but are separated from cash in metrics. Local attestations and role selections do not establish verified identity or legal electronic signatures. The local roles are a prototype policy matrix, not a claim of regulatory compliance.

Signed records, coverage attached to advanced claims, and completed documented sessions are preserved. Payment mistakes use append-only full reversals with reason/date and balanced cash/adjustment reporting. Ready claims may return to draft, and unsubmitted claims may be voided and replaced without erasing history. Submitted claim corrections, payer resubmissions, and changed coverage for delivered care need separate workflows and are not disguised as destructive edits.
