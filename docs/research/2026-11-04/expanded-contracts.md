# Nexus expanded implementation contracts

November 4, 2026. These contracts reconcile the expanded Grok specialist reports and adversarial reviews with the repository. They define the next implementation work; they do not mark capabilities complete. The [50-capability acceptance ledger](../../product/acceptance-ledger.md) remains unchanged. Outpatient comes first, with additional settings preserved in scope. State, payer mix, customer policies and the second care setting are still unspecified.

## Engineering decisions

Keep the Rust/Actix/SQLx/PostgreSQL modular monolith and React/TypeScript frontend. Split work by domain contracts and small changes rather than introducing services to compensate for incomplete workflows. PostgreSQL is the durable store; local model files, runtime configuration and media keys are separate operational dependencies. A PostgreSQL backup alone cannot recover encrypted recordings without their key.

Use the existing persisted transcription and drafting jobs. Their ownership locks, running-state recovery, consent/source rechecks and audit records are real mechanisms. They do not yet provide supervised restart, bounded retry histories or measured fairness. Neither a generic replacement queue nor repurposing those specialized tables for unrelated report delivery is justified.

The current full-row bootstrap and organization-level mutation lock deserve measured load tests. They are not proof of a production capacity failure. Establish realistic synthetic organization sizes, query plans, response size, memory and p50/p95 latency before selecting pagination, aggregate endpoints, finer locks or caches. Any cache must include authorization and source-version boundaries.

Branch work under `codex/`, retain small reviewable changes, serialize migrations/shared routing, and validate integration against `nexus_test`. A worker proposal is evidence to inspect, never an executable command or accepted patch. See [Git workflow](../../development/git-workflow.md).

## Financial identity and money

The coordinator reproduced two live claims for one session using `h2019` and `H2019`. Migration 021 and the current Rust changes give service-code identity one database definition: trim the explicit Unicode boundary-whitespace set, perform ICU root uppercase, and compare the result byte-for-byte. Internal spaces and punctuation remain significant; composed and decomposed Unicode forms are not silently normalized. Blank codes are invalid. Historical text and snapshots are preserved.

The same definition must govern catalog uniqueness, live-claim uniqueness, authorization matching and claim-to-encounter snapshot checks. A case-variant draft PATCH must fail without changing the claim or audit history. Voiding a claim permits a legitimate replacement but cannot erase the first record. Existing authorization policy/date/status/unit rules continue to apply.

