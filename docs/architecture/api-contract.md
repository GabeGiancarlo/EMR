# Nexus local API contract

## Telehealth (local prototype)

Manager routes use the current local `x-nexus-actor` policy. Only admin, clinical lead or the authorized assigned clinician may manage a scheduled encounter. These return JSON with `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.

| Method / route | Contract |
|---|---|
| POST `/api/telehealth/rooms` | `{session_id}` or `{group_id}`, exclusively. Reuses an unended room; expires after four hours. Shared group child sessions are rejected. |
| GET `/api/telehealth/rooms?session_id=…` or `?group_id=…` | Scoped encounter history, including completed encounters. Returns `{rooms}` newest first, bounded to 100. |
| GET `/api/telehealth/rooms/{id}` | `{room,participants,events,invitees}`. No token/hash. Minimal eligible group roster does not grant chart access. |
| POST `/api/telehealth/rooms/{id}/host` | `{}` → one new clinician invitation `{participant,token,expires_at}`. |
| POST `/api/telehealth/rooms/{id}/invite` | `{client_id}` → one invitation for an eligible encounter participant. |
| POST `/api/telehealth/rooms/{id}/participants/{participant}/{admit\|remove}` | `{}`. Admission requires eligible, currently waiting, online participant. |
| POST `/api/telehealth/rooms/{id}/end` | `{}`. Revokes participants and removes signaling; no attendance/documentation change. |

Visitor routes use `Authorization: Bearer <64-hex-character capability>` without a local actor header. GET `/api/telehealth/visit` returns a minimum prejoin snapshot. POST `/api/telehealth/visit/{action}` requires `connection_id` (UUID). Actions:

- `join`, `poll` (optional nonnegative `after` cursor), `leave`.
- `signal`: `{to,to_connection,payload}`; payload contains exactly one bounded WebRTC `description` or `candidate`. Admission/current generation/online host/current eligibility required for both sender and recipient. Delivery expires after two minutes. Poll/join/end perform room cleanup; unattended cleanup remains unfinished.
- `board`: `{stroke:{id,color,points}}` or host-only `{clear:true}`. Four supported ink colors; 1–256 normalized points per stroke, at most 200 strokes. No markup or narrative fields.
- Host-only `admit`/`remove` with `{to}`, and `end`. These recheck staff/encounter eligibility on every request.

Connected snapshots contain room expiry/state, self, permitted participants, current-generation signals/cursor, host presence, permitted board and ICE configuration. Waiting guests see only themselves. Identity/chart IDs, raw credentials, hashes and clinical narratives are excluded. Admission is not identity verification. `visit_busy` (409) rejects another live window; `visit_connection` (409) requires rejoin after a stale generation/presence timeout; `visit_access` (403) stops media on access loss/expiry/end. Long polling interruptions close media and require rejoin. A presence event is not attendance or billable duration. See [evidence and limits](../development/telehealth-checkpoint.md).

## Recording and media (local prototype)

Connected visit snapshots additionally contain `recording:{configured,policies,current}`. Waiting/unconnected participants receive no recording roster or policy list. Hosts receive available organization policies; request participants see the snapshotted purpose, retention review period, mode and agreement decisions without chart-consent identifiers.

| Method / route | Contract |
|---|---|
| GET `/api/recording-policies` | Clinical-role scoped `{configured,policies}` with read audit. |
| POST `/api/recording-policies` | Admin/clinical lead `{title,purpose,retention_days,allow_video}`. Immutable version; no defaults seeded. |
| POST `/api/recording-policies/{id}/retire` | Admin/clinical lead. Ends further capture under that version. |
| POST `/api/telehealth/visit/recordings/{action}` | Visit capability + `connection_id`; actions request, accept, decline, withdraw, start, pause, resume, stop. Request adds `policy_id,mode`. All but own decisions are host-only. Returns recording context. |
| POST `/api/telehealth/visit/recordings/{id}/tracks` | Visit capability + `{connection_id,mime_type}` during authorized recording; returns `{track_id,upload_token,mime_type}`. WebM audio/video whitelist. |
| PUT `/api/recording-tracks/{id}/chunks/{sequence}` | Separate upload bearer, raw bytes. Maximum 8 MiB, ordered from zero, identical duplicates idempotent. First piece must have WebM EBML prefix. |
| POST `/api/recording-tracks/{id}/finish` | Upload bearer + `{expected_chunks,complete}`. Complete requires a nonempty exact stored piece count. |
| GET `/api/telehealth/rooms/{id}/recordings` | Current clinical room scope; `{recordings,tracks}`, no upload hashes/tokens. Audited. |
| GET `/api/telehealth/rooms/{room}/recordings/{recording}/tracks/{track}` | Current clinical room scope; authenticated/decrypted complete WebM with no-store headers and committed playback audit. No range/streaming support yet. |

Upload rights are rechecked against current participant eligibility, generation, chart consent, agreement and policy. Normal pause/end gives 30 seconds for final media; withdrawal and retired policies reject additional bytes. Retained historical playback does not imply continued capture consent. Limits, key setup, recovery evidence and outstanding disclosure/retention work are in the [recording checkpoint](../development/recording-checkpoint.md). Recording does not transcribe, sign, complete attendance or determine billable duration.

Record API responses are JSON; export routes return CSV. Errors are `{error:{code,message}}`. UUID identifiers are strings, money is integer cents, dates are ISO dates, timestamps RFC3339. No example clients are seeded. API listens at `http://127.0.0.1:8090`.

