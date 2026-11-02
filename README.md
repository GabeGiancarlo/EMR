# Nexus

Nexus connects recovery-center care, billing readiness and operational oversight. The local prototype uses a custom React/TypeScript interface, a Rust/Actix API and PostgreSQL. It starts with an empty database: organization setup and every client record come from user input.

Authentication is deliberately deferred. Local operator selection exercises application permissions but does not verify identity. The API requires explicit development mode and refuses a non-loopback bind. Do not use real patient data or publish this mode.

## Local development on this Windows workspace

Checkout: `C:\Users\ganeg\source\EMR`.

Start the project-local PostgreSQL instance:

```powershell
cd C:\Users\ganeg\source\EMR
.\tools\start-database.ps1
```

Alternatively, with a working Docker engine: `docker compose -f compose.local.yml up -d`. Both use localhost port 55432; run only one database option at a time. The portable binaries live under ignored `.local/postgres/pgsql`, downloaded from the official EDB distribution. They are not committed. Database credentials are local-development defaults only.

In a terminal, start the API:

```powershell
.\tools\start-api.ps1
```

In another terminal, start the interface:

```powershell
cd C:\Users\ganeg\source\EMR\frontend
npm ci
npm run dev
```

Open http://127.0.0.1:5173. The API is at http://127.0.0.1:8090; `/healthz` checks database connectivity. Versioned migrations run on API startup. Setup creates the organization, first facility and local administrator atomically.

## Connected workflows

- Organization and facility setup; team roles and expiring permission exceptions.
- Client intake, care episodes, clinician assignment and discharge.
- Session scheduling, authored clinical notes, signing and append-only amendments.
- Shared group encounters, attendance and linked individual documentation.
- Treatment plans, measurable goals, activation, progress reviews and revision history.
- User-authored form templates, versioned responses, source excerpts, review/signing and consent attestations.
- Coverage and authorizations; session-based claim readiness and manual external-submission records.
- Payment recording with transactional overpayment checks.
- Append-only payment reversals, ready-claim return to draft and unsubmitted-claim void history.
- Shared tasks, staff offboarding and reassignment.
- Operational metrics from stored records and audit history.
- Aggregate accounting and internal claim-review CSV exports with bounded date windows.

These are prototype workflows, not a claim of production or regulatory readiness. Exact record fields and boundaries are in [the API contract](docs/architecture/api-contract.md) and [backend notes](api/README.md). The [delivery plan](docs/development/delivery-plan.md) tracks the broader requested scope. External payer transmission, eligibility/remittance adapters, document attachment storage and locally hosted speech recognition require further implementation and validation. Transcript-to-form mapping is explicit and human-reviewed; the application does not yet transcribe audio.

## Verification

```powershell
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --workspace --all-targets -- -D warnings
$env:TEST_DATABASE_URL = 'postgres://nexus:nexus_local@127.0.0.1:55432/nexus_test'
cargo test --locked --test postgres -- --ignored
cargo test --locked --test exports_postgres -- --ignored
cd frontend
npm test
npm run build
```

The database integration suite uses an explicitly disposable `nexus_test` database. It must never be pointed at user records. UI verification uses a separate test database as well; no fixtures are seeded into the ordinary workspace.

## Backup and restore

```powershell
.\tools\backup-database.ps1
.\tools\verify-restore.ps1 -Backup '<absolute path returned by backup>'
```

Backups and SHA-256 manifests stay under ignored `.local/backups`. Restore verification creates a new isolated database and leaves the source untouched. These local backups are not an encrypted production backup service.

## Research and architecture

- [Competitive and product research](docs/research/nexus-market-and-product-research.md)
- [Illustrated research report](docs/research/index.html) (open in a browser)
- [Primary source ledger](docs/research/source-ledger.md)
- [Competitor visual references](docs/research/snapshots/README.md)
- [Delivery and integration plan](docs/development/delivery-plan.md)
- [API contract](docs/architecture/api-contract.md)
- [Implemented data model and invariants](docs/architecture/data-model.md)
- [Accounting and claims export boundaries](docs/architecture/exports.md)
- [Verification evidence and limitations](docs/development/verification.md)

The interface can be built for Vercel later. The Rust API and PostgreSQL need separate suitable hosting, identity enforcement and operational review. No public deployment is configured by this local-development work. The legacy `infra/docker-compose.yml` remains historical reference; use the root `compose.local.yml` for the current database.

## Repository layout

`frontend/` contains custom UI components; `api/` contains the active application; `migrations/` defines durable schema; `tools/` provides local operational helpers; `docs/research/` holds evidence and product analysis. `core/`, `fhir/` and `jobs/` remain parked legacy material and are not active workspace members.

xAI is used only as a development reviewer through `tools/xai-review.mjs`. It is not a clinical application dependency. Source-review usage and outputs remain in ignored `.local/xai`; the API key is read from the environment and never committed.
