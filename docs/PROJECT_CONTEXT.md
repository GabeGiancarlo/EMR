# Nexus — central project context

Updated November 4, 2026. Read this before starting a new repo task; then inspect current source/status. This is a maintained map, not automatic memory of every chat.

## Product and current direction

Nexus connects clinical work, billing and executive oversight for behavioral-health/recovery organizations. The owner brings clinical expertise through a therapist with more than 40 years of experience and can obtain executive feedback through a partner. Billing and executive workflows need dedicated discovery; they must not be clinical pages with different labels.

The target is **Friday November 13, 2026** for all planned internal prototype features. Live external integrations are lower priority; two-way calendar synchronization remains an explicit future requirement. Outpatient is the first customer priority. The owner wants additional care settings supported, but the second setting, state, payer mix and first customer's billing contracts are open. Existing implemented scope is adult outpatient/IOP/PHP; do not silently claim residential or pediatric readiness.

The latest sequence is **functionality first, branding implementation afterward**. Current visuals are rejected. Future brand work should use restrained growth/nature motifs, less saturated earthy greens, supportive neutrals and distinct light/dark palettes, firmer corners, custom components and subtle accessible motion. Profile/settings/theme/sign-out belong at the **bottom left under navigation**. See the [owner review](product/walkthrough-review-2026-11-04.md) and [research/planning review](research/research-planning-review-2026-11-04.md).

The expanded pass authorized later on November 4 has a separate **$500 spending target and ceiling**. It uses higher-effort Grok 4.7 workers and Grok multi-agent research, with OpenAI core review. The expanded run is closed. The [final handoff](research/2026-11-04/final-handoff.md) records the release decision and current walkthrough, and [provider evidence](research/2026-11-04/expanded-run-evidence.md) separates $206.81231595 in confirmed charges from $278 in unresolved reservations. No paid workers remain active.

## What is implemented

| Area | Current fact | Boundary |
|---|---|---|
| Application | React/TypeScript frontend; Rust Actix API with SQLx | Only `api` is an active Cargo workspace member; `core`, `fhir`, `jobs` are parked |
| UI | Custom React/CSS/inline SVG; React and React DOM are runtime frontend dependencies | No shadcn/ui, Radix or Tailwind component system; custom does not mean good usability or proven accessibility |
| Persistence | PostgreSQL 17; 23 migrations (022 one active staff member per email, 023 a visit-event index; both tested in disposable schemas, not yet applied to the local workspaces); migration 021 tested in disposable schemas and applied to both local workspaces; 52 application tables | Not SQLite or browser-only storage. SQLx metadata brings the recorded restore count to 53 |
| Workflows | Registration/intake/admission, versioned programs/services, scheduling, groups, plans/forms/notes, readiness, payment/reversal and CSV paths | Partial implementation across the broader product; see the 50-capability ledger |
| Speech | Local Whisper base.en through whisper.cpp | OpenAI-created open-source model, not Nexus-trained; no hosted speech API required |
| Drafting | Local Qwen3-4B through llama.cpp; approved sources and explicit review | Open-weight third-party model, not Nexus-trained; not autonomous clinical signing or validated clinical accuracy |
| Video | Native WebRTC peers with local signaling, invitations, waiting/admission, moderation and screen sharing | Prior local three-participant evidence; remote-network/TURN, production identity, scale and current Mac device acceptance remain open |
| Recording | Consented own-device tracks, AES-GCM ciphertext in PostgreSQL, playback and transcript review | Not automatic complete capture of every call participant; production key recovery/rotation/storage/lifecycle remain open |
| Billing | Readiness checks and local transitions; one service per current claim, recorded external reference, payments/reversals and exports | Not live payer submission/adjudication, ERA ingestion or bank reconciliation |
| Identity/access | Local staff selection (default) or, in hosted mode, OpenID Connect sign-in mapped to a staff email; organization scope, resource policies, primary-assigned clinician scope and exceptions | Local actor headers are not authentication. Hosted sign-in is built and tested locally against a mock provider but not deployed. Facility/care-team/patient/proxy authorization remains incomplete |

Open-source/self-hosted means control over hosting and a replaceable adapter; it does not remove upstream software/model licensing, maintenance or evaluation obligations. Development Grok calls are separate from application runtime AI and receive no patient data.

