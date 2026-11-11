# Feature reality: implementation, evidence and remaining work

Reviewed November 4, 2026. Based on the substantive Grok `feature-reality-final-report`, current source and dated verification records. The worker inspected source; it did not execute tests. This document separates functioning local software from end-to-end acceptance and production readiness.

## Direct answers

**Data is in PostgreSQL, not SQLite.** The active API uses Actix and SQLx, with 20 migrations and 52 application tables. This Mac runs PostgreSQL 17 on loopback port 55432. Application media ciphertext is also stored in PostgreSQL; media keys and model files are separate local assets. Ordinary setup, synthetic walkthrough and disposable tests use separate databases.

**The models are not Nexus-trained.** Speech uses open-source Whisper base.en through whisper.cpp. Drafting uses Qwen3-4B Q4_K_M through llama.cpp. Nexus owns the surrounding workflow, permission checks, approved-source selection, provenance, review and signing behavior. Self-hosting removes the hosted inference dependency for these paths; it does not remove third-party model/runtime licensing, maintenance or accuracy-evaluation work. Development Grok calls are unrelated to the clinical runtime. [Whisper](https://github.com/openai/whisper), [whisper.cpp](https://github.com/ggml-org/whisper.cpp), [Qwen model](https://huggingface.co/Qwen/Qwen3-4B-GGUF).

**Video is implemented.** Browsers exchange media using WebRTC peer connections; the Rust API manages room, invitation, admission, presence and signaling records. Prior evidence is local synthetic multi-window media, not a validated remote telemedicine service.

**The UI is custom.** React and React DOM are frontend runtime dependencies. There is no shadcn/ui, Radix or premade component framework. Existing custom styling does not satisfy the owner's visual or usability expectations.

## Reality map

| Area | Implemented foundation | Recorded evidence | Remaining acceptance work |
|---|---|---|---|
| Setup and staff workspaces | Organization setup, staff selection, role/resource rules and dedicated role homes | Local setup and selected role journeys | Verified identity, full role journeys, field/facility/care-team/patient/proxy scope |
| Programs and admissions | Versioned programs/services, required forms, capacity and historical decisions, explicit legacy adoption | November 2 integrated checkpoint; focused Mac regression checks | All ledger journeys, configuration UX and remaining scope |
| Client chart and clinical records | MRNs, episodes, referrals, identity history, plans, notes and forms | Synthetic registration-to-admission and signed-document paths | Attachments, coded longitudinal diagnoses, medication/MAR/labs/vitals and richer documentation workflows |
| Scheduling/groups | Calendar views, time-zone handling, conflicts, time away, individual attendance and linked sessions | Dated scheduling scenarios and two reproduced/fixed Mac regressions | Recurrence, series exceptions and every connected workflow; two-way external calendar later |
| Telehealth | Room lifecycle, waiting/admission, participants, media controls, screen sharing and peer signaling | Prior three-participant generated-media test on one machine | Current Mac camera/microphone, mobile/Safari, separate networks, TURN and meaningful group load |
| Recording | Consent/policy snapshots, participant acceptance, own-device tracks, encrypted chunks and playback | Prior capture and restored-track playback using the same retained key | Complete current Mac media chain, recovery after key loss, lifecycle/retention and large-recording operation |
| Speech/transcripts | WAV recognition; saved-media conversion and recognition; reviewed transcript versions/excerpts | Mac synthetic WAV 4.428 seconds; prior saved-media/excerpt flow | Clinical audio accuracy, devices/noise/speakers, end-to-end current Mac saved media acceptance |
| Drafting/knowledge | Approved-source word search, local generation, exact-quote validation and human review | Mac two-source draft in 26.112 seconds with nine cited statements, still awaiting review | Clinical quality/entailment, full current acceptance-to-signing journey and wider scenarios |
| Billing | Policy/auth records, readiness, local status/reference changes, claim-level payment posting/full reversal and CSV | Synthetic readiness/payment/reversal/export paths | Versioned multiline claims, external-response history, appeals, statements, remittance allocation and reconciliation |
| Executive/reporting | Current operational/financial counts and honest missing-data states | Selected local role screens | Agreed definitions, missing financial/outcome/workforce inputs, scoped drilldowns and report builder/export formats |
| Collaboration/access/lifecycle | Audit and selected consent/permission foundations | Dated read/write/export denials and restores | Messaging, patient/caregiver workflows, release controls, retention/holds and derivative-aware destruction |

The [50-capability ledger](../../product/acceptance-ledger.md) remains authoritative: 38 Partial, 11 Missing, 1 Designed at this checkpoint, none accepted complete. This table does not redefine those statuses. Some “missing” features in worker prose are actually partially implemented ledger capabilities; use the ledger rather than a simplified worker label.

## Speech and drafting boundaries

The uploaded-WAV route accepts a constrained audio format and checks clinical access and consent. It runs a configured local executable/model with bounded work and returns text marked for manual review. That response is not a signed chart entry or speaker identification.

The saved-media path is richer: encrypted recording → authorized conversion/recognition job → transcript version → explicit speaker/patient review → approved excerpt → form/note drafting. Current recognition is English and does not automatically identify speakers. The worker source review describes processing limits and cancellation checks; their presence is not accuracy evidence.

Drafting selects approved records, keeps source IDs/versions and model/prompt provenance, checks exact quotations and requires review. Acceptance creates an unsigned note; signing remains separate. Exact quotation validation does not prove that a paraphrase or clinical conclusion is correct. Knowledge search is PostgreSQL word search, not a validated semantic retrieval system.

The later eight-case real local evaluation found that exact quotations can still support an unsupported diagnosis from embedded instructions, wrong-section content, or a mismatched statement citation. Seven structurally valid outputs are not seven semantic passes. See the [verification log](../../development/verification.md). The November 4 Mac tests prove local connectivity and limited synthetic paths. They did not establish a full recorded visit → corrected transcript → accepted draft → signed note → billing-ready packet chain on this platform.

## How video and recording actually work

The current topology is browser peer-to-peer mesh. The API carries signaling and workflow state, not a server-side mixed media stream. Each admitted online participant has peer connections to the others. Mesh has increasing device/network demands as participants increase; no capacity limit is proven by the three-window test.

Default ICE configuration is empty. Optional server configuration exists, but a managed short-lived TURN credential service and verified separate-network behavior remain work. Presence, admission and invitation possession are different from verified identity. Ending a visit does not prove attendance duration or create a billable service. Application signaling controls cannot be described as a production guarantee that a malicious already-connected peer loses its media path.

Recording captures each participating device's own microphone and optional camera, subject to consent and participant acceptance. It does not automatically record every remote stream into one mixed file. Screen sharing is excluded from that camera capture path. A group recording therefore consists of independent tracks and agreement/history records, with incomplete tracks possible after interruption.

Chunks are encrypted with AES-GCM and bound to their record context. Playback is authorized and audited. A PostgreSQL backup contains ciphertext, not the separate key. The prior Windows restore successfully decrypted tracks with the unchanged installation key; that is not a successful key-loss disaster recovery test. Retention review dates do not mean automatic deletion is implemented.

## Local operator and workspace questions

The existing staff/role selector is a local development mechanism. The three workspaces are role experiences, not proof of completed onboarding or multi-tenant account management. A future onboarding flow should grant permissions and choose a useful default experience, while users with several permitted responsibilities can switch appropriately. Do not assume the first outpatient customer needs multiple workspace memberships simply because the demo exposes role switching.

Refresh reloads workspace data; it is not a database reset. The expanded November 4 pass implemented the bottom-left profile menu with appearance controls, permission-filtered settings and a clear local exit to operator selection. Desktop/mobile focus and role switching were checked. Authenticated sign-out still depends on production identity. See the [walkthrough record](../../product/walkthrough-review-2026-11-04.md) for the complete observations and [verification](../../development/verification.md) for current evidence.

## What to demonstrate next

Use a disposable synthetic scenario and an identified integrated revision. For each journey, record actor, source records, allowed and denied actions, persistence after restart, correction behavior and exact remaining external dependency.

1. Outpatient registration through identity/consent, required forms, admission and assignment, including duplicate/capacity failures.
2. Individual and group scheduling through attendance, no-show, reschedule and historical/retired-service behavior.
3. Current Mac call, consented capture, encrypted playback, recognition, corrected transcript/excerpt, reviewed draft and separate signature.
4. Delivered care through required documentation, packet readiness, versioned claim history, manual receiver evidence, payment/adjustment/reversal, statement and reconciliation.
5. Executive drilldowns that match the same operational/financial records without broader clinical disclosure.
6. Remaining ledger modules, patient/caregiver denied paths, record lifecycle and isolated restore with the required keys.

Live payer, payment, calendar and other external connections remain lower priority. Their internal workflows stay in scope. A successful build, an attractive screen, a Grok claim or a prior test count cannot substitute for these acceptance journeys.
