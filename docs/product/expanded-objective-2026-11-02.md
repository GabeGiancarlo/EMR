Build and validate a local Nexus recovery-center EMR, billing, and executive operations prototype with real database persistence, custom integrated UI, onboarding/offboarding, permission policies and audit history, excluding authentication setup; produce extensive cited competitor/compliance research and a staged integration plan. Keep user involved, prepare for Vercel hosting without deploying yet, avoid mock demo data, use ChatGPT and budget-controlled xAI review workers with total xAI spend below $5000.



Updated&#x20;

**Nexus is intended to be a recovery-care operating platform that connects clinical treatment, patient participation, reimbursement, and organizational management around one longitudinal patient record.** Its purpose is to carry information forward: an intake informs a treatment plan; a session produces reviewed documentation; documentation supports reimbursement; outcomes and unresolved work inform the care team and leadership.

I reviewed the GitHub product roadmap, both Figma files—including the detailed specifications inside the DUMP file’s “Prep” section—and the newer local implementation on `nexus/integrated-prototype` at commit `ff556b5`.

The distinction matters: **GitHub’s main branch is older than the local implementation, and the Figma scope is substantially broader than either.** Below:

- **Implemented** means present in the local prototype, not production-certified.
- **Designed** means explicitly represented in Figma or the product plans.
- **Proposed detail** means a technical implementation I am spelling out to make the intended capability concrete.
- **Your requested extension** identifies the deeper recording, AI, and patient knowledge-base workflow you described.

