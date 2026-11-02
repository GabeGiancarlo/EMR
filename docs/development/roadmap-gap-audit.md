# Nexus scope and delivery audit — November 2, 2026

**Stage: early functional engineering prototype (pre-alpha). The requested initial product is incomplete.** There is a real API and database, but several core journeys and the intended role-specific experience have not been delivered. No defensible completion percentage follows from counting pages, tables, tests, or agent spend.

Baseline examined: local commit `981afd0`, branch `nexus/integrated-prototype`. Corrections made after this baseline are listed separately below. This audit combines a Grok source review with root review of the repository, database connectivity/counts, running interface, and selected Figma frames. The raw worker proposal is retained locally; unsupported worker conclusions were not accepted as facts.

## What went wrong

The implementation concentrated on persisted records and individual workflow transitions, then placed them in one shared staff interface. The acceptance work exercised selected happy paths and backend invariants without reconciling the result against all the original role journeys. Presenting that result as the initial build overstated progress.

This contradicts an explicit existing decision: [ADR-004](../decisions/ADR-004-product-architecture.md) requires Clinician, Billing, and Executive / Leadership workspaces and warns against returning to a generic patient-record demo. [Product vision](../product/vision.md) and [roles and workflows](../product/roles-and-workflows.md) already described the integrated operational product. Missing Figma access earlier did not excuse missing these written requirements. The xAI budget did not force the narrower implementation.

The complete historical README roadmap from remote commit `a91ad81b9668df403857ce779bbdbf81fe61984a` is preserved in [original-roadmap.md](../product/original-roadmap.md). Its old status statements are historical, not current status. The accepted scope remains U.S. adult outpatient/IOP/PHP, custom components, no ordinary demo seeds, clinical/billing/executive integration, and authentication-provider setup deferred. Telehealth and the client experience are now explicit acceptance requirements; they are not dismissed because the old README used broader wording.

## Direct answers about the implementation

| Concern | Evidence and present boundary |
| --- | --- |
| Is there a database? | Yes. PostgreSQL, SQLx migrations and a Rust/Actix API. Nine migrations create 24 public tables. The ordinary `nexus` database and the separate `nexus_ui_test` workspace both contain persisted organization state. Counts were inspected without copying record bodies. [Migrations](../../migrations), [API routes](../../api/src/routes.rs), [data model](../architecture/data-model.md). |
| Do records survive refresh/restart? | They are database rows, not in-memory demo objects. Previous browser workflows and a backup restored into an isolated database are recorded in [verification](verification.md). This is persistence evidence. |
| Is retention/deletion implemented? | No complete records lifecycle. Discharge retains records; signed documents and corrections preserve originals. Archive, entered-in-error handling, controlled deletion, legal holds, retention schedules and destruction evidence remain missing. Persistence must not be described as a retention program. No user records were removed by this correction. |
| Are these custom React components? | Yes. Runtime frontend dependencies are React and React DOM only. Controls, icons, dialogs and styles are repository code. No shadcn, Radix, Tailwind or UIkit was found in the current package/HTML. [Package](../../frontend/package.json), [UI components](../../frontend/src/ui.tsx), [styles](../../frontend/src/styles.css). Custom implementation did not establish good UX. |
| Are separate workspaces present? | No. Six staff roles and server permissions exist, including executive and billing roles, but the application shares one navigation and overview. Role-gated buttons are not the clinician, billing and executive journeys specified in ADR-004. |
| Is there a client portal? | No client-facing appointment, forms, messaging or session-join experience. The client chart is a staff view. |
| Is there a calendar? | Scheduling persists individual sessions and groups. The interface has a weekly date strip/agenda and a group tab. It lacks the intended day/week/month calendar, unified individual/group view, robust conflict/availability handling and scheduling UX. A list of sessions is not acceptance of that calendar. |
| Can clients join telemedicine? | No video room, join path, waiting room, device check, participant controls or reconnect handling. No tested clinician/client call exists. |
| Do groups work? | Group enrollment, capacity, attendance, cancellation/completion and linked individual sessions exist and are covered by backend workflow checks. Those checks do not establish a complete scheduling or group-care product. [Group UI](../../frontend/src/GroupEncounters.tsx), [clinical operations](../../api/src/clinical.rs). |
| Does speech work? | Optional local WAV transcription through whisper.cpp works and has a synthetic audio/browser check. Consent and access checks precede transcription; source excerpts can be mapped into reviewed forms. Live microphone capture, session recording, speaker separation and clinical speech evaluation are unfinished. This is speech-to-text; text-to-speech is not part of the implemented workflow. [Speech service](../../api/src/speech.rs). |
| Is billing complete? | Coverage, authorizations, one-service-line claims, internal readiness, manual external references, denial recording, payments/reversals and bounded CSV exports exist. Appeals, a proper billing work queue, electronic payer exchange, eligibility/remittance, reconciliation and a full ledger remain missing. A manually recorded reference is not an electronic submission. |
| Why no sign-in/out? | Identity-provider authentication was deliberately excluded by the owner. The local operator picker is not sign-in. The previous icon-only switch was unclear and the duplicate avatar added confusion; clear workspace exit is corrected below. |
| Was it tested? | Yes, but narrowly relative to the requested product. Unit/API tests and selected manual browser paths exist. Comprehensive browser acceptance for each role, accessibility/usability review, telehealth, production operations and the complete reference design are not established. |

