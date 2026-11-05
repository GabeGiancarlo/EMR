# Nexus expanded engineering and research pass

November 4, 2026. Closed on `codex/prototype-readiness`, source baseline `453c280` plus preserved local changes. The owner requested a broader, higher-powered pass with more Grok workers and **$500 spent on useful Nexus work**. This is a separate spending target and ceiling. Earlier reported charges of $7.476206 and $225 of unknown-cost reservations remain separate in the shared ignored ledger.

## Scope

Functional workflows come before branding implementation. Preserve all 50 capability requirements, outpatient priority, the November 13 target and the bottom-left profile decision. This pass deepens architecture and feature evidence, obtains concrete implementation proposals, integrates reviewed fixes, and develops detailed contracts for missing workflows. It does not turn a worker report or a build into accepted functionality.

## Current checkpoint

- Integrated locally: canonical service identity/migration 021, exact claim-money defaults, bottom-left profile controls, calendar visibility corrections, owner-session terminal-job recovery and permission-aware Billing amounts.
- Verified: 103 Rust unit tests after the worker repair; the earlier 16 PostgreSQL scenarios plus five upgrade/Unicode/rollback tests, four HTTP identity scenarios, an added export-cap test and four worker-recovery tests; 89 frontend tests, builds, strict lint/formatting and selected browser journeys. Existing drafting/transcript PostgreSQL journeys passed again after the worker repair. See the verification log for per-change timing and limitations.
- Reviewed research: financial events/packets/metrics, clinical records, identity/proxy, scheduling, media/models, attachments/lifecycle and custom branding. Source corrections and rejected proposals are preserved in the contracts below.
- Final integration: export boundary regression now passes; nine local-model evaluation harness tests pass; the real eight-case local run completed and exposed semantic failures despite valid quotations. The bootstrap measurement proposal was truncated and not integrated. The portable setup stream ended without a final receipt; no setup script was applied. Owned terminal-job recovery is reviewed, tested and active locally. All 20 final review requests are closed. The [final decision and tour](final-handoff.md) distinguish verified fixes from proposed release gates.
- Provider reconciliation remains open; local calls are closed. Confirmed expanded-run charges: **$206.81231595** (126 requests); unresolved reservations: **$278** (18 outcomes); active: **0**. Confirmed spending did not reach $500.

Specialist councils and adversarial reviews now cover identity, clinical documentation, patient records, scheduling, telehealth/media, portal/messaging/proxy, attachments/lifecycle, executive reporting, operations, forms/tasks, local speech/drafting and branding. Reviewed cross-module decisions are in [expanded implementation contracts](expanded-contracts.md). The final contracts incorporate the subsequent financial/schema and UI reviews. Raw source manifests, response IDs, outputs and usage remain in ignored `.local/xai`; credentials are never included.

## Acceptance boundaries

Pending code proposals are not applied automatically. Migration numbering, shared routing and source integration have one coordinator. Financial tests use only disposable `nexus_test` schemas. Prior ordinary and walkthrough data remain intact. No publication or deployment is authorized by this review.

See the [provider receipts and final local accounting](expanded-run-evidence.md) for actual request IDs, known charges, unknown outcomes and invalid reports.

## Cost accounting

Run ID `deep-pass-2026-11-04` uses the same lock and ledger as previous work. Actual returned `cost_in_usd_ticks / 1e10` determines reported spend; unknown calls retain reservations. Reservations are internal accounting holds, not provider spending limits. Exact account billing and unrelated account activity are outside this ledger.

## Initial reviewed findings

The architecture council returned under response ID `bca6c41d-8ced-986a-b439-fb31fad0854a`; xAI reported $4.0231199, 3,947,557 input tokens (3,053,056 cached), 113,129 output tokens including reasoning across the provider fleet, and no web-search use. Its 36,248-character leader report is a source review, not fresh external research. Retain the modular-monolith direction, field minimization and financial invariant priority. Core review rejected unsupported claims that persisted queues, interruption recovery and knowledge invalidation were absent; those mechanisms exist. Generic checksum uniqueness across patients and automatic deletion of clinical derivatives were also rejected. Specific retries, supervision, fairness and lifecycle work need narrower contracts.

A coordinator-run PostgreSQL/API regression independently reproduced the duplicate-charge defect: `h2019` and `H2019` created two live draft claims for the same session, each for 1,500 cents. The required second-response 409 assertion failed with HTTP 201. This used only a unique schema in `nexus_test`. The first Grok code proposal was rejected as written because its restricted service-code alphabet changes existing behavior and its migration suggestions require correction. Migration 021 was first tested in disposable `nexus_test` schemas, then activated in both local workspaces with all application-table contents preserved. The new duplicate-case API regression, 94 Rust unit tests and all 16 existing PostgreSQL integration scenarios passed on this Mac. Five additional dedicated public-migration/Unicode tests now pass, including populated 019/020 upgrades, immutable-history preservation, collision rollback and lock release. Four later HTTP identity scenarios also pass, covering PATCH/void/Unicode authorization and changed-encounter readiness; see verification for boundaries.


## Integrated application changes

The cents conversion fix now uses one actual RecordForm initializer and one money-input boundary; preselection and interactive selection round-trip exact cents. The bottom-left profile holds theme, permission-filtered settings and a clear local workspace exit; mobile focus and navigation scrolling were checked. Calendar grouping now deduplicates within the current date window so a child remains visible and openable when its parent is outside the view. No all-patient roster aggregate was introduced. The frontend passed 89 tests and a fresh build after the final profile refinements, with targeted desktop/mobile browser checks. See [verification](../../development/verification.md).

## Deeper follow-up queue

After console recovery of seven interrupted cost receipts, a 24-task queue launched 16 concurrent actual Grok calls with per-model throttling and automatic slot refill. Its distinct work includes financial state machines, billing-packet autocomplete, denial workbenches, executive metric definitions, facility/care-team scope, recurring calendars and sync-adapter contracts, diagnoses/medication/lab boundaries, attachments, persisted-worker recovery, model-quality evaluation, portal/proxy/messaging, component research, pagination, portable setup and acceptance gates. Reports are reviewed before synthesis or application. Invalid/truncated reports remain charged outcomes, not completed research.


## Local activation

After the migration tests and independent review, both existing local APIs were restarted on current code. `nexus` and `nexus_walkthrough` now use migration 021; all 52 application-table row fingerprints matched before and after in each database. Both health endpoints passed and the walkthrough UI refreshed successfully. The ordinary and walkthrough records were preserved. This is local development activation, not public deployment or production acceptance.