The primary product workspaces are Clinician, Billing, and Executive/Leadership. The wider designs also serve patients, clinical supervisors, admissions/support staff, prescribers, nurses, and authorized caregivers. The current implementation targets adult outpatient, IOP, and PHP services; detox, residential, housing, and parent-facing concepts appear in the broader designs. [GitHub product vision](https://github.com/GabeGiancarlo/EMR/blob/main/docs/product/vision.md), [Figma detailed specifications](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)

1. **Organization, facility, and program configuration**

   **Purpose and users:** Gives administrators the organizational structure within which treatment, staffing, scheduling, and billing operate.

   Nexus creates an organization, its facilities, and staff records. Facilities have their own timezone and active status. Organization identifiers accompany clinical and financial records so a claim, session, or policy cannot be associated with another organization’s records accidentally.

   The broader configuration design includes treatment levels, service codes, and note templates. A complete program configuration would also define available services, admission rules, capacity, and program-specific workflows.

   **Status:** Organization/facility setup and timezone handling are implemented. A comprehensive program configuration system is not. [Local data model](C:/Users/ganeg/source/EMR/docs/architecture/data-model.md)
2. **Role-specific workspaces**

   **Purpose and users:** Gives each employee a work environment organized around their responsibilities.

   The clinician workspace prioritizes scheduled care, unsigned notes, treatment-plan reviews, and patient progress. Billing prioritizes authorization expirations, documentation blockers, claims, denials, and balances. Leadership prioritizes census, financial performance, operational risk, staffing, and outcomes.

   These workspaces should share the same underlying records. For example, signing a note should resolve the corresponding documentation blocker in billing without requiring a second manual entry.

   **Status:** Explicitly designed. Six staff roles exist in the prototype, but separate, complete role-specific experiences remain unfinished. [GitHub role workflows](https://github.com/GabeGiancarlo/EMR/blob/main/docs/product/roles-and-workflows.md)
3. **Identity, permissions, and controlled access**

   **Purpose and users:** Ensures each person sees and changes only the records appropriate to their role and patient relationship.

   The implemented roles are administrator, clinical lead, clinician, billing, executive, and support. The API checks resource-level read/write permissions and organization scope. Clinicians are additionally restricted to primary-assigned patients. Billing and executive users do not receive clinical note, treatment-plan, or form-response narratives by default.

   Temporary exceptions have a resource, access mode, reason, expiry, and revocation state. They cannot override signed-record immutability or bypass clinician patient-assignment restrictions.

   **Status:** Permission policies are implemented. Actual identity verification remains deliberately deferred: the local `x-nexus-actor` header selects an operator; it is not secure authentication. [Access-control implementation](C:/Users/ganeg/source/EMR/api/src/access.rs)
4. **Search, navigation, and personal interface settings**

   **Purpose and users:** Helps staff find the correct patient or work item quickly and maintain context while moving between tasks.

   Figma specifies search across clients, sessions, and housing; role-aware navigation; patient-chart shortcuts; configurable quick actions; and status information such as being in a session.

   A complete search implementation would return permission-filtered results with identifiers, relevant dates, and record type—not expose inaccessible information through search snippets. Search results should open the relevant chart, appointment, claim, or task directly.

   **Status:** Basic searching/filtering, responsive navigation, light/dark/system themes, refresh feedback, and workspace exit exist locally. Unified cross-domain search and configurable quick-action ordering remain designed capabilities.
5. **Patient registration and intake**

   **Purpose and users:** Enables admissions staff and clinicians to create a reliable patient identity before starting care.

   Current records include legal and preferred names, date of birth, contact information, emergency contact, facility, primary clinician, and lifecycle status: inquiry, intake, active, or discharged.

   Adult eligibility is evaluated using the facility’s calendar date, including on relevant status changes and episode creation. This prevents a browser timezone or a later record edit from bypassing the program’s age rule.

   The complete workflow also needs duplicate detection, referral information, identity confirmation, intake documents, and disposition tracking.

   **Status:** Core registration and adult eligibility are implemented; the full intake journey remains incomplete. [API contract](C:/Users/ganeg/source/EMR/docs/architecture/api-contract.md)
6. **Longitudinal patient chart and facesheet**

   **Purpose and users:** Gives authorized staff a coherent patient history across multiple visits and courses of treatment.

   The patient is a persistent identity; an episode represents a particular course of care. Sessions, notes, plans, forms, consents, coverage, and claims attach to that identity through explicit relationships.

   Figma’s intended facesheet adds MRN, pronouns, preferred language, allergies, legal/privacy flags, diagnoses, medications, latest test results, and a timeline of admissions, reviews, medication changes, and claims.

   The chart should distinguish current facts from historical facts and show where information originated.

   **Status:** A linked staff chart is implemented. The fuller medical facesheet and integrated event timeline are designed. [Facesheet specification](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)
7. **Episodes of care, transfers, and readmission**

   **Purpose and users:** Preserves continuity when a patient changes programs, returns to treatment, or moves between care levels.

   An episode stores patient, facility, level of care, dates, status, and discharge reason. A returning patient should receive a new episode under the existing identity rather than a disconnected duplicate chart.

   Figma shows a broader pipeline through intake, detox, residential/sober living, outpatient/therapy, and relapse/readmission. The proposed technical model for transfers adds effective-dated care-level intervals, receiving-team assignments, and handoff tasks while preserving the previous treatment context.

   **Status:** Episodes exist. Full transfer, referral, and readmission coordination is not implemented. [Billing pipeline design](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=10-2)
8. **Discharge and continuing-care coordination**

   **Purpose and users:** Helps clinicians and case managers close an episode while maintaining recovery support and unfinished administrative work.

   Current discharge records the end date and reason, completes active plans, and cancels scheduled activity after the episode’s end date. The patient becomes discharged only when no other active episode remains. Financial balances and historical documentation remain intact.

   The intended extended service should also produce a reviewed discharge summary, aftercare plan, referrals, follow-up tasks, and patient resources. Those are separate deliverables that need completion states and accountable owners.

   **Status:** Core discharge transitions are implemented. Comprehensive aftercare coordination is proposed detail consistent with the recovery-continuity plan. [Data model and lifecycle](C:/Users/ganeg/source/EMR/docs/architecture/data-model.md)
9. **Housing and residential capacity**

   **Purpose and users:** Supports residential operators, house managers, admissions staff, and leadership.

   The designs show housing occupancy, beds, residential/sober-living stages, and bed reservations. Technically, this needs facilities, rooms/beds, availability intervals, reservations, admissions, transfers, and discharges. Concurrent assignments must not allocate the same bed twice.

   Occupancy should be derived from effective assignments and available capacity for a defined date—not from a manually entered dashboard percentage.

   **Status:** Historical/broader design scope. A bed-management system is not implemented, and its presence in Figma does not change the current adult outpatient/IOP/PHP delivery scope. [Owner design](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-3817)
10. **Scheduling and calendar management**

    **Purpose and users:** Coordinates patient appointments, clinician availability, groups, and operational responsibilities.

    Sessions store patient, episode, clinician, start/end timestamps, service type, location, and status. Figma calls for day/week/month views, service-type colors, event details, chart links, and telehealth entry.

    A complete scheduler must handle rescheduling, cancellations, no-shows, clinician and patient conflicts, location capacity, recurrence exceptions, and facility-local display. A recurring appointment series should create individually accountable occurrences.

    **Status:** Persisted sessions, an agenda/date strip, and group scheduling exist. The full calendar, conflict engine, and availability workflow are unfinished. [Clinician calendar design](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=58-1743)
11. **Group treatment and individual attendance**

    **Purpose and users:** Allows a facilitator to manage one group encounter while retaining separate clinical and billing records for every participant.

    Groups have a facilitator, facility, category, schedule, capacity, and location. Enrollment creates an attendance record and a linked individual session for each patient. Attendance states synchronize with that session.

    Capacity checks are transactional; duplicate enrollment is prevented. Completion requires resolved attendance, and signed or billed attendance cannot be casually rewritten.

    This structure allows one shared group to produce individual notes, authorization use, and claims without treating every participant’s record as a copy of the entire group.

    **Status:** Implemented locally. [Group workflow contract](C:/Users/ganeg/source/EMR/docs/architecture/api-contract.md)
12. **Telehealth session creation and participation**

    **Purpose and users:** Lets patients and clinicians conduct treatment remotely from the scheduled encounter.

    The intended function is to create a session-associated video room, issue participant-specific access, admit the correct people, and record participation events against the encounter. Figma also specifies screen sharing and a whiteboard.

    **Proposed technical detail:** An encounter-linked room would use expiring join credentials, authenticated clinician admission controls, device checks, reconnect handling, and participant join/leave events. A WebRTC implementation would need signaling and media connectivity services, with explicit choices about media routing and recording.

    The invitation should not expose patient information in its URL. Ending a call should not automatically certify that treatment or documentation is complete.

    **Status:** Designed; no working clinician/patient video-call implementation exists. [Telehealth specification](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)
13. **Recording consent and session-media capture**

    **Purpose and users:** Supports authorized documentation assistance while preserving participant choice and confidentiality.

    Figma describes saving telehealth recordings to the chart. A complete implementation must connect each recording to its encounter, participants, consent evidence, capture times, storage location, and retention policy.

    **Proposed technical detail:** Recording needs a visible indicator, pause/stop controls, refusal handling, restricted playback, encrypted object storage, and audited access. Group sessions require participant-level handling; one patient’s consent cannot automatically authorize recording everyone.

    Media capture, transcription processing, and later disclosure should be separately governed actions. “Auto-save” should only occur when the applicable workflow permits recording and storage.

    **Status:** Consent records exist. Live recording and durable session-media storage do not.
14. **Speech-to-text transcription**

    **Purpose and users:** Converts speech into editable text to reduce clinician typing.

    The current endpoint, `POST /api/speech/transcribe`, accepts uploaded WAV audio and invokes a local `whisper.cpp` executable. It requires PCM16 mono audio at 16 kHz, with limits of 20 MiB and ten minutes. One job runs at a time in the API process, with a 120-second execution timeout.

    It checks the operator, patient assignment, and active same-patient recording consent, then revalidates authorization before returning the result. The response identifies the engine and requires manual review.

    **Status:** Implemented for uploaded files. Live streaming, speaker separation, and clinical transcription accuracy evaluation remain unfinished. Temporary files are normally cleaned up; audio is not automatically retained as a chart recording. [Speech implementation and limits](C:/Users/ganeg/source/EMR/docs/setup/local-speech.md)
15. **Transcript review and speaker attribution**

    **Purpose and users:** Helps clinicians distinguish reliable session evidence from recognition mistakes.

    **Proposed technical detail:** The full workflow should preserve timestamped transcript segments, corrections, source-media references, and speaker assignments. Uncertain speech should be marked for review. A speaker label should not become a patient identity until verified.

    Negation, medication names, doses, dates, and safety statements need particular attention because an apparently small transcription error can reverse meaning. Group speech must not automatically place one participant’s disclosures in another participant’s chart.

    **Status:** Exact text excerpts can currently be reviewed and mapped into forms. Timestamped transcript editing and verified speaker attribution are not implemented.
16. **AI-assisted clinical note generation**

    **Purpose and users:** Turns reviewed session evidence into a structured draft that a clinician can evaluate and sign.

    **Your requested extension:** A generation service would take the reviewed transcript, selected note template, relevant treatment goals, and permitted patient context; propose a draft; and link its statements to supporting evidence.

    **Proposed technical detail:** Each generation should retain input record/version references, model identifier, template version, generation time, and reviewer decisions. Unsupported conclusions should remain absent or explicitly unresolved. Generation must not silently establish a diagnosis, medication change, completed intervention, or signature.

    **Status:** The prototype currently produces a deterministic draft note from signed form labels and answers. That is implemented document assembly, not an AI clinical summarizer. [Form-to-note workflow](C:/Users/ganeg/source/EMR/docs/architecture/api-contract.md)
17. **Patient-specific longitudinal knowledge base**

    **Purpose and users:** Helps authorized clinicians understand the patient’s recovery history across sessions, episodes, and providers.

    **Your requested extension:** Nexus would maintain searchable, source-linked knowledge from approved notes, treatment plans, reviewed transcripts, assessments, and follow-up records.

    **Proposed technical detail:** Canonical clinical records remain the authoritative source. A secondary search index could contain text chunks and optional embeddings tagged with organization, patient, episode, source/version, date, confidentiality classification, and access policy.

    Retrieval must enforce authorization before returning content. Answers should identify their source records and distinguish historical observations from current conditions. Amended or superseded records require corresponding index updates.

    **Status:** Linked longitudinal records exist. A patient-specific semantic knowledge base or retrieval-augmented assistant does not.
18. **Clinical note authoring**

    **Purpose and users:** Allows clinicians to document the actual service delivered.

    Notes currently link to the patient and optionally a session/template, with title, body, author, status, and amendment history.

    Figma expands this into SOAP/DAP/group/discharge template selection, rich text, smart phrases, diagnosis selection, intervention libraries, and session-duration feedback. These features should populate drafts while preserving clinician control over factual statements.

    Clinical duration and billable duration must remain distinguishable; a visible timer alone cannot establish a billable service.

    **Status:** Drafting, session linkage, signing, and amendment exist. The rich editor, smart phrases, intervention library, and complete timer workflow are designed. [Clinical-documentation specification](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)
19. **Signing, co-signing, amendments, and document history**

    **Purpose and users:** Makes clinical authorship and subsequent corrections traceable.

    Signed notes are immutable in the API and database. Corrections create a new linked amendment with a reason; the original remains available. The current local signing action records a workflow attestation.

    Figma additionally calls for pending-signature queues, co-signing, and supervisor involvement. A full implementation needs separate author and supervisor attestations, timestamps, and the precise document version each person signed.

    **Status:** Signing and linked amendments are implemented. Verified electronic identity and complete co-signature flows are not. The Figma “break-glass unlock” concept is not implemented; current code preserves signed originals. [Data model](C:/Users/ganeg/source/EMR/docs/architecture/data-model.md)
20. **Treatment plans, goals, and reviews**

    **Purpose and users:** Converts the patient’s needs into measurable, reviewable treatment work.

    Plans contain presenting needs, strengths, goals, and review dates. Goals define a description, measure, baseline, target, and optional target date.

    Activation signs a plan with at least one goal. Reviews append observations against every goal, using statuses such as on track, at risk, achieved, or discontinued. Revision copies the plan into a new draft and preserves the prior signed version.

    The broader design adds problems/objectives grids, participant signatures, intervention selection, and advance review reminders.

    **Status:** Core plans, measurable goals, reviews, and version-preserving revision are implemented.
21. **Form-template authoring**

    **Purpose and users:** Lets clinical leadership configure the organization’s intake, progress, assessment, discharge, and consent documents.

    Templates contain named fields, labels, types, required flags, and selectable options. Supported response types include text, long text, number, date, checkbox, and select.

    A response snapshots the template used at creation, so later template edits do not retroactively change a completed document’s questions or meaning.

    **Status:** Implemented. Multilingual form presentation, sophisticated conditional branching, and the full consent-signature builder are broader design requirements. [Structured-form contract](C:/Users/ganeg/source/EMR/docs/architecture/api-contract.md)
22. **Form applicability and completion tracking**

    **Purpose and users:** Shows clinicians which organization-defined forms apply to the patient’s present stage and care level.

    Rules match active templates to the patient’s stage and relevant episode. Each match is marked required or suggested and needed, draft, or signed.

    Completion is scoped to the same patient, template, and relevant episode, preventing a signed form from a previous admission from automatically satisfying a new episode’s requirement.

    **Status:** Implemented. These are organization-authored rules; Nexus does not infer legal requirements, and the current rules do not automatically block admission or discharge.
23. **Structured responses and evidence provenance**

    **Purpose and users:** Makes form answers reviewable and traceable to their source.

    Responses preserve the template snapshot, answers, author, source type, source text, and field-specific excerpts. An excerpt must appear verbatim in the stored source text.

    Drafts may be incomplete. Review and signing enforce required fields. Editing a reviewed response returns it to draft; signed responses require amendment. A signed response can generate a linked draft note.

    **Status:** Implemented. Current field mapping is human-directed, not automatic AI extraction.
24. **Attachments, scanned documents, and OCR**

    **Purpose and users:** Brings outside records, uploaded forms, images, and scanned paperwork into the patient’s chart.

    Figma specifies file/audio/image attachments and OCR-assisted ingestion.

    **Proposed technical detail:** Each file needs patient/episode association, uploader, content type, size, checksum, storage key, access classification, and provenance. Upload validation, malware scanning, and controlled download access belong to the ingestion path. OCR output should be stored as a derivative linked to the original, with review before becoming structured clinical data.

    **Status:** Designed; a complete attachment-storage or OCR pipeline is not implemented. [Attachment specification](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)
25. **Diagnoses and clinical summary**

    **Purpose and users:** Gives clinicians and billers a structured account of the conditions addressed in treatment.

    Figma specifies a searchable diagnosis picker, clinical snapshot, and diagnosis linkage to claims.

    **Proposed technical detail:** Diagnoses need code system/version, status, effective dates, author, and patient/episode association. Encounter-specific diagnoses should remain distinguishable from the longitudinal problem list. Suggested codes require clinician confirmation before downstream use.

    **Status:** Claims can store a diagnosis code. A complete coded diagnosis catalog, problem list, and clinical-summary module are not implemented.
26. **Medication reconciliation and prescribing**

    **Purpose and users:** Supports prescribers, nurses, and clinicians who need an accurate medication history.

    Figma specifies medication name, dose, route, frequency, start/end dates, prescriber, PRN/controlled/high-risk flags, drug search, prescription instructions, interaction checks, and e-prescribing.

    **Proposed technical detail:** Reconciliation must distinguish patient-reported medications, verified active orders, discontinued items, and uncertain entries. Prescribing needs an approved external integration, attributable orders, transmission status, and failure handling.

    **Status:** Designed only. FDB and Surescripts appear as possible dependencies in the design text; their presence is not evidence of a selected or functioning integration.
27. **Medication administration, refills, and pharmacy coordination**

    **Purpose and users:** Helps nursing and prescribing staff track administration and resolve refill requests.

    The designed MAR timeline records administration events and late/missed doses. The refill queue supports approval/denial and pharmacy/patient notification.

    **Proposed technical detail:** Administration events need order reference, intended and actual time, dose, route, administering staff, and exception reason. A refill request and the resulting prescription are separate records; an approved request should not be shown as successfully transmitted without external confirmation.

    **Status:** Designed, not implemented. [Medication specification](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)
28. **Laboratory orders, specimens, and results**

    **Purpose and users:** Supports clinicians and collection staff ordering and reviewing tests.

    Figma specifies test selection, priority, collection site, diagnosis linkage, and progression from ordered to collected, received, and verified. Results include values, units, reference ranges, critical flags, trends, review, and patient release.

    **Proposed technical detail:** Orders, specimens, result versions, and acknowledgments must be distinct. Corrected results need revision history. Critical notifications need an owner and acknowledgment timestamp.

    **Status:** Designed. There is no active laboratory-ordering or result-interface implementation. [Lab specification](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)
29. **Vitals, nursing records, imaging, and medical referrals**

    **Purpose and users:** Expands the chart beyond psychotherapy documentation where the program provides or coordinates medical care.

    The clinician design explicitly includes vital signs, nursing records, blood-related information, imaging examinations, medical notes, and referrals.

    **Proposed technical detail:** Observations need measurement time, units, method/source, author, and encounter context. Referrals need destination, reason, status, supporting documents, and returned results. Imaging reports and image files require separate representation.

    **Status:** These are named design modules, not implemented end-to-end workflows. Their detailed technical schemas remain to be specified. [Clinician medical modules](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=56-1561)
30. **Tasks, alerts, and accountable follow-through**

    **Purpose and users:** Turns unresolved work into assigned actions for clinical, billing, administrative, and leadership teams.

    Tasks currently include optional patient association, assignee, title, description, due date, priority, and status.

    The designs add urgency categories, overdue views, bulk actions, reassignment, authorization alerts, missing signatures, and discharge-review work. A complete automated alert should link to its triggering record, explain the condition, name the responsible role, and resolve when the underlying issue is corrected.

    **Status:** Manual task management is implemented. The comprehensive event/rule-driven alert and escalation system is not.
31. **Secure messaging and team collaboration**

    **Purpose and users:** Allows staff and patients to coordinate without losing the connection between communications and care.

    Figma specifies unread/starred inboxes, patient-linked threads, attachments, mentions, clinical/billing/admin channels, searchable history, and co-sign requests.

    **Proposed technical detail:** Messages need recipient authorization, delivery/read state, attachment access, and explicit handling when added to the clinical chart. A billing channel should not inherit unrestricted access to psychotherapy narratives.

    **Status:** Designed; a functioning messaging backend and portal mailbox are not implemented. [Communication specification](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5081)
32. **Patient portal**

    **Purpose and users:** Gives patients a direct way to participate in care between appointments.

    The designed portal includes upcoming appointments, confirmation/change requests, provider messaging, document uploads, consent forms, visit summaries, released lab results, refill information, account balances, payments, recovery resources, profile settings, and MFA configuration.

    **Proposed technical detail:** Portal access must map a verified identity to the correct patient and expose only records explicitly eligible for release. Appointment changes should be requests until accepted by scheduling rules or staff.

    **Status:** Designed. The current patient chart is staff-facing; no complete patient portal exists.
33. **Caregiver participation and recovery education**

    **Purpose and users:** Supports authorized family/caregiver involvement and patient learning outside sessions.

    Figma includes parent progress tracking, behavioral updates, consent-form messages, and a recovery-resource library of PDFs/videos.

    For an adult-first product, caregiver access would need explicit patient-linked authorization and a defined information scope. It must not assume that a family relationship grants chart access. Educational assignments could link to a treatment goal and record patient acknowledgment without treating a view as proof of clinical improvement.

    **Status:** Caregiver/parent concepts and resources are designed. Adult proxy access and resource-assignment workflows are not implemented. [Caregiver design](https://www.figma.com/design/CbcAzJxtlVPyNyIHbtYpmQ/EMR-DUMP-FILE?node-id=1-5977)
34. **Insurance policies and eligibility**

    **Purpose and users:** Helps admissions and billing determine which coverage may apply to a service.

    Current policies store payer, member/group identifiers, effective/termination dates, and status. Claims reference a patient’s policy, and readiness evaluates coverage against the service date.

    Figma additionally specifies primary/secondary coverage and real-time eligibility checks.

    **Proposed technical detail:** Eligibility results should preserve request time, requested service date, payer response, normalized benefits, and unresolved exceptions. A manually active policy must not be displayed as electronically verified coverage.

    **Status:** Policy records and date checks are implemented; live eligibility exchange is not. [Billing design](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=10-2)
35. **Authorization lifecycle and service-unit tracking**

    **Purpose and users:** Helps billing and utilization staff manage approved services and identify impending exhaustion or expiration.

    Authorizations store patient, policy, authorization number, service code, approved units, dates, and status. Claim readiness checks applicable authorization and remaining capacity.

    Units are reserved transactionally when claims become ready. Submitted, denied, and paid claims continue to consume their units; denial does not undo delivered care. Returning an unsubmitted claim to draft releases its reservation.

    **Status:** Core authorization records and reservations are implemented. Renewal requests, payer responses, and the designed expiration queue remain incomplete.
36. **Charge capture and claim preparation**

    **Purpose and users:** Converts documented services into financial records ready for billing review.

    A claim currently references patient, policy, completed session, optional authorization, service code, diagnosis code, units, and integer-cent amount.

    Figma’s fuller claim model includes multiple service lines, modifiers, diagnosis pointers, supporting documentation, and claim details.

    **Proposed technical detail:** Charges and claim versions should preserve the relationship to the original service so corrections or resubmissions do not create duplicate receivables.

    **Status:** A one-service-line claim model is implemented. Full multiline claim preparation is designed.
37. **Explainable billing readiness**

    **Purpose and users:** Tells clinicians and billers exactly why a service is or is not ready to bill.

    Readiness evaluates completed service, signed documentation, policy dates, codes, and selected authorization. The returned checklist includes a code, title, pass/fail result, explanation, and owner role.

    The displayed checklist and the transition to ready use the same backend evaluation, preventing the interface from showing approval while the server uses different rules.

    **Status:** Implemented. “Ready” means internal checks passed; it does not mean transmitted, accepted, or payable. [Readiness contract](C:/Users/ganeg/source/EMR/docs/architecture/api-contract.md)
38. **Claims transmission and status tracking**

    **Purpose and users:** Sends claims to an external recipient and tracks what actually happened afterward.

    Figma specifies batch submission, 837P export, clearinghouse transmission, and claim status timelines.

    **Proposed technical detail:** Submission needs distinct attempt records, stable identifiers, duplicate protection, acknowledgments, rejection handling, and traceable source responses. Transmission, acceptance, adjudication, and payment must be separate states.

    **Status:** External transmission is not implemented. The current `record-submission` action stores a reference for a submission performed outside Nexus; it does not send anything. [Financial export boundaries](C:/Users/ganeg/source/EMR/docs/architecture/exports.md)
39. **Denials, corrections, and appeals**

    **Purpose and users:** Helps billers recover reimbursement and identify recurring causes of rejection or denial.

    Current code can record a denial reason for a submitted claim. The designs add denial queues, appeal deadlines, appeal-letter templates, supporting documents, corrected claims, and resubmission.

    **Proposed technical detail:** An appeal needs its original claim/denial reference, deadline, owner, evidence package, submission proof, and decision. Corrections must preserve original submitted content and external response history.

    **Status:** Denial recording exists; a complete appeals and corrected-claim lifecycle does not. [Denial/appeal design](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=10-2)
40. **Payments, adjustments, balances, and reversals**

    **Purpose and users:** Lets billing staff account for receipts and corrections without destroying financial history.

    Payments use integer cents and identify claim, date, reference, and source: payer, client, or adjustment. Posting prevents overpayment and duplicate references.

    Full reversals append a separate record with reason, date, actor, and original payment link. The original remains unchanged. A reversal can reopen a paid balance, and cash metrics keep actual receipts separate from adjustments.

    **Status:** Implemented locally. Recording a reversal does not issue a bank/card refund. Partial remittance allocation and a complete general ledger are not implemented.
41. **Invoices, statements, patient payments, and remittance reconciliation**

    **Purpose and users:** Supports patient collections and matching external payment explanations to receivables.

    Figma includes invoice creation, statements, payer/patient balance breakdowns, EOB viewing, remittance export, and portal “Pay Now.”

    **Proposed technical detail:** Invoices need issued versions and delivery history. Patient payment processing needs tokenized provider integration and verified payment events. Remittance reconciliation needs line-level allocation, adjustments, unmatched-item queues, and duplicate-file detection.

    **Status:** Designed. Current payment records and CSV downloads do not constitute invoice delivery, card processing, or electronic remittance reconciliation. [Patient billing detail design](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=12-356)
42. **Billing queues and financial performance**

    **Purpose and users:** Helps billing teams prioritize work by urgency, financial exposure, and next action.

    Designs include claims by status/payer/date, pending authorizations, aging balances, denials, appeals, days to payment, collection rate, and denial rate.

    **Proposed technical detail:** Every metric needs a defined population and calculation. For example, “days to payment” must specify whether it measures service-to-payment, submission-to-payment, or another interval. A collection rate needs a declared denominator and adjustment treatment.

    **Status:** Basic financial totals and claim actions exist. The complete queue-driven billing workspace and defined KPI suite remain unfinished.
43. **Executive census, capacity, and workforce visibility**

    **Purpose and users:** Gives leaders an operational view of the organization.

    Figma shows active clients, admissions, occupancy, completions, session attendance, staff utilization, task completion, and program distribution.

    **Proposed technical detail:** Census requires an as-of date and episode rules; capacity requires available slots/beds; utilization requires a defined available-work denominator. Drill-through should expose permitted operational facts without automatically opening clinical narratives.

    **Status:** Basic counts and staff/task records exist. Capacity, utilization, historical trends, and the complete executive workspace are designed. [Executive design](https://www.figma.com/design/iUrPfmFZq0F29oWuDZFF35/Demo?node-id=30-615)
44. **Executive financial, outcome, and risk reporting**

    **Purpose and users:** Helps leadership identify where intervention is needed.

    Designs show revenue, profit/margin, outcomes, compliance, efficiency, recovery milestones, and an actionable queue.

    **Proposed technical detail:** Profit needs actual expense/accounting data; outcome rates need defined measures and cohorts; documentation compliance needs explicit rules and due dates. A dashboard’s illustrated “compliance score” or “safety score” is not a validated formula.

    Each signal should identify its underlying records, date window, owner, and allowed action.

    **Status:** Some source data exists. Comprehensive outcome models, risk calculations, profit reporting, and validated compliance scores are not implemented.
45. **Report building, exports, and scheduled distribution**

    **Purpose and users:** Supports analysis by billing, clinical leadership, administrators, and executives.

    Figma specifies report libraries, date/program/payer/diagnosis filters, saved/shared reports, charts, PDF/CSV/XLSX exports, and scheduled email distribution.

    Current exports provide bounded internal-claim CSVs and aggregate daily accounting CSVs. They enforce roles, organization scope, audit commits, date limits, and spreadsheet-formula protections.

    **Status:** Those CSV exports are implemented. A general report builder, scheduled distribution, and the full format range are not. No scheduled email delivery is implied by the current export endpoints. [Export specification](C:/Users/ganeg/source/EMR/docs/architecture/exports.md)
46. **Staff onboarding, assignment, and offboarding**

    **Purpose and users:** Helps administrators maintain access and continuity when personnel change.

    Staff records include role, facility, active status, name, and email. Current offboarding transfers open tasks, future sessions, and primary patient assignments while preserving historical authorship. It prevents removal of the current operator or last administrator through that workflow.

    The broader plan adds credentials, supervision, training acknowledgments, and effective-dated assignments.

    **Status:** Core staff management and transfer-aware offboarding are implemented. Credential lifecycle and verified identity-provider revocation remain unfinished.
47. **Consent, confidentiality, and disclosure management**

    **Purpose and users:** Records what a patient has authorized and restricts sensitive uses of their information.

    Current consent records have patient, kind, grant/expiry/revocation dates, and scope. Recording consent is actively checked by the speech endpoint.

    The broader design requires consent templates, signatories, multilingual presentation, sensitive-record flags, and controlled disclosure.

    **Proposed technical detail:** A complete policy engine must evaluate recipient, purpose, record category, applicable consent version, expiration, and revocation for each relevant action. Separate psychotherapy/SUD counseling-note handling requires explicit modeling rather than a generic “confidential” flag.

    **Status:** Basic consent records and speech gating exist. Comprehensive disclosure policy and segregated sensitive-note storage do not. [Current confidentiality boundaries](C:/Users/ganeg/source/EMR/docs/architecture/data-model.md)
48. **Audit history and administrative monitoring**

    **Purpose and users:** Lets authorized reviewers determine who accessed or changed information.

    Current audit events record actor, organization, action, resource, record identifier, and time without copying clinical narratives into logs. Audit records are append-only.

    Figma expands this into searchable audit views, user/date filters, export, sensitive-access monitoring, policy repositories, and alert rules.

    **Status:** Core auditing exists. Comprehensive monitoring, richer export context, policy-version management, and rules such as automatic clinical alerts remain designed capabilities.
49. **Record integrity, correction, retention, and deletion**

    **Purpose and users:** Protects clinicians, patients, and financial staff from overwritten or silently altered history.

    Mutable edits require the original `updated_at` value. The server locks the row and rejects stale edits instead of overwriting another user’s changes. Financial and clinical relationships are constrained to the appropriate patient and organization.

    Signed records use amendments; payments use reversals. These mechanisms preserve history.

    **Proposed technical detail:** A complete lifecycle also needs archive states, entered-in-error handling, retention schedules, legal holds, authorized destruction, and removal of derived search/media copies where appropriate.

    **Status:** Concurrency and important immutability controls are implemented. A full retention/deletion program is not.
50. **Interoperability, migration, automation, and operational reliability**

    **Purpose and users:** Connects Nexus to other systems and keeps it recoverable over time.

    The GitHub roadmap includes FHIR import/export, Kipu/QuickBooks migration strategy, background jobs, eligibility checks, report generation, and claim-status polling.

    **Proposed technical detail:** Imports need source identifiers, mapping versions, validation, duplicate handling, rejected-record queues, and reconciliation. Background jobs need retry rules, idempotency, failure visibility, and audit context. Production operations require monitored services, controlled secrets, backup protection, and demonstrated restore procedures.

    **Status:** Rust/Actix, PostgreSQL/SQLx persistence, migrations, custom React/TypeScript UI, local backup/restore tooling, and selected workflow tests exist. The parked `fhir/` and `jobs/` folders are not functioning production integrations. [GitHub roadmap](https://github.com/GabeGiancarlo/EMR/blob/main/README.md), [Local verification record](C:/Users/ganeg/source/EMR/docs/development/verification.md)

For the specific capability you described, the technically precise product statement would be:

> **Nexus is intended to provision an encounter-linked telehealth room with controlled participant access; capture session media only through an authorized recording workflow; process that media through speech recognition; present timestamped, speaker-attributed text for clinician correction; generate source-linked draft documentation against a versioned clinical template; preserve clinician review, signature, and amendment history; and index permitted, approved information into a patient-scoped longitudinal knowledge base that supports later treatment and recovery follow-up. Each stage retains its patient/episode association, provenance, access rules, processing status, and retention treatment.**

**That complete chain is the intended capability, not the current implementation.** Today, the verified local chain is narrower: uploaded WAV → consent/access checks → local speech-to-text → manual excerpt-to-form mapping → reviewed and signed form → deterministic draft note → clinician signing. Video sessions, durable recordings, diarization, AI clinical summarization, and the searchable patient knowledge base remain to be built.