## Figma evidence now available

The ordinary web reader could not retrieve the files, but their public design canvases opened in the in-app browser. No Figma login or account creation was performed. Visual inspection was partial: the named Demo frames below and the DUMP overview/layer inventory, not every frame or interaction.

| Reference inspected | Concrete design intent | Implementation gap |
| --- | --- | --- |
| [Demo — Desktop - owner - v2](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=0-1) | Revenue, compliance, outcomes, client counts, efficiency/profit; billing and therapist entry points; client progress, staff, calendar and an actionable queue. | Shared overview counts do not deliver this leadership workspace. Definitions and drill-throughs must use real records; illustrated values are design examples, not seed data. |
| [Demo — Desktop - Clinican - v2](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=0-1) | Prominent Start Telehealth; Start Session/New Note/Upload Form; clinical chart modules; task queue; weekly schedule. | Telehealth and attachments missing; clinical chart and daily work experience substantially narrower. Medication/lab features shown require separate implementation and validation. |
| [Demo — Desktop - Clinican - v3](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=58-1743) | Large calendar with Day/Week/Month, date navigation and events; adjacent urgent/today task queue; Notes/Messages/Client Progress/Alerts and Log out navigation. | Current agenda/date strip, shared tasks and operator icon fall short. |
| [EMR DUMP FILE](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5994) | Broad design inventory including clinician versions, Telehealth, Calendar, Notes, Reports, recent messages, financial metrics, and a Parent Progress Tracking group. | Needs a frame-by-frame inventory before claiming design coverage. Parent/pediatric references are historical design material; they do not silently override the agreed adult-first scope. |

Read-only reference captures are under ignored `.local/design-review/`: `figma-owner-v2.png`, `figma-clinician-v2.png`, `figma-clinician-v3.png`, `figma-dump-overview.png`, and `figma-dump-layers.txt`. These are evidence of what was viewed, not a complete design export. The old designs guide information architecture and workflows; they do not require copying their sample people or metrics.

## Original roadmap traceability

Every original phase remains in scope unless explicitly deferred by the owner. This table groups the original bullets; [the preserved roadmap](../product/original-roadmap.md) retains their exact wording and original TODOs.

