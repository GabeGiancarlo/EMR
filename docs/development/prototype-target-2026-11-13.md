# November 13 prototype delivery target

## Owner-confirmed target

Confirmed on November 4, 2026: **Friday, November 13, 2026**, America/New_York. The owner wants **all planned product features**, with external integrations lower priority for this checkpoint. This is broader than a staff-only demonstration of existing workflows.

The [50-capability acceptance ledger](../product/acceptance-ledger.md) remains the scope of record. Its current baseline is **38 Partial, 11 Missing, and 1 Designed**. This plan does not upgrade those statuses or establish that every capability will be finished by the target date. Progress must be backed by implementation and acceptance evidence.

The [November 4 owner walkthrough review](../product/walkthrough-review-2026-11-04.md) establishes that the current visual design and role workflows are not accepted. The owner's subsequent direction is **functional completeness first, branding implementation afterward**, with billing/executive and branding research now authorized. Outpatient is the first customer priority; additional care settings must remain possible, but the second setting, state and payer mix are not yet specified. The dates below are delivery targets, not a validated estimate or promise that all gaps fit the remaining time.

The current population remains U.S. adult outpatient/IOP/PHP. The ledger's broader housing and clinical capabilities remain in scope; they must not disappear merely because the current implementation lacks them. Authentication-provider installation remains deferred under the existing project decision; local operator selection is not verified identity.

## What counts as a working prototype

Each planned internal workflow must have persisted records, useful validation and error states, role-appropriate access, connected upstream/downstream actions, correction/history behavior, and an observed browser acceptance path. A screen, a mocked success response, or passing unit tests alone does not satisfy the workflow.

External-dependent portions must be separately identified. Local orders, eligibility records, claim files, submission histories, payment ledgers, messages, and patient views can be implemented and demonstrated locally. They must not claim a prescription was transmitted, a payer verified coverage, a payment was processed, a message was delivered externally, or an external identity was verified without genuine integration evidence. Deprioritizing integrations does not remove the corresponding internal product workflows.

## Parallel workstreams covering all 50 capabilities

| Workstream | Ledger IDs | Internal completion evidence | Dependencies and external boundary |
|---|---|---|---|
| Foundation, administration and operations | 1–4, 46, 48, 50 | Full organization/facility/program configuration; complete role journeys; permission/exception coverage; navigation and preferences; staff credentials/expiry and handoffs; searchable audit and policy versions; reproducible startup and demonstrated backup/restore. | Shared contracts, authorization and migrations support every workstream. Identity providers, hosted operations and external interoperability adapters are lower priority, with their disconnected state visible. |
| Intake and longitudinal care | 5–9, 20, 25, 29 | Intake queues and duplicate/identity review; complete facesheet and timeline; coded/versioned diagnoses; effective transfers and receiving-team handoffs; discharge/aftercare and follow-up ownership; housing/bed assignments if used; plan reviews/reminders/acknowledgements; observations, vitals, nursing and referrals. | Needs forms, attachment access, tasks and shared care-team scope. Entered data must identify its source and must not masquerade as imported provider evidence. |
| Scheduling, visits and media | 10–15 | Recurrence and series exceptions; rooms/capacity/availability; group roster/attendance corrections; full telehealth controls; consent/refusal/withdrawal; recording/playback/recovery; local speech, timed/speaker review and approved excerpts; visible failure/retry behavior. | Program services and patient/episode scope must survive the whole chain. Remote-network/TURN support needs separate evidence; local browser success is not proof of remote connectivity. Accuracy, diarization and device behavior need explicit evaluation. |
| Documentation, knowledge and attachments | 16–19, 21–24 | Useful authored notes and phrases; review/co-sign/supervisor queues; immutable signatures/amendments; conditional and multilingual templates; applicability across care stages; source/version-linked responses; patient-scoped retrieval/answers; reviewed local AI drafts; validated attachment upload/quarantine/download/OCR and correction propagation. | Depends on clinical sources, consent, patient scope and lifecycle. No automatic clinical signing/diagnosis. Scanning or semantic retrieval is not accepted merely because an adapter field exists. |
| Medication and laboratory workflows | 26–28 | Medication/allergy reconciliation, orders, pharmacy/refill work; MAR scheduled/administered/missed/held events and corrections; laboratory order/specimen/result units/ranges/versioning, acknowledgement and controlled release. | Uses coded clinical records, tasks, longitudinal history and portal disclosure. Actual e-prescribing/pharmacy/lab exchange is lower priority; manual/local records must be labeled accurately. |
| Collaboration, patient and caregiver experience | 30–33, 47 | Owned/deduplicated alerts and escalations; permission-scoped threads, attachments and read state; patient appointments/change requests, released records, forms/consents, messages, uploads, balances/resources; explicit caregiver purpose/category/date scope and revocation. | Requires record-release rules, consent, notifications, attachment access and dedicated patient/proxy scope tests. Demonstrate local patient/proxy actors without implying authenticated production access or externally delivered notifications. |
| Revenue operations | 34–41 | Ordered coverage and dated eligibility evidence; authorization renewal/expiry/utilization; multiline/versioned charges and claims; actionable readiness; batch/file/attempt history; denials, appeals and corrected versions; payments/reversals; statements, remittance allocation, unmatched queues and reconciliation. | Depends on delivered care, signed documentation and immutable service context. Live eligibility, clearinghouse acknowledgements, payer adjudication, payment/refund processing and bank operations remain external dependencies. Synthetic responses cannot establish connectivity. |
| Analytics, reporting and record lifecycle | 42–45, 49 | Defined billing and executive indicators with matching drilldowns; historical census/workforce/capacity; outcome/expense/risk inputs where supported; scoped report builder and matching CSV/PDF/XLSX output; saved/scheduled report controls; archive/entered-in-error, retention/holds and derivative-aware destruction. | Metrics depend on actual source modules. Never infer profit from receipts alone or invent compliance/outcome scores. Scheduled external distribution requires recipient authorization and delivery integration. |

