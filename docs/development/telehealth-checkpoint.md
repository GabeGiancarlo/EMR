# Encounter-linked telehealth checkpoint — November 2, 2026

This advances capability 12. It does not complete the 50-capability product or the recording → transcription review → source-linked draft → patient knowledge-base chain. Authentication remains deliberately deferred and services remain loopback-only.

## Working behavior

The calendar and group workspace open the same persisted encounter room. A room belongs to an individual session or shared group; group child sessions cannot create separate rooms. Administrators, clinical leads and authorized assigned clinicians prepare access. Group facilitators receive a minimum enrollment roster without gaining primary-assignment chart access.

Participant-specific invitations are hashed in PostgreSQL and expire with the four-hour room. The UI uses a URL fragment, removes it on entry and retains the capability only in that window's session storage. Reissue invalidates the old token. A live invitation cannot join in two windows or be rotated while connected. Waiting guests cannot see other people, signal to them or read the board. Admission is a clinician action.

The visit supports camera/microphone preview, joining without devices, in-call device settings, mute/video controls, screen replacement/camera restoration, media reconnection, leave, clinician removal/end, and a shared whiteboard. Local tracks stop on exit or access loss. Participation events persist separately from attendance, documentation and billing. There is no recording button or silent capture service.

The server rechecks staff activity/role, individual assignment or group facilitation, active episode/enrollment and scheduled encounter state. Organization-before-room locking orders these checks against staff/encounter changes. Signals require current connection generations, admission, host presence, eligible peers and unexpired transport messages.

## Observed evidence

- **46 Rust unit tests**, **seven PostgreSQL/HTTP suites**, **39 frontend tests**, strict Clippy, Rust formatting, TypeScript and Vite build passed. CI includes the telehealth database suite.
- The new HTTP scenario creates a unique schema only in `nexus_test`. It verifies concurrent room reuse/window joins, billing/foreign-organization denial, hashing/rotation, waiting privacy, admission/removal, cursor/generation isolation, whiteboard permissions, host absence, token expiry, offboarding, facilitator reassignment, patient discharge, group roster versus chart scope, immutable events and unchanged scheduled care after call end.
- Peer-engine regressions cover admission/generation cleanup, stale/duplicate signals, ICE buffering, simultaneous offers, replacement tracks/sender reuse, reconnect and local-device ownership. These tests use protocol doubles; browser transport was checked separately.
- Three separate browser windows used the production `TelehealthVisit` with a dedicated development device adapter. Two guests waited for admission, then all three displayed remote moving frames and connected peer state. Purple generated screen frames reached the other windows; stopping sharing restored the camera. Mute/camera-off, reconnection, whiteboard propagation/host clearing, clinician departure/media pause and rejoin were observed.
- A host who joined without devices enabled them through in-call settings and appeared as video in the other windows. Removal stopped the guest's media and displayed invitation unavailability. End stopped the other participants. Database inspection showed no remaining signals, with the group and both attendance rows still scheduled.
- The actual `/visit` entry opened from the staff manager and joined without hardware capture. Calendar and group entry points reopened the same room; the manager showed its connected clinician and subsequently the persisted ended state. No real camera, microphone, screen or patient media was used.
- Light/dark desktop layouts were inspected; a video-grid overflow was corrected. The viewport tool did not apply its requested mobile dimensions in this session, so mobile telehealth acceptance remains unverified. The override was reset.
- Ordinary and existing QA databases were backed up before migration/restart. Migration 011 adds four tables. A new ordinary-database backup restored to `nexus_restore_20261102155246_55ed4c3b` with **29 public tables**. The ordinary API is healthy. Existing client records were not changed by telehealth tests.

Screenshots remain outside Git: `.local/design-review/telehealth-screen-sharing-light.png`, `telehealth-three-participants-whiteboard.png` and `telehealth-calendar-manager.png` in that directory.

## Remaining acceptance

1. Media was verified on one computer using generated video and silent audio. Real capture, audible audio quality, the native screen chooser, separate networks and browser interoperability remain unverified. No hosted telemedicine service is claimed.
2. Default ICE configuration is empty. Optional `NEXUS_ICE_SERVERS_JSON` is bounded engineering configuration, not a TURN credential service. Admitted browsers receive configured ICE credentials. Do not configure a reusable relay administrative/shared secret; provision expiring credentials and validate self-hosted TURN before remote acceptance.
3. Media closes cooperatively on the next poll after access/host loss. Signaling alone cannot forcibly disconnect malicious clients that already established a peer-to-peer connection. Production revocation requires a controlled media gateway/SFU and validation. Three participants do not establish group capacity/load limits.
4. Polling is once per second, requests time out after eight seconds, and presence expires after twenty seconds. Longer interruptions require rejoin. Background throttling and application/database clock skew need broader tests. Organization serialization favors current consistency over throughput; no load claim is made.
5. Signal delivery expires after two minutes; physical cleanup happens on that room's poll/join or end. An unattended cleanup scheduler is still needed. Whiteboard content persists in the room and is not a signed chart note. Retention/destruction/legal holds remain unfinished.
6. Consent-governed recording/storage, timed transcript review, verified speakers, AI clinical drafting and patient-scoped retrieval remain separate unfinished capabilities. Presence cannot establish delivered care or billable duration.
7. Invitations are bearer capabilities, not identity verification. Authentication-provider setup remains deferred. No deployment or GitHub push was performed.

## Grok review disposition

Two development-source-only workers completed: peer-engine proposal **$0.410096**, access/lifecycle review **$0.095084**. No fixtures, capabilities, database exports, patient records, media or secrets were sent. Cumulative known billed usage is **$5.18555**, with **$200** conservatively reserved for earlier requests of unknown cost. Account billing remains authoritative.

The engine proposal was reviewed alongside the working implementation. Incorporated improvements include discarding abandoned ICE on ignored glare, one received track per kind, lifecycle guards, duplicate signal suppression and propagating track replacement failures. Focused regressions and real browser transport checks passed afterward.

The access review identified missing in-call device settings and misleading ended wording on access loss; both were corrected. Its duplicate-room/join and lock claims overlooked organization/room locks and schema uniqueness outside its packet. Concurrent tests explicitly verify these outcomes. Its covering-clinician concern overlooked primary-assignment scope checks; care-team delegation is not added here. TURN and presence-window concerns remain limits above. Model output is not execution evidence.

## Isolated media harness and local review

`frontend/telehealth-qa.html` and `frontend/qa/telehealth.tsx` are a separate development entry, excluded from the production index build. They inject canvas frames and silent audio through normal MediaStreams; they do not stub signaling, admission, database writes or peer transport.

QA used port 5175 → API 8092 → `nexus_telehealth_ui_da9088a3`. Only explicit `NEXUS_TELEHEALTH_QA_FIXTURE` enables the Vite fixture endpoint. That ignored file contains `{name,links:[{label,token}]}` for short-lived test invitations. The harness also checks development mode and port 5175. Never enable this endpoint in the ordinary workspace or deploy it. Test calls ended and their invitations are no longer usable.

The normal workspace remains `http://127.0.0.1:5173/`, backed by user-created records. Review through a scheduled encounter → **Open telehealth** → prepare clinician/client access → open the invitations in separate participant windows. Device capture requires the participant's explicit action. Nothing is recorded.