Authentication is deliberately deferred. Local development uses `x-nexus-actor: <staff UUID>` to exercise permission policies; this is NOT authentication. Server refuses to start unless `NEXUS_LOCAL_DEV=1`; bind must be loopback. CORS allows localhost:5173 and 127.0.0.1:5173. Do not deploy this mode publicly.

## Setup and context

- GET `/healthz`: database health.
- GET `/api/bootstrap`: `{setup_required, authentication:"deferred-local-only", actor:null|staff, organizations:[], facilities:[], staff:[], clients:[], episodes:[], sessions:[], notes:[], tasks:[], policies:[], authorizations:[], claims:[], payments:[], templates:[], consents:[], exceptions:[], audit:[], metrics:{active_clients,open_tasks,scheduled_sessions,draft_notes,outstanding_cents}}`. Before selecting an actor only organization/facility and active staff directory are returned. After actor selection arrays are organization scoped and permission filtered. Billing cannot read note body; executive cannot read note body either. Only admin can manage staff and permission exceptions.
- POST `/api/setup`: `{organization_name,facility_name,admin_name,admin_email}` creates first organization/facility/admin atomically. Returns `{organization,facility,actor}`; rejects repeat setup. Save actor.id for request header.

## Records

GET `/api/{resource}` returns array. POST creates and returns record (201). PATCH `/api/{resource}/{id}` merges permitted fields and returns record. Mutable PATCH bodies must include reserved `_expected_updated_at`, the `updated_at` timestamp from the row originally loaded for the edit. It is removed before field cleaning and is not stored. A missing or null token returns 428 `precondition_required`. After the row is locked, a token that does not match `updated_at` returns 409 `conflict` and does not write or audit; the message tells the client to reload the latest version and preserve unsaved edits. Do not fetch a newer `updated_at` immediately before saving. First-party editors pass the loaded row timestamp for form responses, goals, group attendance, modal edits, and inline task changes. POST workflow actions do not require this token; they use locked state checks. All writes audit actor/action/resource/id without duplicating clinical body. IDs, organization_id, created_at, updated_at are server managed. No hard deletion of clinical or financial history.

Resource and editable fields:

- facilities: name, timezone, active
- staff: name, email, role (admin|clinical_lead|clinician|billing|executive|support), facility_id, active
- clients: first_name, last_name, preferred_name, birth_date, email, phone, status (inquiry|intake|active|discharged), facility_id, primary_clinician_id, emergency_contact
- episodes: client_id, facility_id, level_of_care (outpatient|iop|php|residential|detox), start_date, end_date, status (planned|active|discharged), discharge_reason
- sessions: client_id, episode_id, clinician_id, starts_at, ends_at, kind (individual|group|assessment|family|case_management), status (scheduled|completed|cancelled|no_show), location
- notes: client_id, session_id (nullable), template_id (nullable), title, body, status initially draft. Author server assigned. Signed records immutable; amendments append.
- tasks: client_id (nullable), assignee_id (nullable), title, due_date (nullable), priority (low|normal|high|urgent), status (open|in_progress|done), description
- policies: client_id, payer_name, member_id, group_number, effective_date, termination_date (nullable), status (active|inactive)
- authorizations: client_id, policy_id, authorization_number, service_code, units_approved (integer), starts_on, ends_on, status (active|expired|exhausted)
- claims: client_id, policy_id, session_id, authorization_id (nullable), service_code, diagnosis_code, amount_cents, units (positive integer, default 1), status initially draft. External transmission is not implemented.
- payments: claim_id, amount_cents (>0), received_on, reference, source (payer|client|adjustment). Append only; prevent overpayment.
- templates: name, kind (intake|progress|assessment|discharge|consent), description, fields (array of {key,label,type,required}), active, applies_to_stage (any|intake|active|discharged, default any), applies_to_care_level (any|outpatient|iop|php, default any), required_for_stage (boolean, default false). These are user-authored form definitions and organization applicability rules, not clinical or legal advice. Changing a rule does not rewrite form response snapshots.
- consents: client_id, kind, granted_on, expires_on (nullable), revoked_on (nullable), scope. This records an attestation; does not produce legal sufficiency.
- exceptions: staff_id, resource, reason, expires_at, revoked (bool), access_mode (read|write, default read). Admin grant; auditable temporary resource access. Write also permits read. Cannot grant admin/staff/exception management, signed note mutation, or bypass clinician client-assignment scope.

## Workflow actions

Adult-first intake policy: new clients, changes to `birth_date`, and client transitions into `intake` or `active` require 18 completed calendar years on the selected facility's local date. New or newly activated episodes enforce the same client age check. February 29 birthdays use March 1 in a non-leap anniversary year. Violations return 400 `validation`. Existing rows are not rewritten, and unchanged birth dates allow unrelated corrections. This is the scoped prototype's program rule, not jurisdiction-specific legal guidance.

POST `/api/notes/{id}/sign` with `{}`: author or clinical_lead/admin; body required, immutable signed content.
POST `/api/notes/{id}/amend` with `{body,reason}`: creates new draft amendment linked by amends_note_id; preserves original signed note.
GET `/api/claims/{id}/readiness`: claims readers in the same organization receive `{ready, checklist:[{code,title,passed,message,owner_role}]}`. `owner_role` is `clinical` or `billing`. The checklist is the same evaluation used by mark-ready and record-submission. It does not include clinical narrative. `ready` means the rules passed, not that the claim was transmitted.
POST `/api/claims/{id}/ready` with `{}`: mark a draft claim ready only when that evaluation has no blockers. Ready means internally validated, NOT submitted.
POST `/api/claims/{id}/record-submission` with `{external_reference}`: records a submission performed outside Nexus, requires ready state and the same evaluation; does not send a claim.
POST `/api/claims/{id}/deny` with `{reason}`: records denial on a submitted claim.
POST `/api/episodes/{id}/discharge` with `{end_date,discharge_reason}`: closes episode and sets client discharged only when no other active episodes remain.
POST `/api/staff/{id}/offboard` with `{transfer_to}`: deactivate and transfer open tasks/future sessions/client assignment atomically; cannot offboard current actor or last admin.

Frontend should hide unavailable actions according to role, display actual server errors, and refresh bootstrap after writes. Empty states should guide record creation, never display invented counts, clients, or outcomes.

## Integrity and reporting