## Working sequence

These are delivery targets, not completed milestones. Keep the ledger honest when a gate fails or a dependency remains unavailable.

| Dates | Target |
|---|---|
| November 4–5 | Repair confirmed regressions; reproduce Mac runtime including speech/drafting; break each workstream into accepted record contracts, permissions, migrations and browser journeys. |
| November 5–8 | Parallel implementation of absent clinical modules, attachments, collaboration/patient scope, and remaining revenue workflows. Integrate small vertical slices rather than disconnected screens. |
| November 8–10 | Connect transfers/discharge, recurrence, document supervision, patient/caregiver release, reporting and lifecycle to those modules; exercise corrections and cross-role handoffs. |
| November 11 | Run the complete acceptance matrix on one integrated revision; resolve data-integrity, authorization and workflow blockers before polish. |
| November 12 | Stabilization: fresh setup, restart persistence, migration/restore, error recovery, accessibility and responsive walkthrough; review every remaining ledger gap. |
| November 13 | Run the owner walkthrough against the actual integrated revision, report pass/fail evidence for every internal capability, and list lower-priority external dependencies separately. |

The OpenAI/Codex coordinator owns contracts, integration and verification; subordinate research/implementation workers use Grok through explicit bounded calls. OpenAI core reviewers may independently review critical work. Limit concurrent Grok requests deliberately; shared-file writes, API route wiring and migration numbering remain serialized. Do not let parallel work overwrite the same files or create incompatible domain concepts. See the [orchestration guide](orchestration.md).

## Acceptance matrix and evidence rules

For each capability, record: ledger ID, implementation revision, synthetic scenario, local role, browser steps, expected state, persisted result, denial/error path, automated invariant checks, and remaining external dependency. Mark an internal capability accepted only after all applicable checks pass on the integrated revision.

Cross-workstream acceptance must include:

1. Fresh organization setup through intake, required forms and admission, with duplicate/capacity/permission failures exercised.
2. Individual and group scheduling through attendance, including recurrence exceptions, time away and retired/historical service behavior.
3. Telehealth consent, capture, encrypted playback, recognition, corrected source review, drafted documentation, supervisor review/signature/amendment and scoped retrieval.
4. Diagnoses, observations, medication reconciliation/MAR and laboratory review connected to the chart, tasks and authorized patient release.
5. Patient/caregiver appointments, forms, uploads, messages and released records with category/date restrictions and revocation tests.
6. Documentation blockers through claim readiness, correction/appeal, payment/reversal, statement/remittance allocation and matching dashboards/exports.
7. Transfers, discharge/aftercare, staff offboarding, archive/holds and derivative-aware lifecycle without orphaning responsibilities or losing histories.
8. Restart and isolated restore, including media/key recovery; ordinary workspace remains free of synthetic fixtures.

## November 4 baseline and first work

- Local repository connected to `GabeGiancarlo/EMR`; source checkpoint `453c280` on `nexus/integrated-prototype`.
- Readiness work is on `codex/prototype-readiness`; no remote publication is implied by this plan.
- Ordinary workspace: `http://127.0.0.1:5173`, database `nexus`, empty first-run setup.
- Separate synthetic walkthrough: `http://127.0.0.1:5174`, database `nexus_walkthrough`.
- Baseline checks passed: 83 frontend tests, 93 Rust unit tests, 16 PostgreSQL integration tests, frontend production build, API health and selected browser journeys. These are baseline evidence, not acceptance of all 50 capabilities.
- Fixed scheduling regressions: nullable catalog selections now serialize as null; equivalent appointment timestamps compare as exact instants. Both failures were reproduced before the fixes. Updated frontend suite (84 tests), program-service unit checks, the full-form PostgreSQL scenario and frontend build pass.
- Pinned local speech/drafting models are installed and actual application API checks pass on this Mac: consented synthetic WAV transcription and one evidence-linked proposal awaiting human review. This does not establish clinical accuracy or Mac end-to-end saved-media acceptance.
- The xAI credential is retrievable from the named macOS Keychain entry without displaying or saving it. The owner subsequently authorized Grok research workers and raised the ceiling to $1,500, explicitly encouraging useful depth without treating that amount as a spending target. Current invocation and cost evidence belongs in the [research review](../research/research-planning-review-2026-11-04.md), not this earlier baseline.

Record later implementation/checkpoint evidence in [verification.md](verification.md) and update the [acceptance ledger](../product/acceptance-ledger.md) rather than treating this schedule as completion evidence.
