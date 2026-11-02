# Nexus integrated prototype delivery plan

## Product intent

Build a recovery-treatment operating system connecting clinical care, billing readiness and executive oversight. Start with empty real storage, custom components and human-reviewed records. The owner confirmed U.S. adult outpatient, IOP and PHP as the first scope. State-specific rules and payer mix still require operating-center input. The Vegas conversation is an informal progress walkthrough, not a public launch deadline.

Authentication provider installation is explicitly deferred by the owner. Permission policies and audit events are application features, but selecting a local operator is not identity verification. Local-only startup must fail closed for public binding. No production patient data or public launch until identity and deployment controls are validated.

## Acceptance map

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

Speech recognition cannot responsibly be invented or clinically validated within a short prototype sprint. First deliver transcript import and human-reviewed mapping to form fields, with source provenance. Evaluate an optional locally hosted open-weight recognizer through an adapter, consent controls and accuracy testing. The app must remain useful without it. Never silently use browser cloud dictation or send clinical content to development-review models.

Claims preparation and reconciliation can be owned by Nexus. Actual payer transmission, eligibility and remittance require trading-partner enrollment, format/licensing decisions and adapter validation; a local status change must never imply successful external transmission. Keep accounting exports free of patient identifiers unless the receiving system and agreement explicitly support that use.

## Working conventions

- Checkout: C:/Users/ganeg/source/EMR, branch nexus/integrated-prototype.
- Root agent owns integration, operations and verification; specialist agents own backend, frontend and research during their work.
- xAI reviewers receive scoped development source packets only. tools/xai-review.mjs records billed usage in ignored .local/xai/usage.json and caps attempts at ten with conservative per-call reservations. No automatic retry loops.
- Original Figma references were absent from repository/history as of November 2, 2026; the owner confirmed proceeding with a fresh custom interface without them.
- Keep acceptance status honest; do not label a prototype complete merely because builds pass.