| Original phase | Present evidence | Still open |
| --- | --- | --- |
| 0 — Repo reset and architecture alignment | Active Rust API, custom React frontend; product/ADR documentation. | Ensure delivered UX follows those decisions; retire stale status statements. `fhir/` and `jobs/` remain outside active Cargo workspace membership. |
| 1 — Backend foundation | PostgreSQL/SQLx, organizations/facilities/staff/clients, structured validation/errors, modular workflow code, audit events, real database tests. | Complete domain/module boundaries, expanded workflow schema and invariants; broad form validation and usability. Existing foundations do not imply all entities are modeled. |
| 2 — Security, access and audit | Role policies, organization scoping, clinician primary-assignment rules, expiring exceptions, sensitive-operation audits; loopback-only local mode. | Identity remains intentionally deferred. Broader care-team/facility access, comprehensive audit/security review and production protections remain open. A staff facility field is not facility-level authorization. |
| 3 — Clinician workspace | Staff chart, individual/group sessions, notes/signatures/amendments, plans/goals/reviews, authored forms, shared tasks. | Dedicated clinician workspace, full calendar, richer chart, attachments, alerts/messages, telehealth and live session-to-document workflow. |
| 4 — Billing workspace | Policies, authorizations, claims/readiness, denial recording, payments/balances and reversals. | Dedicated work queue, appeals, reconciliation, external exchange and remaining financial lifecycle. |
| 5 — Executive / leadership | Basic census/counts, financial totals, staff/tasks, per-goal progress data. | Capacity and outcome trends, documentation posture, staff workload, risk indicators, executive action queue and useful drill-throughs. |
| 6 — Integrations and automation | Internal accounting/claims CSV exports and local WAV adapter; research on integration boundaries. | Active FHIR pipeline, validated Kipu/QuickBooks migration, jobs, payer eligibility, broader reports and claim polling. Parked source folders do not mean working integrations. |
| 7 — Production readiness | CI definitions, local start/backup/restore scripts, loopback checks, prepared Docker image. | Hosted architecture, secrets, monitoring, encrypted backups/DR, security/performance/compliance acceptance. Local Docker runtime smoke remains blocked by the machine's Docker failure. |

The original TODOs for module boundaries, role-aligned persistence, identity bootstrap, audit/logging, replacing mock responses, React integration and activating parked modules are therefore not replaced with a shorter feature list. Persistence and the React bootstrap have substantive implementation; identity remains deferred; the others need the phase-specific work above.

## Corrections after baseline 981afd0

- Adult eligibility enforced on new clients, changes to birth date, transition into intake/active status, and new/activated episodes. Uses facility-local calendar date and the documented March 1 anniversary for February 29 births in non-leap years. Existing records are not erased; unchanged stored dates permit unrelated corrections. Backend eligibility is authoritative; the UI supplies an adult-program hint and DOB limit.
- Light, dark and system themes with a local preference; system preference changes are observed while System is selected. The control is placed in the interface rather than floating over content. Custom controls remain in use.
- Refresh now has a visible label, busy state, error handling and success confirmation.
- Duplicate top avatar removed. Exit workspace returns to local operator selection and clears the selected chart/navigation state. This does not claim to terminate an authenticated session.

These are bounded defect corrections. They do not close the major workspace, calendar, telehealth, portal, records-lifecycle or billing gaps. Current checks are recorded in [verification](verification.md).

## Recovery sequence and acceptance gates

1. **Role-specific information architecture and designs.** Inventory the remaining Figma frames; map each screen/action to a role and a persisted domain operation. Deliver clinician daily work, billing work queue and executive operations as distinct journeys using shared records. No decorative metric or clickable-looking control without defined behavior, empty/loading/error states and a source definition.
2. **Scheduling and telehealth.** Unified individual/group day/week/month calendar, facility timezone, clinician/client availability and conflict handling, cancellation/rescheduling, chart links and task handoffs. Then a real two-party call: join/waiting room, device/permission failure, admission, leave/reconnect and completion linked to documentation. Choose media transport explicitly; do not substitute a Join button for a working call or pretend a blank integration is connected.
3. **Client and records lifecycle.** Complete intake validation, duplicate handling, consent/form journey, chart correction, attachments, archive/entered-in-error paths and a defined controlled deletion/retention policy. Add the client-facing appointments/forms/session journey, while preserving the deferred identity-provider boundary for local development.
4. **Session documentation and finance.** Live consent-aware speech capture and human-reviewed form/note suggestions; evaluate accuracy and group speaker attribution. Finish appeals, reconciliation and billing queue transitions; distinguish internal state from actual payer acknowledgment throughout.
5. **Integrated acceptance and operations.** Run each role from onboarding through its daily workflow and offboarding using only explicitly synthetic test fixtures in isolated databases. Verify permissions and exceptions, failed inputs, concurrent edits, reload/restart persistence, accessible desktop/mobile themes and recovery from service failure. Publish a workflow-level result ledger. Keep public hosting dependent on the owner-deferred identity and required deployment controls.

A reviewable initial product must pass those agreed workflow gates. Passing a build or having more agent calls is not a substitute. Scope changes should be explicit, with remaining work visible to the owner rather than silently excluded.