The migration is a maintenance operation with application writers stopped. Its compatibility preflight and collision checks fail atomically rather than choosing which financial record to retain. The migration holds locks; it is not an online zero-downtime upgrade. PostgreSQL must supply the ICU `unicode` collation. An ICU upgrade requires compatibility checks and index rebuilding under a reviewed maintenance procedure; refreshing collation metadata alone does not rebuild stored index keys. [PostgreSQL collation documentation](https://www.postgresql.org/docs/17/collation.html).

Claim-form defaults have a separate units contract: persisted defaults are integer cents; conversion to display dollars happens once; submission converts once back to integer cents. Explicit initial values take precedence over catalog defaults. Preselected sessions and interactive session changes must use the same conversion, including zero, missing defaults and fractional-dollar amounts. This change is separate from service-code identity and still needs its own integrated tests.

## Billing workflow and readiness packets

Build the workflow around observable events: coverage recorded or verified, authorization received, service scheduled, attendance recorded, service delivered, documentation reviewed/signed, charge created, readiness evaluated, packet prepared, transmission attempted, acknowledgement received, adjudication received, payment allocated, and reconciliation completed. Each event needs its own evidence. A readiness pass is neither a submitted claim nor a promise of payment.

CMS distinguishes eligibility, professional claims, implementation acknowledgements, claim acknowledgements and payment/advice transactions. An exported CSV or manually entered submission reference cannot establish any of these external responses. [CMS standard transaction guide](https://www.cms.gov/Outreach-and-Education/MLN/WBT/MLN4462429-MLN-WBT-1500/1500/lesson03/11/index.html).

The next internal billing slice should define these records and transitions:

| Record | Required behavior | Correction and concurrency rule |
|---|---|---|
| Charge and line version | Link actual service, patient/episode, provider, code/modifiers, units, cents and supporting document versions | Preserve prior versions; economic duplicate guard independent of payer routing |
| Readiness evaluation | Store policy version, evaluated source versions, blockers and responsible role | Re-evaluate after source change; never silently carry a stale pass forward |
| Packet manifest | Template version, purpose/recipient, included record versions, omissions and validation result | Export is immutable evidence; regenerate a new manifest after correction |
| Submission attempt | Stable internal idempotency key, payload hash, adapter, attempt status and external evidence | Unknown outcome enters reconciliation; no blind retransmission |
| Adjudication event | Original payer identifiers and response, line allocations and reason codes | Duplicate ingestion is idempotent; corrected responses link to history |
| Payment allocation | Integer cents, payment source, allocation target, posting/effective dates | Reversals append compensating entries; partial allocation stays visible |
| Reconciliation item | Unmatched amounts, ownership, reason, evidence and resolution | A manual local posting never claims verified bank settlement |

Authorization consumption must remain attached to stable delivered-service identity across claim corrections; creating a draft replacement must not release units for care already delivered. Keep receipts and unapplied credit separate from allocations so an overpayment does not bypass the existing claim cap. Legacy backfill needs an explicit reconciled opening policy and unclassified balances where evidence is insufficient; do not recognize revenue for every draft claim or infer patient liability from a client payment.

A configurable packet should prefill known demographics, coverage, attendance, service details and approved documentation references. Show each field's source, allow an explicit override with reason when permitted, and retain the selected source version. Missing facts remain missing. Autocomplete must not invent attendance, medical necessity, diagnosis, signatures, payer requirements or paid amounts.

No-show and cancelled encounters must not satisfy a delivered-service billing requirement. A correction after billing creates a review/correction workflow. Cancelling a calendar event must **not automatically reverse a payment or alter a signed note**. A separate authorized financial action records the justified correction.

Prioritize CSV and document packets as internal handoffs while preserving an adapter boundary for genuine payer exchange. Start with an authored outpatient policy, then add IOP/PHP or another chosen setting using versioned rules. Do not guess a universal insurance form list. See [billing lifecycle](billing-lifecycle.md), [competitor comparison](billing-competitors.md) and [outpatient billing contract](outpatient-billing-contract.md).

## Billing and executive observability

Billing needs an actionable work queue: blocked charges, unsigned supporting records, expiring/insufficient authorization, unsent ready packets, rejected transmissions, denials with deadlines, unmatched payments and reconciliation exceptions. Every count opens exactly the records included in it; each item has an owner, next action and age. Group similar work only when the action and permissions are genuinely shared.

Executives need a different overview: financial movement, census and capacity, access/wait time, attendance, documentation throughput, staffing workload and outcome follow-up. An involved owner can drill into permitted records; a finance-only owner should not receive full clinical rows merely because the UI hides them. Define server projections before expanding dashboards.

| Indicator | Definition required before implementation | Current limit |
|---|---|---|
| Cash received | Payer/client cash receipts less matching reversals, excluding adjustment-source entries, using an explicit event-date window | Local ledger evidence; not bank reconciliation |
| Outstanding receivables | Charges less valid allocations/adjustments as of a named point in time | Current single-service claims; historical reconstruction needs event history |
| Readiness backlog | Eligible delivered services with at least one unresolved blocker | Distinguish missing evidence from zero work |
| Clean claim rate | Agreed processing-stage denominator and no-manual-intervention numerator | Local ready status is not a clean payer-claim measure |
| Denial rate | Claim- or line-level adjudicated cohort, first/final decision policy and date window | Denied local status alone is insufficient for a reliable standard measure |
| Days in A/R | Defined net receivable balance and average daily net patient revenue | Do not substitute cash or gross charges for missing net revenue |
| Capacity/census | Facility/program, effective dates, eligible population and denominator | Current census differs from historical occupancy |
| Attendance | Scheduled eligible encounters, explicit exclusions and completed/no-show outcomes | Group parent count and patient attendance count are different measures |
| Outcomes | Named instrument/version, baseline/follow-up windows and missing-data policy | Notes or treatment-plan completion are not validated outcome scores |

HFMA defines revenue-cycle measures with specific numerators, denominators and source systems. Nexus should retain those definitions or label an internal proxy clearly rather than reuse a familiar KPI name for a different calculation. No industry-wide target or benchmark is adopted here. [HFMA MAP Keys](https://www.hfma.org/data-and-insights/map-initiative/map-keys/).

Customization should use typed, allowlisted filters and sections: facility, program, payer, clinician, period and status where authorized. Persist the saved view's owner, definition version and visibility. Recheck permissions whenever it is opened, exported or distributed. Do not accept arbitrary SQL or rely on permissions captured when the report was scheduled. Start with saved views and CSV; scheduled delivery and PDF/XLSX are separate capabilities.

## Identity, profiles and communications

Local operator headers exercise authorization rules but are not authentication. Production identity remains an explicit deployment dependency. Staff, patient and authorized adult proxy principals need separate contracts; a patient must never gain staff routes by changing a frontend workspace mode.

Define resource-, patient-, facility- and field-level permissions server-side. The existing primary-clinician relationship is not a complete care-team assignment model. Exceptions need effective dates, purpose, grantor, revocation and audit. Recheck scope after waiting for mutation locks, at export/download time and before publishing asynchronous results.

The bottom-left profile disclosure should expose identity, theme, available settings and a clear way to leave/switch the local operator. Unsaved-change protection still applies. Workspace modes reflect the actor's permitted views; they are not multiple organizations or a substitute for onboarding. A future profile card should request only the fields/actions allowed for the current viewer, rather than downloading a complete profile and hiding pieces.

For messaging, define a thread purpose, explicit participants, patient/episode linkage where applicable, message versions, read markers and attachment references. Revocation must affect subsequent reads/search/downloads. Notifications should avoid clinical content outside the protected application. A staff message, patient message and cosign request may share presentation primitives without sharing all permissions or retention semantics.

The portal requires explicit release states for records/resources, appointment change requests, consent workflows and authorized financial views. Adult proxy access needs documented scope, dates, recipient and revocation; family relationship alone never establishes permission. No secure portal or caregiver capability is claimed at this checkpoint.

## Clinical documentation and structured records

Supervision needs a real assignment/policy model before a cosign queue. Preserve the authored version and the supervisor's separate attestation. A co-sign request must reference the exact note or form version and patient/organization. Source changes, rejection, reassignment and amendment cannot silently reuse an earlier approval. Existing signed-record triggers must continue to protect their original fields.

The clinical council's sample SQL contained invalid composite references and reused an unrelated note-protection trigger. It is **not an approved migration**. The actionable deliverable is a staged contract: supervision policy and assignments, version-bound requests/decisions, sign/readiness enforcement, supervisor queue, then concurrent/revocation/amendment journeys.

Medication reconciliation, allergies, diagnoses, labs, observations and referrals remain distinct missing clinical modules. Implement typed records with effective time, author/source, code-system/version where relevant, status and explicit correction history. Do not infer a diagnosis or medication order from generated prose. A lab's units/reference range and review state belong to that result version; acknowledgement is not the same event as releasing it to a patient. Medication administration must reference an order and distinguish administered, missed, held and corrected events.

For discharge, tie the reviewed summary, aftercare plan, referrals and follow-up ownership to the episode. Required-form rules can support authored discharge policy; they do not themselves create aftercare continuity. Owner/clinician review should validate the forms and workflow before treating them as clinically complete.

## Scheduling and groups

Represent a visible group once on the calendar while preserving per-patient attendance and documentation. If a filtered parent is absent, do not hide an otherwise authorized child encounter. A visible subset must not imply a complete roster or expose hidden patients through totals, names or tooltips.

Recurrence is a separate future contract: store the recurrence rule, facility timezone, original wall time, generated occurrence identifiers and exceptions. Define this occurrence, future occurrences and whole-series edits explicitly. DST gaps require correction and folds require an explicit occurrence. Signed or billed encounters keep their snapshots; recurrence edits cannot rewrite them.

Room occupancy, clinician availability, patient conflicts and program capacity have different denominators. Check the applicable constraints inside the mutation transaction. A proposed recurrence cancellation flag that automatically changes billing is rejected; financial correction remains a separate audited operation.

## Media, speech and knowledge

Whisper base.en through whisper.cpp and Qwen3-4B through llama.cpp are local third-party models. Nexus controls hosting and integration; it did not train those models. Uploaded recognition, saved-media processing and reviewed-source drafting exist. Live streamed recognition, diarization, broader clinical accuracy and semantic retrieval remain open.

Keep model evaluation separate from connection tests: use consented/synthetic evaluation sets, specify language, acoustic conditions, clinically important omissions/substitutions, silence behavior and provenance failures. Generated note drafts require clinician review and independent signing. Timers and inferred speech duration cannot establish billable service duration.

Native WebRTC signaling, invitations and local call features exist. Verify separate networks, TURN, browser/device compatibility, participant removal and consent withdrawal across an actual call before accepting telehealth. Current own-device recording is not automatic complete mixed-call capture. Preserve each track's participant and consent provenance.

Retry and supervision should retain each failed attempt and its inputs. A retry creates a new attempt linked to the original, revalidates current access/consent/source versions, and uses bounded backoff only for explicitly retryable failures. Rewriting terminal failure rows or merely continuing after failed recovery can lose history or strand running jobs. The current council's retry patch is therefore rejected. Test ownership loss, database restart, child-process cancellation, stale source, revoked consent and successful publication races before changing the worker lifecycle.

## Attachments and lifecycle

An attachment needs organization/patient ownership, an immutable content version, storage metadata, permitted type/size, quarantine/scan status and an authorized download path. OCR is a derivative with its own provenance and review status; uploading or extracting text does not make it approved clinical evidence. A checksum can support integrity or scoped deduplication but must not become a global cross-patient uniqueness constraint.

Separate original content, storage objects, extracted text and clinical references. Deletion/retention needs legal holds and a reviewed policy for derived knowledge and signed records. Do not cascade-delete signed evidence or null its provenance. Preserve a meaningful tombstone and audit when content can lawfully be removed; the exact retention/legal policy requires customer and jurisdiction decisions.

Do not modify the existing recording encryption context merely to prepare for a nonexistent attachment path. A new attachment context must be domain-separated and tested without changing decryption of old media. The council's generalized helper is a proposal, not an implemented storage service.

The deeper attachment proposal was rejected as implementation-ready: generic editable fields must not include scan status, hashes, measured byte counts, uploader identity or legal holds. A patient-write permission cannot authorize declaring an object clean. Trusted scanner evidence must bind the exact immutable object version. Define attachment metadata/filename permissions explicitly rather than promising billing denial while reusing broadly readable client access. Buffered `Bytes` APIs are not streaming, a short MIME sniff does not establish safety, and guessed archive expansion limits are not validated defenses. Start with a conservative supported-type policy and actual bounded parsers/scanners. Do not introduce OCR cascade deletion or a guessed 30-day cleanup rule before reference/hold policy exists. [OWASP file-upload guidance](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html).

## Media and worker follow-up corrections

The complete visit UI already waits for admission/host presence before constructing peers, preserves microphone audio while sharing, restores the camera on share end, advances signaling cursors and offers media reconnection. Signal expiry is already defined in migration 011. These were omitted dependencies in a worker packet. Display video is explicitly excluded by recording capture, its consent UI and contract; supporting screen recording requires a separate reviewed scope.

A proposed close helper cleared participant connection IDs while promising to preserve final upload grace. The real upload authorization compares participant and track connection IDs first, so that change would reject final uploads and was not applied. A proposed stronger ICE parser also incorrectly rejected empty end-of-candidates signals with null mid/index. Preserve valid protocol behavior. Glare/early-candidate and rejoin/open-track scenarios remain reproduction targets, not newly confirmed failures. [W3C addIceCandidate algorithm](https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-addicecandidate).

Worker terminal-failure stranding is source-confirmed: processing can commit `running`, then fail, and a failed terminal/audit transaction leaves a row that the queued-only loop never revisits. Its first proposed repair used repeated global scans through pooled connections, which could fail another worker's job after ownership loss. Reject that repair and its mirrored SQL/predicate tests. The narrow revision should retry the exact pending job, preserve its original static error code, and perform startup recovery/failure transactions through the dedicated advisory-lock connection. Require real database fault-injection, cancellation/ready preservation and ownership-loss tests before integration. This does not by itself add automatic worker failover or resolve every publication-ownership race.

## Branding and interaction requirements

The owner direction is retained: muted earthy green with neutral light/dark surfaces, firmer corners, readable header spacing, restrained growth motifs, custom icons/components and subtle motion. The brand must organize clinical, financial and executive work without making every surface green. Implement this after functional workflows are stable.

Explore three mark families: a junction of three paths, a restrained branching form, and an open geometric growth structure. Judge originality, meaning, silhouette, monochrome reproduction and small-size recognition. Prepare icon-only, horizontal lockup, wordmark, dark/light, favicon and print variants. No mark or palette is approved, and the wordmark need not be uppercase. Avoid adopting the council's arbitrary stroke-width, file-size and node-count recommendations as brand requirements.

Use primitive, semantic and component tokens for color, typography, geometry, space and motion. Define default, hover, focus, pressed, selected, disabled, loading, error and success states. Person cards must work on keyboard and touch as well as hover. Status needs a label plus color/icon; emoji can be optional decoration rather than the sole meaning. A growth-style timeline still needs clear event time, actor, action and correction linkage.

Keep patterns subtle and outside dense notes, forms and financial tables. Reduce excessive panel/pill framing and unexplained empty space. Background texture must not compete with reading or state recognition. Add motion for feedback and continuity, with a reduced-motion alternative; avoid perpetual decorative animation.

Correct an important council claim: WCAG 2.2 Focus Appearance is **AAA**, while Focus Not Obscured (Minimum) is **AA**. A strong focus ring remains a useful Nexus design target, but do not mislabel the criteria or claim conformance from a token choice. Test contrast, keyboard behavior, zoom, error recovery and touch targets on complete workflows. [W3C WCAG 2.2 additions](https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/).

## Delivery order and acceptance

1. Close reproduced integrity defects with migration and regression evidence; integrate the claim-money fix separately.
2. Complete profile/account navigation and the confirmed form/calendar correctness work without a full visual rewrite.
3. Define the shared access/provenance contracts needed by attachments, supervision, portal and clinical records; build vertical workflows with explicit permission/correction tests.
4. Implement financial event history, versioned packets and reconciliation before sophisticated billing metrics. Live exchange remains lower priority, with clear synthetic adapter labeling.
5. Build executive definitions and matching drilldowns from available facts. Add new metrics only with their required data.
6. Exercise complete outpatient journeys, then apply the branding system to those stable workflows. Preserve the broader 50-capability scope and unresolved additional-setting requirements.

For each slice record the revision, platform, tests, actual browser path, affected records and remaining limitations in [verification](../../development/verification.md). November 13 remains a high-risk target. A specialist report, successful build or large test count does not establish product acceptance.


## Follow-up review corrections — November 4

The subsequent financial, packet, denial-workbench and executive-metric councils returned useful directions but also material errors. Their raw reports are not accepted specifications. Core review and primary sources establish these corrections:

- Stable delivered-service identity must prevent duplicate economic charges across claim versions and payer routing. Including an authored timestamp in the unique key defeats that protection. The local canonical function is `nexus_service_code_v1`; proposed DDL must use valid scoped foreign-key targets and actual migration conventions. Append-only comments and aggregate views do not enforce immutable, nonempty balanced postings or concurrent allocation limits. Enforce invariants transactionally, including sealing a posted transaction against later lines.
- Keep receipts, allocations, adjudications, patient responsibility and bank reconciliation separate. A receipt may be unapplied; do not require every receipt to have a prior adjudication. A recoupment is not automatically a new payer receivable. Worked examples must reconcile both journal entries and balances after every event. Preserve current exports until additive projections prove parity, including reversals and adjustments.
- Packet autocomplete copies verified structured values and identifies gaps. It must not infer a diagnosis code from narrative or invent billed units from a session that has no such field. Source provenance does not expand permissions: billing users do not gain restricted signed-note text simply because it supports a packet. Export only purpose-appropriate authorized fields, retain exact source versions, preserve the original export manifest, and append correction/staleness evidence before generating another packet version. For CMS-1500 mappings, rendering NPI is 24J and billing NPI is 33A; 24A is the service date. [CMS claim-form instructions](https://www.cms.gov/outreach-and-education/mln/wbt/mln4462429-mln-wbt-1500/1500/lesson04/19/index.html).
- Rejection, denial, appeal and corrected claim are separate paths, not a mandatory linear sequence. Do not assign a generic appeal deadline when payer/product/state rules are unknown. Configurable internal follow-up SLAs are distinct from filing deadlines. Medicare's first redetermination example is generally 120 days after receipt, demonstrating why a generic 30–90-day default is unsafe; it is not a universal Nexus deadline. Current financial write permissions remain billing/admin, not clinical lead. [CMS redetermination guidance](https://www.cms.gov/medicare/appeals-grievances/fee-for-service/first-level-appeal-redetermination-medicare-contractor).
- Attendance requires a defined appointment population that includes relevant no-shows and avoids counting both a group attendance row and its linked client session. A readiness backlog includes missing/unsigned documentation and delivered services lacking a claim. Cash-allocation coverage uses a cash receipt cohort with a defined as-of date; adjustments are separate. Allocations posted this month divided by cash received this month can exceed 100% because the cohorts differ. Unmatched item count, unmatched cents and aging are separate measures; cents divided by item count is not a reconciliation percentage.
- Use HFMA labels only for the matching published definitions. Clean-claim rate concerns processing-tool edits before submission, not clearinghouse acceptance. The published denial rate uses remitted claims and includes initial/appeal denials. Raw cash or outstanding cents are not substitutes for ratios with different denominators. Custom Nexus metrics should be named and defined as such. [HFMA MAP Keys](https://www.hfma.org/data-and-insights/map-initiative/map-keys/).

Current-source corrections also matter: authorized empty ledger collections return zero, not unknown; today's session count includes all statuses; active authorizations whose end dates have passed can appear in the expiring collection; note/task measures are counts rather than implemented rates. Upcoming plan-review collections lack a strict upcoming-date cutoff. Existing navigation does not establish exact cohort-filtered drilldowns. Billing bootstrap includes patient-identifying fields, so it is not an aggregates-only/no-PHI surface. Versioned clinical/catalog snapshots already exist, while durable financial export manifests remain proposed.

Competitor comparisons must use actual primary product documentation and distinguish workflow from marketing. Remove unsupported denial-rate benchmarks, universal automation promises and claims of Nexus superiority. A supported workflow lesson is to connect completed documentation to billing tasks while providing billing information separately from clinical note access. [TherapyNotes billing workflow](https://support.therapynotes.com/hc/en-us/articles/30661397280155-Quick-Start-Billing), [TherapyNotes billing workspace](https://support.therapynotes.com/hc/en-us/articles/30661348713115-Main-Billing-Page).

The coordinator queued explicit correction requests for these findings, alongside narrower implementation/test proposals. Acceptance still requires executable money examples, concurrency/correction tests, denied-field disclosure checks and source-backed metrics; a corrected prose report alone is insufficient.

The next metric revision still mixed cohorts: a qualifying $100 receipt and a $50 reversal of an older excluded receipt can produce a misleading 200% allocation figure when the denominator is period net cash. Use the same stable receipt cohort and its attributed reversals for allocation coverage. A zero net balance is not necessarily an empty cohort. Conversely, dollars per encounter is a valid named average; it is simply not a percentage. Do not manufacture a persisted-data defect from a `partially_paid` status or null/fractional money that the current schema disallows. The current residual adjustment formula is valid under its closed payment-source enum; flow-versus-snapshot labeling and checked arithmetic remain separate review topics.

Preserve the current payment-reversal status behavior: reversing a positive settled payment reopens its balance, and replacement payment currently requires `submitted` or `denied`. Keeping the claim `paid` would block correction. Splitting claim lifecycle from financial position is future design. No-policy self-pay, no-show fees, secondary coverage and corrected submitted claims are also future paths; optional authorization is not optional insurance policy. Readiness failures are not payer denials, and a blocker's owner role grants no additional clinical disclosure permission.

Secondary billed charges are not universally equal to the receivable transferred from the primary payer. Keep charge, submission amount, adjudication, responsibility and settlement separate. Refunds and takebacks need evidence before assigning new patient debt; provider-level offsets may not identify a patient or represent a separate cash withdrawal. [CMS secondary-payer instructions](https://www.cms.gov/regulations-and-guidance/guidance/manuals/downloads/msp105c05.pdf), [CMS remittance guidance](https://www.cms.gov/medicare/coding-billing/electronic-billing/health-care-payment-remittance-advice).

The revised journal proposal still lacked its claimed guarantees: line-insert triggers do not validate an empty header, update/delete guards do not prevent adding lines to a sealed entry, and nullable or organization-only foreign keys do not establish all patient/service/coverage relationships. A cutover that writes new receipts while CSV reads only old payments would omit cash. Require an atomic reader cutover or deduplicated reconciled bridge, with executable examples and failure tests. Do not infer causal replacement chains among historical voids solely from timestamp order.

## Dependency-complete review corrections

Some specialist source packets omitted downstream validation or later migrations. Missing packet evidence is not evidence of a missing application feature. Subsequent review traced the complete call graph and rejected the following broad claims before implementation.

**Drafting already validates provenance and consent.** `clinical_model::validate_draft` requires known source IDs and exact nonempty quotations; the adapter rejects truncated/invalid output and records model SHA and prompt version. `current_sources` resolves refreshed knowledge through source collection and excerpt/processing-consent checks. Worker processing, acceptance and note signing revalidate relevant evidence. These are current mechanisms, not proposed replacements. See [clinical model validation](../../../api/src/clinical_model.rs) and the [drafting contract](../../development/knowledge-drafting-contract.md).

Historical generation evidence remains available under chart permissions after a source or processing-consent change; clinician editing after acceptance is also intentional. Sealing historical reads or forcing note text to equal the original model-body hash would change that contract. A useful improvement is a clear view of the original proposal, accepted statements and later clinician edits. Changed text must not be mislabeled as wholly clinician-authored merely because a hash differs. Exact citations establish source correspondence, not clinical entailment. Evaluate negation, historical attribution, contradictions, omissions, unsupported statements and source instructions separately. Same-patient historical evidence is not automatically out of scope; its permitted purpose/context needs an explicit policy.

**Signed-document protections already exist.** Plan history guards are in migration 005, signed-form guards in 006, and append-only plan reviews in 004. Plans/forms have scoped composite keys; migration 016 supplies the note tenant/patient key. Workflow transitions lock the organization and record, with transactional audit. The claimed unprotected last-writer race was not demonstrated. A displayed-version requirement may still improve signing intent, but it is a proposed enhancement.

Separate supervisory requests and immutable decisions remain needed. Their lifecycle must allow return → amendment → new approval without borrowing approval from an earlier signed version. Choose a terminal-per-artifact rule or explicit append-only supersession. A unique historical `attested` row is not a unique *current* attestation and can accidentally prohibit reapproval forever. Record applicable authority and policy versions, recheck current permission after locks, define precedence for overlapping requirements, and preserve existing valid records where signer and author differ.

**Portal acceptance must be atomic.** Consume the invitation, create/bind the principal, activate the appropriate grant/session and record audit in one transaction. Rollback restores token consumption; committed response loss is a different replay/recovery case. Follow organization-before-row locking and recheck expiry, logout, binding/grant revocation and record release after acquiring locks. Preserve a future grant start instead of replacing it with acceptance time. Mistaken recipient/contact changes need revocation and reissue handling. Proposed portal records must enforce organization/patient-consistent relationships; an empty messaging surface stays unavailable.

Proxy access requires evidence of patient delegation or applicable legal authority. Staff approval and recipient adulthood alone do not establish that authority. Its legal scope remains jurisdiction dependent. The portal cannot be publicly enabled while staff UUID headers still act as credentials; keep the current loopback/synthetic-data boundary until verified identity covers the whole service. [HHS personal-representative guidance](https://www.hhs.gov/hipaa/for-professionals/privacy/guidance/personal-representatives/index.html).

**Production identity needs a complete cutover inventory.** An authentication-session table must have a distinct name: `sessions` already stores clinical encounters and must never be truncated as an authentication rollback. Programs, admissions and telehealth management parse actor headers outside `access::actor`; bootstrap also branches on header absence. Frontend recording, speech import, registration and exports make requests outside the central API helper. All must share a mode-aware identity path before cookie-based authentication can replace operator headers. Existing telehealth bearer capabilities and offboarding transfers/revocations should be extended, not duplicated or declared absent. Organization/staff-required audit records also need an explicit design for unauthenticated security events.

Cross-origin localhost ports are not automatically cross-site. Login correlation cookies require callback-compatible SameSite behavior distinct from the staff session cookie. Bind one-use state, nonce, PKCE and allowed redirects; a verified external subject alone cannot create the first administrator. Define recent-authentication age explicitly. These are proposed production requirements, not an implemented login. [MDN site definition](https://developer.mozilla.org/en-US/docs/Glossary/Site), [OpenID Connect](https://openid.net/specs/openid-connect-core-1_0.html), [OAuth security guidance](https://www.rfc-editor.org/rfc/rfc9700).

**Packets need additive history and complete review preconditions.** Positive non-null claim units already have a database constraint in migration 002, and readiness reloads persisted claims. The defensive units fallback is not a reproduced null-units bypass. Claim/session patient equality and timezone-aware timestamps also already exist. Do not force edits of ready/submitted/paid legacy claims merely to add new provenance: permit authorized additive attestations against their immutable versions. Sealing must compare the preview's complete content/version hash; `claim.updated_at` alone cannot detect independently changed provider, template or policy evidence. Preserve minimized values and typed source references alongside hashes. Internal packet requirements depend on artifact purpose; not every packet universally requires an NPI or authorization. Preserve the current internal claims CSV's denial/void/reference history, and do not relabel local payment posting as payer adjudication.

## Clinical record scope

The medication/laboratory follow-up proposes a useful manual-recording first slice, but cannot remove scheduled MAR, refills, orders/specimens or critical-result follow-up from the acceptance ledger. Preserve unknown values rather than defaulting an unspecified `as_needed` field to false; label the actual source instead of calling every record patient-reported. Allergy completeness and positive assertions must reconcile transactionally, so a new contradicting allergy cannot coexist with a current unqualified no-known-allergy assertion. Proposed admin-attestation restrictions change current policy and require an explicit decision. FHIR terminology/resource alignment is not a claim that external prescribing or lab interfaces are implemented. [HL7 AllergyIntolerance](https://hl7.org/fhir/R4/allergyintolerance.html), [HL7 MedicationAdministration](https://hl7.org/fhir/R4/medicationadministration.html).

A longitudinal problem-list slice should keep provisional concerns visible, distinguish clinical confirmation from code/payer eligibility, and preserve the exact diagnosis version attached to a claim. Resolve conflicting linked diagnosis and saved claim text explicitly. The worker's draft DDL had invalid foreign-key arity, incomplete patient scope and editable verification state that bypassed confirmation; it was not applied. Observations need typed values and appropriate units/components rather than a universal positive-number rule.

Form snapshots, author/ancestry allowlists, required-field validation, signed-history triggers and the structured response editor already exist. A narrower open contract concerns which signed amendment may support *new* notes and requirements. Current requirement evaluation reduces responses to template/episode/status and can let an earlier eligible ancestor satisfy a requirement despite a later source-ineligible amendment. Define the current signed amendment and competing-branch policy under existing locks, preserving original history. Do not automatically invalidate every historical form when a live template changes, require the same author for every correction, or equate a valid paraphrase with a verbatim excerpt.

For lifecycle work, begin with a provenance inventory and non-destructive impact preview. Bind future destruction approval to exact object/impact/policy versions and revalidate at execution, including group records, holds, cycles and incomplete graph traversal. The current media vault has one installation key with recording-specific authenticated context; it does not have a separate key per recording. Rotation and retirement are different operations. A restored backup must honor durable deletion decisions. Reject guessed universal retention periods, magic court-order bypass strings and automatic cascading deletion of clinical derivatives. HIPAA documentation retention is not a universal medical-record retention schedule. [HHS retention explanation](https://www.hhs.gov/hipaa/for-professionals/faq/does-hipaa-require-covered-entities-to-keep-medical-records-for-any-period/index.html), [45 CFR 164.530(j)](https://www.ecfr.gov/current/title-45/subtitle-A/subchapter-C/part-164/subpart-E/section-164.530).

## Model evaluation and calendar integration boundaries

The current Whisper model and implementation are replaceable local dependencies. Whisper is an OpenAI-created general-purpose recognition model with MIT-licensed code/weights; running it locally does not call the hosted API. [Whisper source and license](https://github.com/openai/whisper). NVIDIA's Parakeet TDT 0.6B v2 is an alternative English model with word timestamps and CC-BY-4.0 terms, not a model currently integrated into Nexus. Its published benchmark results are not evidence of Nexus clinical accuracy or Mac latency. [Official NVIDIA model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2).

Evaluate candidate models on the same consented or synthetic audio, with recorded model hashes, hardware and runtime. Measure clinically important entities, negation, attribution and omissions alongside word error rate, latency and reviewer correction effort. Silence/noise, overlapping speakers, accents, dates/numbers and difficult devices need separate cases. Do not advertise validated clinical performance from the existing synthetic connection test. `NEXUS_WHISPER_CLI` and model configuration are environment settings, not request headers.

A follow-up worker's evaluation harness generated expected mock answers and graded those answers. That is not model evaluation and was rejected. Constructed answers can test validators; fidelity evaluation must save actual model outputs. Fully specified synthetic cases need source text, dates, attribution, allowed template sections and review rubrics. Changing symptoms across different dates is not automatically a contradiction, and a source's instruction not to document something remains data for human review rather than a new model instruction. Unvalidated weighted scores and a 90-point acceptance threshold were also rejected. Report concrete case failures and reviewer findings before defining a supported quality threshold.

For future Google Calendar sync, retain a separate provider mirror and stable link to Nexus encounters. Google uses initial/full sync and incremental tokens; an expired token can require rebuilding the mirror. Nexus clinical encounters, attendance and billing evidence must survive that reset. Persist the new token only after the complete paginated result is applied. [Google sync guidance](https://developers.google.com/workspace/calendar/api/guides/sync).

Recurring instance identity uses `recurringEventId` plus `originalStartTime`, including moved instances. Keep cancellation exceptions rather than recreating them on the next expansion. Private extended properties are private to an event's calendar copy; they are not a secret vault. Store opaque identifiers there, not clinical text or credentials. [Google event reference](https://developers.google.com/workspace/calendar/api/v3/reference/events).

Push notifications signal changes and require a subsequent API fetch. Documented resource-state values include `sync`, `exists` and `not_exists`; a collection's `exists` notification can include a deletion. Do not interpret it as proof every event still exists. Channels expire and need explicit replacement, with possible overlap. Deduplicate notifications and retain a polling/reconciliation path. [Google push guidance](https://developers.google.com/workspace/calendar/api/guides/push). OAuth scopes, delegated-calendar rights, field disclosure, ETag conflict handling and disconnect behavior need contract tests before genuine two-way synchronization is accepted.


## Final core-review corrections: signed amendment heads

The final Grok provenance council's proposed implementation is held, despite identifying the real stale-ancestor new-use issue. Its suggested `sources::collect → knowledge::resolve → refresh → sources::collect` call graph is recursive. Existing note lineage follows ancestors, not forward descendants, and does not implement a generic form-head walker. Amendment-parent uniqueness does not exist; the unique originating `form_response_id` is a different constraint. Multiple signed leaves therefore require an explicit competing-head rule or durable supersession selection before promising a unique current head.

Separate candidate review/sign transitions from consumption of already-signed records: a universal current-signed-head gate would prevent first signatures and corrective drafts. Billing readiness needs its own minimized eligibility projection, not the clinician-only knowledge resolver or expanded access to narrative. Historical signed records/snapshots remain readable independently of live template activation; obsolete knowledge projections are actually deleted by synchronization, while original records and saved drafting evidence remain. Accepted drafts are unsigned editable notes; amendment requirements begin after signing. These errors are rejected proposals, not changes applied to Nexus.


## Final core-review corrections: financial contract

The final council's replacement financial contract is rejected in its current form. Use a consistent positive-reduction example: 20,000 charge = 8,000 contractual reduction + 12,000 allowed; 12,000 allowed = 9,600 payer responsibility + 2,400 patient responsibility. Payment settles obligations and is not interchangeable with responsibility. The report's `allowed = paid + contractual adjustment + patient responsibility` equation is wrong.

An allocation reversal does not move cash. Adding a further 3,600 receipt to 12,000 of receipts raises cash to 15,600, rather than leaving cash unchanged; the example would leave unapplied funds if obligations are already settled. Current Nexus reversals reverse an entire payment once, and current payments require a claim, include noncash adjustments and cannot exceed its balance. They do not already implement unapplied deposits, partial allocation reversal, negative refunds or independent adjudication. New receipt/allocation/refund records require an atomic reconciled reader transition, not an optional view hiding incomplete cutover.

Seal immutable receipt facts separately from later allocation events; a blanket prohibition on allocating a sealed receipt would prevent later application of unapplied cash. A header/lines journal is not inherently invalid: require nonempty/balanced posting and atomic sealing. The proposed thin FinancialEvent wrapper does not solve those invariants by renaming them. Existing verification does not validate this proposed financial model. All figures here are synthetic examples, not payer policy.


## Final core-review corrections: invitation and revocation concurrency

Rollback restores an invitation's consumption only when its acceptance transaction fails. A successfully committed invitation remains consumed: reject replay or provide narrowly identity-bound idempotent response recovery. Losing the HTTP response must not reopen the token.

Define a shared lock/linearization point for proxy revocation and release. A revocation committed before the release acquires and rechecks authority denies the release; a release committed first is not retroactively undone. These are contracts for the proposed portal, not verified current portal behavior.

The complete staff cutover also includes the direct actor-header parser in `access::lock_mutation`, anonymous bootstrap directory and first-admin `/api/setup` path, frontend operator-storage lifecycle and CORS configuration. Preserve existing hashed bearer visitor authorization; it is a separate capability flow, not absent identity code. A cookie-only bootstrap does not close the service boundary. Local proxy development can proceed in parallel, but public enablement requires complete staff identity enforcement and evidence-backed per-action proxy authority.


## Final core-review corrections: longitudinal clinical records

Do not block clinical correction because a problem version was used by a claim, force void/replacement, or automatically repoint that claim. Preserve exact historical diagnosis evidence and separately review future use and financial corrections. Confirmation is not universal payer eligibility; any prototype restriction must be labeled as an operational rule.

A useful first slice remains problems plus allergy assertions/completeness, followed by typed observations and referral follow-through. Its schema still needs explicit logical-record keys, patient-scoped amendment constraints, one authoritative confirmation/verification transition, terminology system **and version**, and a separate completeness assertion contract. Unknown allergy completeness may coexist with positive allergies; an unqualified current no-known-allergy assertion may not. Existing form answers already have snapshot-based type/option validation. The final council's two-table/index proposal does not resolve these requirements and was not applied.


## Final core-review corrections: admission and worker ownership

Admission activation, event/evidence and audit share one transaction with a single final commit. A propagated failure rolls them back; immutable history does not justify partially committed activation or the council's proposed compensation premise. A transfer cannot reuse program attachment to change an existing episode's non-null program: assignment uniqueness and the immutable episode-program guard require an intentional new-episode/transfer contract.

An advisory-lock probe through `pg_locks` is not itself an atomic publication fence. Immediate supervised respawn is also not proven safe merely because startup recovery is one-time and lock-gated: an old pooled claim may commit `running` after the replacement's recovery snapshot, leaving it unvisited. Preserve the narrow terminal-save repair while treating claim/publication fencing, restart supervision and deterministic crash tests as open work. A council cannot waive those acceptance boundaries because of the November 13 target.

Ordinary post-admission client PATCH already permits demographic corrections subject to version/permission/duplicate/age checks and audit; absence of a richer correction-history UI is not an absence of editing. Program intake/discharge checks currently accept any source-valid signed response rather than resolving amendment heads. Discharge uses current program/template requirements, including explicitly required inactive templates; admission snapshots preserve past decisions but do not freeze discharge requirements.

The first serialized terminal transition wins; organization transaction locks serialize mutations but do not prove worker ownership. Existing owner-loss tests terminate before persistence, not mid-transaction, and key-loss-after-backup is not an executed recovery drill. Transcription job terminal immutability has no draft-job-like database trigger, although current worker/cancel predicates preserve terminal states. Bootstrap reads do not acquire the organization mutation lock. A cache key based only on organization, patient and maximum source version misses authorization and source-set changes.


## Final core-review corrections: assigned tasks and communications

Task visibility and authority differ: an assigned clinician may read a task but cannot mutate it unless the linked patient meets the primary-clinician mutation scope. Treat this as a workflow/policy alignment and negative-test gap; do not widen patient access with a generic assignment exception. Current assignee validation requires active staff even when editing a completed task, and no dedicated task state machine exists.

Overdue display uses the viewer's facility timezone with fallback; tasks have no facility/timezone identity. Intake's `facility_local_today` does not define task deadlines. A stable alert identity must specify source and rule versions, recurrence/reopening and null handling independently of assignee changes; the proposed nonexistent source_key/index could duplicate null-key alerts or collide during offboarding.

Active staff status is not disclosure authorization for messaging. Explicit participants, scoped content and rechecks on reads/actions/escalation/downloads remain engineering requirements. Existing `urgent` task priority is an ordinary priority label; preserving it does not promise clinical triage or emergency response. No proposed urgency ban or scope reduction was applied.


## Final core-review corrections: remote media and final upload

Final-upload grace is additional authorization time, not generation-independent permission or guaranteed delivery after network loss. Upload/finish still compares the participant's **current** connection ID with the track; rejoining under a new generation can invalidate the old upload. Joining installs the supplied connection UUID; issuing a new invitation token is a separate operation. Preserve these boundaries unless a separate capability design changes them.

Preserve both ICE end-marker representations (null candidate and an object with empty candidate string); polite-peer glare handling can legitimately use rollback. Gathered relay candidates alone do not prove relay use: remote acceptance requires selected candidate-pair statistics plus observed bidirectional media on separate networks. Earlier generated-media browser evidence is not current Mac live camera/microphone, hardware or remote-network acceptance.


## Final core-review corrections: attachment storage and holds

Bind size, type, digest and scan attempts to immutable bytes. Without a trusted scanner, keep uploads quarantined and unavailable for ordinary download or OCR. Database ciphertext/metadata/audit may share a transaction; external object storage requires staged immutable bytes, promotion/retry and orphan cleanup outside SQL rollback. Current recordings use an installation key; do not assume future attachment keys must use that design.

Authorize filenames, listing, search, preview, OCR and download by category; changing category requires authority on old and new classifications. Use scoped hold placement/release history, recheck all historical references (not only current heads), and fail closed on incomplete traversal. A backup predating deletion also predates an in-database tombstone: reconcile independently durable later disposition/hold decisions and all object/OCR/search derivatives before serving restored data. That reconciliation is not proven by the existing isolated restore/table-count check.

## Closeout: partial financial grants have per-metric dependencies

A persisted support-role exception reproduced a false balance on a paid claim. The corrected bootstrap exposes outstanding only with claims, payments and reversals; unbilled requires claims; cash and adjustments both require payments and reversals. Null means the necessary records cannot be read. Zero is reserved for a readable population with a real zero. No lookup of denied records is needed to produce this distinction. Billing renders unavailable rather than rebuilding gross cash from visible rows. The [verification log](../../development/verification.md) distinguishes this actual change from proposed dashboard measures. Executive field minimization and bank reconciliation remain separate open work.
