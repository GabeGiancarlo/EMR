# Nexus integrated prototype delivery plan

## Confirmed November 13 target

On November 4, 2026, the owner set **Friday, November 13, 2026** as the target for a fully working prototype covering **all planned product features**, with external integrations lower priority. The [November 13 execution plan](prototype-target-2026-11-13.md) maps all 50 capabilities to workstreams, sequencing and acceptance evidence. This is broader than demonstrating only the existing staff workflows. The older Vegas note below records prior context and does not replace this confirmed target.

## Product intent

Build a recovery-treatment operating system connecting clinical care, billing readiness and executive oversight. Start with empty real storage, custom components and human-reviewed records. The owner confirmed U.S. adult outpatient, IOP and PHP as the first scope. State-specific rules and payer mix still require operating-center input. The Vegas conversation is an informal progress walkthrough, not a public launch deadline.

Authentication provider installation is explicitly deferred by the owner. Permission policies and audit events are application features, but selecting a local operator is not identity verification. Local-only startup must fail closed for public binding. No production patient data or public launch until identity and deployment controls are validated.

## Acceptance map

The November 2 review found that the shared interface and workflow tests did not satisfy the original three-workspace product plan. Treat this document as a delivery plan, not a completion ledger. The [scope audit](roadmap-gap-audit.md) reconciles the original roadmap, Figma evidence and remaining work; it takes precedence over earlier narrow milestone summaries.

Each item requires working user flows, persisted state, meaningful validation, accessible empty/error/loading states and tests for critical invariants. A view existing is not evidence its workflow is complete.

| Area | Prototype acceptance |
| --- | --- |
| Setup | Create organization, facility and initial local operator atomically; prevent repeated setup |
| Intake | Create client and care episode; capture consent attestations, coverage and assignment; no invented records |
| Care | Schedule sessions; write template-based notes; review and sign; preserve signed originals and amendment chain |
| Team | Explicit roles, visible permissions, expiring exceptions and staff offboarding with reassignment |
| Client offboarding | Discharge episodes with reason; preserve history and outstanding billing work |
| Billing | Policy and authorization coverage, documentation readiness, claim transitions, manual external references, payment ledger |
| Leadership | Metrics derived from actual records, drill-through action queues, restricted clinical narrative |
| Forms | User-authored field definitions; reviewed source-to-field drafting; no automatic clinical attestation |
| Operations | Reproducible local startup, migration tests, backup/restore verification, CI and explicit deployment boundary |
| Integrations | Clearly distinguish implemented adapters, import/export, and unconfigured external systems |
| Research | Cited competitor evidence, dated snapshots, market hypotheses, compliance obligations and validation questions |

## Milestone sequence

1. Persistence, migrations, core workflows, permission boundaries and integrated interface.
2. End-to-end testing of intake through documentation, claim readiness/payment and discharge; edge cases and accessibility.
3. Extend group attendance, treatment planning, structured forms, consent granularity, revenue workflows and operational gaps identified by research.
4. Stakeholder walkthrough with empty-state onboarding and user-created records; verify the exact agreed meeting scope.
5. Public deployment only after identity provider wiring, scope validation, vendor agreements and operational/security acceptance. This milestone is outside the current authentication-excluded scope.

## Speech and external billing

Transcript import, human-reviewed field mapping and an optional local whisper.cpp adapter are implemented. The pinned recognizer passed a synthetic WAV test through the actual UI; this is not clinical accuracy validation. Recording consent, role and clinician-assignment checks gate transcription, and a person reviews the words and selects each excerpt. Live capture, diarization and domain-specific accuracy evaluation remain open. The app remains useful without the recognizer. Never silently use browser cloud dictation or send clinical content to development-review models.

Claims preparation and reconciliation can be owned by Nexus. Actual payer transmission, eligibility and remittance require trading-partner enrollment, format/licensing decisions and adapter validation; a local status change must never imply successful external transmission. Keep accounting exports free of patient identifiers unless the receiving system and agreement explicitly support that use.

## Working conventions

Historical November 2 checkpoint below: its Windows path, feature gaps and $5,000 authorization describe that earlier session. Current November 4 direction, Mac runtime and the $1,500 research/readiness ceiling are in [central context](../PROJECT_CONTEXT.md) and [current orchestration](orchestration.md). Do not inherit the old budget or treat the old gap list as current source evidence.

Current local checkpoint includes intake, episodes, care plans, individual/group encounters, reviewed/signed documentation, stage/care-level form routing, local WAV transcription, billing readiness, manual external references, payments/reversals, exports and staff handoffs. Browser and database evidence is recorded in [verification](verification.md). Five scoped Grok implementation proposals were integrated and checked locally; a failed staff-role worker was completed by the root agent. This is an expanded working prototype, not completion of the entire EMR scope.

Next priorities are the dedicated clinician/billing/executive journeys, a full calendar and working telehealth/client journey, then records lifecycle/attachments and the remaining documentation/financial workflows. See the scope audit for acceptance gates. Broader care-team/facility permissions and billing reconciliation remain open. Live capture and clinical speech evaluation remain separate from the verified WAV adapter. External payer connectivity needs actual trading-partner inputs. Authentication stays explicitly deferred.

- Checkout: C:/Users/ganeg/source/EMR, branch nexus/integrated-prototype.
- Root agent owns integration, operations and verification; specialist agents own backend, frontend and research during their work.
- xAI reviewers and implementation workers receive scoped development source packets only. The shared ledger records known billed usage and conservative holds for pending/unknown charges against the owner's $5,000 total ceiling. Up to four implementation workers run in parallel; proposals require local review and verification. See [the worker workflow](xai-workers.md). No automatic retry loops.
- The owner supplied the Demo and EMR DUMP FILE Figma URLs on November 2. Selected Demo frames and the DUMP inventory have now been viewed; frame evidence and inspection limits are in the scope audit. Earlier permission to proceed without the missing files does not supersede these newly available references.
- Keep acceptance status honest; do not label a prototype complete merely because builds pass.
