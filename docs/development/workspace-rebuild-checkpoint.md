# Workspace rebuild checkpoint — November 2, 2026

This is an incremental implementation record for the [expanded objective](../product/expanded-objective-2026-11-02.md), not acceptance of the complete product. The [50-capability ledger](../product/acceptance-ledger.md) preserves the remaining scope. Reference evidence comes from the [Figma source inspection](../product/figma-source-inspection.md).

## Search and facility-time entry

- `WorkspaceSearch.tsx` / `search.ts`: global search from the current authorized bootstrap, explicit searchable field allowlists and record-specific navigation. It does not search note bodies/transcripts or store query/record copies in browser storage. The server continues enforcing actions. This is an in-memory search over the loaded workspace, not the proposed longitudinal clinical knowledge base or a paginated search service.
- Search results open the working record or filter the relevant queue with a readable selected-record label. Search can be opened by its header button or Ctrl/Cmd+K. Native dialog focus/Escape behavior applies. Navigation and clinical/coverage/audit tabs respect returned read permissions.
- `schedulingTime.ts` / `forms.tsx`: appointment entry uses the episode/facility timezone. DST gaps are rejected; repeated minutes require an explicit occurrence. Existing sub-minute precision is retained on unrelated edits. Changing a client clears dependent episode/session/coverage selections.
- Node tests verify permission denial even when stale arrays contain records, narrative exclusion, Unicode names, multi-term matches, actor exit, facility time conversion, gaps/folds, half-hour shifts, skipped dates, and original timestamp precision. These tests use synthetic in-memory fixtures only.

Browser checks in the existing QA workspace verified finding the existing QA handoff task and opening its task queue, the readable selected-record label, facility timezone guidance, and both UTC-offset choices for a repeated time. The unsaved appointment was cancelled. Existing client/session records were not modified by these checks.

## Role workspaces and calendar

`RoleWorkspace.tsx` / `workspace.ts` provide clinical, billing and executive home screens. Clinical prioritizes today's care, drafts, reviews and tasks; billing shows claims, authorizations and balances; executive shows census, staffing, workload and financial actuals. Workspace selection changes presentation without granting access. Charges use facility-local service dates; cash/adjustments and reversals use their posting periods; balances and census remain current stocks. Missing data/permissions show unavailable/unknown states, including clinical counts without clinical access. Outcomes, profit and compliance scores remain unavailable.

Dashboard links open matching claim/task/staff/client queues. The executive scheduled-client count opens all appointments awaiting attendance, including past unresolved care and one entry per group participant. `CalendarWorkspace.tsx` / `calendar.ts` show one parent event per group in day/week/month/agenda views, with facility/clinician/type/status filters, seven weekdays, 24 hours, event detail, chart/group navigation and a task queue. Mobile scrolling stays inside the calendar. These are custom React/CSS components with no new UI component dependency. Figma informed role separation and calendar/task layout; exact reproduction and complete design acceptance are not claimed.

## Scheduling persistence

Migration 010 adds staff time away. `availability.rs` provides scoped audited create/edit/cancellation with version protection. `scheduling.rs` checks clinician/client overlaps, enrollment, restored attendance and group rescheduling. `access::lock_mutation` serializes organization writes and rechecks active operator/permissions after waiting. Offboarding rolls back when transferred care conflicts with reservations or time away. Existing ordinary and QA client/session records were not rewritten.

`StaffAvailability.tsx` provides persisted blocks, facility-time entry, DST choices, server errors and reversible cancellation. An attendance queue supports follow-through. Calendar availability overlays, rooms, recurrence and recurring-series exceptions remain missing.

## Observed validation

- 43 Rust unit tests and all six PostgreSQL/HTTP suites passed; strict Clippy and formatting passed. The new unique-schema scheduling suite covers concurrent requests, clinician/client conflicts, adjacent appointments, cancelled/no-show release, empty-group reservations, multiple participants, failed enrollment without orphan sessions, group-reschedule rollback, attendance restoration, booking/time-away races, availability scope/redaction/version checks, conflicting/successful offboarding, operator deactivation and exception expiry during lock waits, and organization isolation.
- One older role-change fixture transferred two overlapping appointments to one administrator. Its times were separated; the real conflict rule was preserved.
- 36 frontend tests and TypeScript/Vite build passed, covering search permissions/narrative exclusion, financial periods/reversals/unknown states, census/staffing/queues, adult intake, calendar/DST and existing form/audio validation.
- Browser: search opens the exact QA task; repeated 01:30 offers both offsets; month view deduplicates the group; group detail opens the existing encounter; claim/staff/workload links open matching queues; workspace/theme choices survive reload. Calendar and executive layouts fit 390×844 (content width 375). Dark selected-calendar contrast was inspected and corrected.
- Only in `nexus_ui_test`, created `Synthetic availability verification` for the existing QA operator, verified it after reload, then cancelled reversibly. Its cancelled row and audits remain. No client/session record was changed or deleted; no fixture was added to the ordinary workspace.
- Both API health checks passed after restart. A fresh ordinary backup restored into `nexus_restore_20261102152222_02022818` with 25 public tables; source unchanged. Screenshots are ignored local artifacts at `.local/design-review/nexus-calendar-dark.png` and `nexus-availability-verified.png`.

## Worker usage and remaining scope

Grok's calendar proposal was integrated with local corrections. The role response reached its output limit; only complete file objects were recovered, reviewed and tested, with local integration and tests. The scheduling worker timed out and three smaller retries lost connections, so scheduling was implemented locally. Failed calls remain in the ledger. Known billed usage is $4.68037 with $200 reserved for unknown charges (not asserted actual spending), below the $5,000 ceiling. Workers received development source only.

The 50-capability objective remains incomplete. Controlled encounter-linked telehealth, durable consented recording, timed/speaker-reviewed transcription, model-assisted source-linked drafts, approved patient knowledge retrieval, patient portal, fuller medical records, retention and complete external revenue exchange remain open. Authentication setup is deliberately deferred. No public deployment or GitHub push occurred.