Facility timezone names are validated against PostgreSQL's timezone catalog. Claim coverage and authorization checks use the session's facility calendar date. Checklist display and claim transitions share that readiness evaluation, including reserved authorization units. Future sessions cannot be marked completed or used for signed session notes or billing readiness. Discharge cancellations also use the facility calendar date.

Linked client, episode, session, policy and authorization IDs cannot be reassigned after creation. Signed notes are immutable at both API and database levels. Sessions with signed documentation or advanced claims cannot be rewritten. Coverage referenced by a non-draft claim is preserved; enter a new coverage record for new terms. Payments and audit events are append only. Authorized units are reserved by ready claims and remain consumed for submitted, denied and paid claims; denial does not undo care delivered. Payment reference is unique per claim, preventing duplicate postings; repeated writes return 409 rather than adding another entry.

Metrics include outstanding_cents (submitted/denied/paid balances less posted payments and adjustments), unbilled_cents (draft/ready amounts), cash_received_cents (payer/client receipts), and adjustments_cents. Adjustments do not count as cash. Counts are scoped to the selected actor's visible records. Group-kind session rows are generated by shared group attendance; direct creation of standalone group sessions is rejected.

## Treatment planning and group encounters

### Scheduling reservations and time away

New and rescheduled individual appointments reserve clinician and client. Group parents reserve the facilitator even before enrollment; attendance-linked sessions reserve each participating client without double-counting the facilitator. Scheduled/completed care reserves its interval; cancelled/no-show care does not. Intervals are half-open, allowing back-to-back appointments. Conflicts return 409 without other clients' details. Restoring excused attendance, group rescheduling and offboarding future assignments are checked transactionally; a conflict rolls back the entire operation. Migration preserves historical overlaps, and title-only edits do not rewrite them.

- GET `/api/scheduling/unavailability`: organization-scoped rows, requires session read access. Reasons are visible to administrators, clinical leads, support and the owning clinician; other permitted viewers receive `Unavailable`.
- POST `/api/scheduling/unavailability`: `{staff_id, starts_at, ends_at, reason, active?}`. Administrators, clinical leads and support manage clinical staff blocks; clinicians manage their own. Requires session write access. Active blocks require active clinical staff and cannot overlap active blocks or reserved care. Reasons contain 1–200 characters; keep personal/clinical details out of this field.
- PATCH `/api/scheduling/unavailability/{id}`: editable `starts_at`, `ends_at`, `reason`, `active`; immutable owner. `_expected_updated_at` is required (428 missing, 409 stale). Set `active:false` to cancel reversibly. Administrators can cancel inactive staff members' blocks; reactivation requires active clinical staff.

Current record/workflow mutations acquire an organization advisory transaction lock before record locks. Operator activity, role and write exceptions are rechecked after waiting; expiry uses wall-clock time. This conservative prototype strategy serializes writes per organization; production load capacity is not established. Direct SQL writes bypass these application checks and are not a supported booking interface. Recurrence, room capacity and calendar availability overlays remain unimplemented.

Browser entry uses the episode/facility timezone and sends offset-bearing timestamps. Nonexistent local times are rejected; repeated DST minutes require an occurrence choice. The API independently validates timestamp intervals.

The following resources use the same GET/POST/PATCH pattern and are included in bootstrap:

- treatment_plans: client_id, episode_id, title, presenting_needs, strengths, next_review_date (nullable). Server sets author_id and status=draft. Signed active plans are immutable. POST `/api/treatment_plans/{id}/activate` with `{}` requires at least one goal; author/clinical lead/admin attests and signs. POST `/api/treatment_plans/{id}/complete` with `{reason}` completes active plan. POST `/api/treatment_plans/{id}/revise` with `{reason}` copies signed plan and goals into a new draft linked by supersedes_plan_id; original remains active until new draft activates and atomically supersedes it.
- goals: plan_id, description, measure, baseline, target, target_date (nullable). Definitions editable only while plan is draft. Server derives client_id/organization_id from plan. Goal outcome/progress is recorded in reviews, preserving the signed goal definition.
- plan_reviews: plan_id, summary, goal_progress (array of {goal_id,status:on_track|at_risk|achieved|discontinued,observation}), next_review_date (nullable). Append only. Server derives client_id and reviewer_id; snapshots signed goal definitions. Requires an active plan and one unique observation for every goal; review observations never overwrite goal definitions.
- groups: facility_id, clinician_id, title, kind (process|psychoeducation|skills|family|support), starts_at, ends_at, capacity (positive integer max100), location. Server sets status=scheduled. POST `/api/groups/{id}/complete` with `{}` requires each participant resolved to attended/no_show/excused and end time passed. POST `/api/groups/{id}/cancel` with `{reason}` cancels a future/scheduled group and linked scheduled attendance sessions. One group is one actual shared encounter.
- group_attendance: group_id, client_id, episode_id, status (scheduled|attended|no_show|excused, default scheduled). Server creates and returns session_id pointing to a matching group-kind session. Unique(group_id,client_id), capacity checked transactionally. PATCH changes status only, synchronizing the linked session to scheduled/completed/no_show/cancelled. Notes and claims use this real session_id. Attended/no_show status requires group start time passed. Signed or billed attendance cannot be rewritten.

Groups expose attendance through separate scoped records. Clinicians see their facilitated groups or groups with their assigned clients, and only assigned-client attendance. Treatment content follows the clinical narrative permission policy; billing/executive/support have no plan/goal/review content by default. Exceptions may grant scoped access explicitly. Planned care is not presented as delivered care.

## Structured forms and provenance

`form_responses` is included in bootstrap and supports GET/POST/PATCH. Create fields: client_id, episode_id (nullable), session_id (nullable), template_id, answers (object keyed by template field key, default {}), source_type (manual|transcript, default manual), source_excerpt (text, default empty), field_provenance (object keyed by answer field key with `{excerpt}` values). The server snapshots the complete template as template_snapshot, sets author_id and status=draft. An excerpt must occur verbatim in source_excerpt; this is traceability, not a claim that transcription or extraction was automated.

PATCH accepts answers, source_type, source_excerpt and field_provenance; an edit after review returns the response to draft and clears the review attestation. Template/client/session/episode references are immutable. Answer types are validated against the snapshot. Field definitions support text, textarea, number, date, checkbox and select (with nonempty string options array). Required answers are enforced on review/sign; partial drafts may be saved.

- POST `/api/form_responses/{id}/review` `{}` validates the answers and records reviewer_id/reviewed_at, status=reviewed.
- POST `/api/form_responses/{id}/sign` `{}` requires reviewed state; author, clinical lead or administrator attests, status=signed. Signed responses are immutable.
- POST `/api/form_responses/{id}/amend` `{reason}` copies the original snapshot and answers into a new draft linked by amends_response_id, preserving original history.
- POST `/api/form_responses/{id}/create-note` `{title}` creates one draft note from a signed form's actual answers and field labels, using its client/session references. The note has form_response_id provenance. It still requires normal clinical review and signing; no clinical content is invented.

These are local workflow attestations; verified identity and legal electronic-signature controls remain deferred with authentication. Source text may be entered, imported, or generated by the optional local whisper.cpp adapter. Form-field extraction remains an explicit human selection. A separate optional local-model workflow now proposes source-linked note statements for explicit review; it does not sign notes or establish clinical truth.

## Organization form applicability

Template applicability is organization-authored. `applies_to_stage` and `applies_to_care_level` default to `any`. `required_for_stage` defaults to false. These fields do not encode statute, payer policy, or clinical standards. Nexus never infers a requirement that an author did not store on an active template. No form content is seeded.

GET `/api/clients/{id}/form-requirements` requires `form_responses` read and the existing client-assignment scope (clinicians only for clients they are assigned as primary clinician; admin and clinical lead remain organization-scoped). It returns `{client_id, client_status, relevant_episode_id, relevant_stage, relevant_care_level, disclaimer, requirements:[{template_id, template_name, disposition: required|suggested, completion: needed|draft|signed, response_status, reason, episode_id, applies_to_stage, applies_to_care_level, required_for_stage}]}`.

