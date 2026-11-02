# Nexus local API contract

Record API responses are JSON; export routes return CSV. Errors are `{error:{code,message}}`. UUID identifiers are strings, money is integer cents, dates are ISO dates, timestamps RFC3339. No example clients are seeded. API listens at `http://127.0.0.1:8090`.

Authentication is deliberately deferred. Local development uses `x-nexus-actor: <staff UUID>` to exercise permission policies; this is NOT authentication. Server refuses to start unless `NEXUS_LOCAL_DEV=1`; bind must be loopback. CORS allows localhost:5173 and 127.0.0.1:5173. Do not deploy this mode publicly.

## Setup and context

- GET `/healthz`: database health.
- GET `/api/bootstrap`: `{setup_required, authentication:"deferred-local-only", actor:null|staff, organizations:[], facilities:[], staff:[], clients:[], episodes:[], sessions:[], notes:[], tasks:[], policies:[], authorizations:[], claims:[], payments:[], templates:[], consents:[], exceptions:[], audit:[], metrics:{active_clients,open_tasks,scheduled_sessions,draft_notes,outstanding_cents}}`. Before selecting an actor only organization/facility and active staff directory are returned. After actor selection arrays are organization scoped and permission filtered. Billing cannot read note body; executive cannot read note body either. Only admin can manage staff and permission exceptions.
- POST `/api/setup`: `{organization_name,facility_name,admin_name,admin_email}` creates first organization/facility/admin atomically. Returns `{organization,facility,actor}`; rejects repeat setup. Save actor.id for request header.

## Records

GET `/api/{resource}` returns array. POST creates and returns record (201). PATCH `/api/{resource}/{id}` merges permitted fields and returns record. All writes audit actor/action/resource/id without duplicating clinical body. IDs, organization_id, created_at, updated_at are server managed. No hard deletion of clinical or financial history.

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
- templates: name, kind (intake|progress|assessment|discharge|consent), description, fields (array of {key,label,type,required}), active. These are user-authored form definitions, not clinical advice.
- consents: client_id, kind, granted_on, expires_on (nullable), revoked_on (nullable), scope. This records an attestation; does not produce legal sufficiency.
- exceptions: staff_id, resource, reason, expires_at, revoked (bool), access_mode (read|write, default read). Admin grant; auditable temporary resource access. Write also permits read. Cannot grant admin/staff/exception management, signed note mutation, or bypass clinician client-assignment scope.

## Workflow actions

POST `/api/notes/{id}/sign` with `{}`: author or clinical_lead/admin; body required, immutable signed content.
POST `/api/notes/{id}/amend` with `{body,reason}`: creates new draft amendment linked by amends_note_id; preserves original signed note.
POST `/api/claims/{id}/ready` with `{}`: validate completed session, signed note for session, active policy/date coverage, required billing codes and authorization coverage if selected. Ready means internally validated, NOT submitted.
POST `/api/claims/{id}/record-submission` with `{external_reference}`: records a submission performed outside Nexus, requires ready state; does not send a claim.
POST `/api/claims/{id}/deny` with `{reason}`: records denial on a submitted claim.
POST `/api/episodes/{id}/discharge` with `{end_date,discharge_reason}`: closes episode and sets client discharged only when no other active episodes remain.
POST `/api/staff/{id}/offboard` with `{transfer_to}`: deactivate and transfer open tasks/future sessions/client assignment atomically; cannot offboard current actor or last admin.

Frontend should hide unavailable actions according to role, display actual server errors, and refresh bootstrap after writes. Empty states should guide record creation, never display invented counts, clients, or outcomes.

## Integrity and reporting

Facility timezone names are validated against PostgreSQL's timezone catalog. Claim coverage and authorization checks use the session's facility calendar date. Future sessions cannot be marked completed or used for signed session notes or billing readiness. Discharge cancellations also use the facility calendar date.

Linked client, episode, session, policy and authorization IDs cannot be reassigned after creation. Signed notes are immutable at both API and database levels. Sessions with signed documentation or advanced claims cannot be rewritten. Coverage referenced by a non-draft claim is preserved; enter a new coverage record for new terms. Payments and audit events are append only. Authorized units are reserved by ready claims and remain consumed for submitted, denied and paid claims; denial does not undo care delivered. Payment reference is unique per claim, preventing duplicate postings; repeated writes return 409 rather than adding another entry.

Metrics include outstanding_cents (submitted/denied/paid balances less posted payments and adjustments), unbilled_cents (draft/ready amounts), cash_received_cents (payer/client receipts), and adjustments_cents. Adjustments do not count as cash. Counts are scoped to the selected actor's visible records. Group-kind session rows are generated by shared group attendance; direct creation of standalone group sessions is rejected.

## Treatment planning and group encounters

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

These are local workflow attestations; verified identity and legal electronic-signature controls remain deferred with authentication. Pasted transcript excerpts are manually supplied. Speech recognition and model-based field extraction are not advertised as running services.

## Billing corrections

`payment_reversals` is a read-only bootstrap/list resource with payment_id, claim_id, copied positive amount_cents, reversed_on, reason, actor_id and timestamps. POST `/api/payments/{id}/reverse` `{reason,reversed_on}` appends a full reversal once. A reason of at least 10 characters and a date from original received_on through today are required. A second reversal returns 409. The original receipt and its source remain untouched. Claim balance and cash/adjustment metrics subtract reversals. A fully paid claim reopens to submitted (or denied if it has a recorded denial) when a reversal makes its balance positive. Cash exports book the negative movement on reversed_on, separately from original received_on.

POST `/api/claims/{id}/return-to-draft` `{}` returns a ready, unsubmitted claim to editable draft and releases its reserved authorization units. POST `/api/claims/{id}/void` `{reason}` marks a draft or ready claim voided; submitted/denied/paid claims cannot use this action. The original claim is retained with void_reason, and a replacement draft may use the same session/service. Voiding an unsubmitted record releases its reservation; it does not erase actual care or manufacture a payer reversal. Claim network correction/resubmission remains outside this local workflow.