## Current checkout and Mac runtime

Repository: `GabeGiancarlo/EMR`; origin `https://github.com/GabeGiancarlo/EMR.git`. Source checkpoint `453c280` on `nexus/integrated-prototype`; local work on `codex/prototype-readiness`. The base commit connects versioned programs to admission, scheduling and billing (54 files, 7,181 insertions, 218 deletions). The November 5 readiness checkpoint, including reviewed source, migrations, tests, research and the guided walkthrough, lives on the [GitHub readiness branch](https://github.com/GabeGiancarlo/EMR/tree/codex/prototype-readiness). Run `git status -sb` to see any subsequent local changes.

Mac checkout: `/Users/gabrielgiancarlo/Documents/ChatGPT/Nexus`.

| Workspace | Frontend | API | Database |
|---|---|---|---|
| Ordinary empty setup | `http://127.0.0.1:5173` | `http://127.0.0.1:8090` | `nexus` |
| Synthetic owner walkthrough | `http://127.0.0.1:5174` | `http://127.0.0.1:8091` | `nexus_walkthrough` |
| Disposable integration tests | — | Test harness | `nexus_test` |

PostgreSQL listens on loopback port 55432; this Mac's cluster is in ignored `.local/postgres`. Local runtime helpers/logs/process registry, model files and media keys are under `.local/`. The machine-specific launcher is `.local/start-dev.py`; inspect it before using it, and do not assume it exists in a fresh clone. The tracked [local setup](development/local-setup.md) and root README describe the earlier Windows setup; a portable Mac bootstrap remains to be formalized. Do not reset the owner walkthrough or ordinary database for tests.

Optional local model service: loopback port 8093, alias `nexus-qwen3-4b`; models are not committed. Media keys are separate from database backups. Recovery needs both the restored ciphertext and the correct separately protected key.

## Hosted demonstration (prepared, not deployed)

As of November 11, 2026 the owner's direction is a first hosted version that is a **synthetic-data demonstration behind real sign-in**, not a system for real patient data: frontend on the owner's personal Vercel, API and PostgreSQL on a container host, sign-in through a hosted identity provider. `NEXUS_AUTH_MODE=oidc` enables it; `local` remains the default and keeps the loopback guard. As of November 12 the identity provider is set up (Auth0 tenant `dev-vulcan-01`, application "Nexus Demo", a staff-only connection with sign-up disabled) and a Vercel project exists at `https://nexus-demo-orcin.vercel.app`, but the API and database are not deployed, so the site does not build yet and nothing is live. The container host is the owner's choice (recommended: Render, by the `render.yaml` blueprint). The hosted API also has `/livez` (liveness) beside `/healthz` (readiness, with worker states), an `X-Request-Id` on every response, per-address request limits (429 `rate_limited`) and bounded database waits (503 `database_busy`, `database_timeout`, `database_unavailable`); see `api/README.md`. Steps, variables and open questions are in [demo hosting](deployment/demo-hosting.md); `.env.example` lists every variable.

## Evidence and remaining readiness work

Baseline Mac evidence at `453c280`: 83 frontend tests, 93 Rust unit tests, 16 PostgreSQL integration scenarios, frontend build and selected browser journeys. Two reproduced scheduling/form regressions were fixed locally: blank optional catalog references now serialize as null; equivalent RFC3339 timestamps compare as exact instants. The updated 84-test frontend suite, program-service unit/integration regression and frontend build passed. Consult [verification](development/verification.md) for scope; a full suite passing before a fix is not a full rerun after it.

Actual Mac API calls transcribed consented synthetic speech and generated a reviewed-source clinical draft awaiting human review. A later eight-case local evaluation found that the model could turn embedded source instructions into an unsupported diagnosis and put unrelated text under medications while passing exact-quote validation; drafting-quality acceptance remains blocked by semantic reliability. This proves the connections, not accuracy across real clinical audio or complete saved-media acceptance. Earlier November 2 Windows evidence includes three-participant video and restored encrypted tracks with the same installation key; it does not establish recovery after losing that key or current Mac remote-call readiness.

The capability ledger baseline is 38 Partial, 11 Missing and 1 Designed, with none accepted as complete. Do not convert those counts to a completion percentage. Important work includes patient/proxy access, recurring scheduling, attachments, medication/MAR/labs, messaging, financial event history/reconciliation, reporting/outcome inputs, record lifecycle and professional deployment controls. The November 13 target is high risk until these are broken into testable contracts and integrated scenarios.

PostgreSQL is an appropriate relational foundation. The present limitations are implementation/operations: verified identity, finer access scope, real deployment secrets/TLS/backups and key recovery, paginated/scoped queries, concurrency measurements, lifecycle and monitored jobs. Bootstrap currently loads authorized collections and computes aggregates in memory; generic writes serialize by organization. These are review targets, not measured capacity failures. Avoid a premature microservice rewrite.

The November 4 Grok audits and core review identified a source-backed duplicate-charge risk: catalog matching normalizes service-code case while the live-claim unique index uses raw codes. The coordinator reproduced the duplicate in `nexus_test`, then implemented a shared database identity rule and migration 021. The regression, existing integration suite, five upgrade/Unicode/rollback tests and four added HTTP identity scenarios pass. The HTTP cases cover PATCH collision history preservation, void/replacement, Unicode authorization and stale encounter readiness; they do not add cross-tenant/role coverage. Both local workspaces now run migration 021; before/after fingerprints match for all 52 application tables in each database. Claim/snapshot/catalog/authorization matching now share the local database identity function. A separate double-conversion defect in a currently unmounted preselected-session claim initializer is fixed locally, with both form paths tested. See the [engineering review](research/2026-11-04/engineering-review.md) for evidence and rejected worker findings. Executive bootstrap also exposes whole authorized rows, including some diagnosis/contact fields; field-level minimization is proposed work.

The two persisted workers now retry unsaved terminal failure/audit writes on their dedicated advisory-lock session; four PostgreSQL tests cover rollback, cancellation between attempts and owner-connection loss. Pooled claim/success-publication fencing and automatic takeover remain open. Both local APIs run the repair with all 52 application-table fingerprints preserved.

The bottom-left profile menu is implemented locally with existing permission/dirty-state guards, theme and local operator exit; mobile settings navigation restores focus to the visible heading. Calendar parents only suppress child appointments within the displayed window. The frontend suite now has 89 passing tests and its build passed; see the dated verification log for later focused checks. These fixes do not accept the full branding system or production identity.

A settled claim regression verifies that limited read grants do not create phantom balances: the API returns null for financial measures whose required sources are unreadable, and Billing displays “Unavailable.” Authorized zeros remain numeric. See the [final handoff](research/2026-11-04/final-handoff.md) and [guided tour](product/local-walkthrough.md).

Detailed reviewed research now covers billing lifecycle/competitors, outpatient financial records, executive metrics, branding/components and the paid logo test. The Keychain key was verified against the owner's xAI team; actual response IDs and provider costs are in [run evidence](research/2026-11-04/xai-run-evidence.md). Initial failures and invalid acknowledgments are not counted as successful research. Older roadmap/code audits and the November 2 delivery-plan budget are historical; current source, this context and current user instructions take precedence.

## Navigation and maintenance

- [Current research/planning/review](research/research-planning-review-2026-11-04.md): conclusions, evidence, billing/executive/brand research and next decisions.
- [Acceptance ledger](product/acceptance-ledger.md) and [November 13 plan](development/prototype-target-2026-11-13.md): scope and acceptance gates.
- [Architecture](architecture/overview.md), [data model](architecture/data-model.md), [access policy](architecture/auth-rbac-audit.md): implemented contracts; verify against code/migrations.
- [Demo hosting](deployment/demo-hosting.md): how the synthetic demonstration is hosted, and what only the owner can do.
- [Production readiness](deployment/production-readiness.md): what is in place, what is left before the demonstration is live, and what is required before real patient data.
- [Orchestration](development/orchestration.md), [Grok runners](development/xai-workers.md), [Git workflow](development/git-workflow.md): how recurring work is coordinated.

At a handoff, record branch/revision, dirty files and their purpose, decisions changed, checks actually run, unresolved risks and the next concrete task. Update this page when durable facts change; keep detailed historical test results in verification/checkpoint documents. A new Codex task rooted in this repository can discover `AGENTS.md`; unrelated chats do not automatically gain this filesystem context.