`relevant_stage` is the client status. It is not derived from the episode. `relevant_episode_id` is the current active episode, otherwise the latest discharged episode (latest end date, then start date, then creation time, then id). Planned episodes are ignored. `relevant_care_level` is that episode's level of care, or null when none exists. Care-level-specific rules match only `outpatient`, `iop`, or `php`. Residential and detox episodes match only rules set to `any`. Inquiry matches only stage `any`.

A rule matches when the template is active, its stage is `any` or equals the client status, and its care level is `any` or equals the relevant care level. Matching rules with `required_for_stage` true are `required`; other matches are `suggested`. Inactive and non-matching templates are omitted.

`completion=signed` only when a signed form response exists for the same client, template, and relevant episode. When no relevant episode exists, including intake with no episode, the signed row must have `episode_id` null. Draft and reviewed rows for that same key yield `completion=draft` (`response_status` distinguishes them). Otherwise `needed`. A signed row wins over a draft. Reasons restate the organization-authored rule; they do not determine legal or clinical requirements. Editing applicability does not rewrite `form_responses.template_snapshot`.

## Billing corrections

`payment_reversals` is a read-only bootstrap/list resource with payment_id, claim_id, copied positive amount_cents, reversed_on, reason, actor_id and timestamps. POST `/api/payments/{id}/reverse` `{reason,reversed_on}` appends a full reversal once. A reason of at least 10 characters and a date from original received_on through today are required. A second reversal returns 409. The original receipt and its source remain untouched. Claim balance and cash/adjustment metrics subtract reversals. A fully paid claim reopens to submitted (or denied if it has a recorded denial) when a reversal makes its balance positive. Cash exports book the negative movement on reversed_on, separately from original received_on.

POST `/api/claims/{id}/return-to-draft` `{}` returns a ready, unsubmitted claim to editable draft and releases its reserved authorization units. POST `/api/claims/{id}/void` `{reason}` marks a draft or ready claim voided; submitted/denied/paid claims cannot use this action. The original claim is retained with void_reason, and a replacement draft may use the same session/service. Voiding an unsubmitted record releases its reservation; it does not erase actual care or manufacture a payer reversal. Claim network correction/resubmission remains outside this local workflow.

## Optional local transcription

POST `/api/speech/transcribe` accepts a raw `audio/wav` body and `x-nexus-client` / `x-nexus-consent` UUID headers in addition to the local actor header. Only administrators, clinical leads, and primary-assigned clinicians can transcribe. An active same-client recording consent is required using the client's facility calendar date. Resource exceptions cannot extend this role set. Authorization is re-read under locks before processing and in the completion-audit transaction.

Input is PCM16 mono 16 kHz, at most 20 MiB / 10 minutes. One whisper.cpp process runs at a time, with a 120-second execution limit and no hosted fallback. Missing local configuration returns 503, occupied worker 429, invalid WAV 400, and oversized input 413. A successful no-store response is `{transcript, engine: "whisper.cpp", manual_review_required: true}`. Nothing is signed or saved as a form automatically. See [setup, provenance and observed verification](../setup/local-speech.md).

## Recorded-media transcripts and source-bound forms

See [transcript endpoints, review states and provenance](../development/transcript-contract.md) and the [verified checkpoint](../development/transcript-checkpoint.md). Migration 013 adds persistent background recognition, versioned review, and patient-specific immutable excerpts. Authentication remains deliberately deferred.

## Approved patient knowledge and clinical drafting

The [knowledge/drafting contract](../development/knowledge-drafting-contract.md) specifies patient-scoped word search, source detail, index refresh, durable model jobs, cancellation and immutable statement-level review. Migrations 015–016 persist derived documents/chunks and generation/review history. Acceptance creates an unsigned note through the existing clinical workflow. Current-source and patient/role checks run again before publication, acceptance and signing. Clinical-role restrictions are stricter than general resource exceptions. No hosted clinical-model fallback exists. See [observed verification and limits](../development/knowledge-drafting-checkpoint.md).
