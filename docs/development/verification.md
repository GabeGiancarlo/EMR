# Local verification record

The latest [knowledge and drafting checkpoint](knowledge-drafting-checkpoint.md) records 86 passing Rust unit checks, 11 PostgreSQL/HTTP scenarios, 61 frontend checks, build/lint/format checks, approved-record word search, actual local-model generation, explicit review, edited/signed note persistence, cancellation and matching clinical-history digests after a 42-table restore. A development hashing optimization reduced the observed synthetic generation from 56.5 to 4.5 seconds. It follows the [transcript checkpoint](transcript-checkpoint.md), [recording checkpoint](recording-checkpoint.md), [telehealth checkpoint](telehealth-checkpoint.md) and [workspace checkpoint](workspace-rebuild-checkpoint.md). Older entries retain their original counts and limits. The [50-capability acceptance ledger](../product/acceptance-ledger.md) remains the scope record; these checkpoints are not full product acceptance.

Verified November 2, 2026 against the local PostgreSQL/API and custom frontend. This records observed behavior, not a production-readiness or compliance certification.

**Product acceptance remains incomplete.** The owner's review exposed gaps not closed by the tests below. The [scope and delivery audit](roadmap-gap-audit.md) maps those gaps to the original roadmap and newly supplied Figma references. Earlier browser checks establish only the paths specifically described here.

## Isolation

- `nexus`: ordinary workspace; no sample clients or demo fixtures seeded.
- `nexus_test`: explicitly disposable backend integration database. Export, form-routing and speech authorization tests each create and remove their own isolated schema inside it.
- `nexus_ui_test`: separate browser QA database originally populated with synthetic fixtures. The owner has since entered additional review records; do not assume all its rows are disposable or reset it. No payer submission or real payment occurs in the recorded QA paths.
- xAI receives development source files only. No clinical content, credentials, database dumps or user records are included.

## Browser path exercised

Using the actual interface, created a client with blank optional fields, an active IOP episode, a completed linked session, a draft note and its signed original, a policy, an authorization, a claim, a successful readiness transition, an explicitly synthetic external reference, and a payment. The resulting claim displayed Paid, cash received $100, outstanding $0. The accounting download contained one aggregate date/source row with 10,000 cash cents and no client identifiers.

The browser test found and resolved two input issues: date controls needed input-event handling, and optional text fields must remain empty strings while unlinked references and absent dates are null. Native Node serialization regression tests now cover this distinction.

Local screenshots are retained under ignored `.local/screenshots` so QA fixtures do not become application content.

Additional browser checks passed for full payment reversal (original retained; net cash returned to zero and balance reopened), authored template creation, exact source-excerpt mapping, reviewed/signed form response, deterministic draft-note creation, signed treatment plan with measurable goal and progress review, group enrollment/attendance/completion with individual session linkage, staff offboarding with open-task transfer, and client discharge with record retention.

Responsive checks at a 390 × 844 viewport verified that the page fits the viewport and wide tables scroll within their containers. Closed mobile navigation was initially still exposed to keyboard and assistive navigation; it is now hidden until opened and supports Escape dismissal.

## Automated validation

Rust unit and real PostgreSQL workflow checks cover tenant/panel access, role exceptions, immutable signed documents, claim readiness, authorization reservations, concurrent payment limits, discharge/offboarding, care plans, group attendance, structured template snapshots and reviewed form answers. CSV route checks cover role denial, tenant isolation, private-field exclusion, quoting/formula protection, date windows and transactional audit failure.

Run the commands in the root README for current results. Browser verification complements those checks; a successful build alone is not acceptance.

Latest expanded checkpoint: 39 Rust unit tests, four real PostgreSQL/HTTP integration scenarios, and eight frontend serializer/WAV validation tests passed. Strict Clippy, Rust formatting, TypeScript and the Vite build passed. Operational JavaScript and PowerShell helpers passed syntax checks. CI now includes the form-routing and speech authorization database scenarios explicitly.

The workflow integration scenario also checks a Pacific-time evening service crossing the UTC date boundary against one-day coverage/authorization and rejects completion of future sessions. Claims readiness and discharge scheduling use the facility calendar date. Additional speech checks use facilities on opposite sides of UTC midnight and verify that concurrent consent revocation and operator deactivation are observed after their locks are released.

## Operational evidence and limits

Expanded Grok implementation work added mandatory loaded-version tokens for PATCH and an explainable billing checklist using the same backend rules as claim transitions. A two-tab browser check confirmed that the first edit persists, the stale second edit is rejected, and its unsaved text remains visible. Database checks confirm that rejected stale writes do not append update audits. The billing browser check identified a missing signed group-session note, cleared that blocker after signing, and persisted internal readiness without payer submission.

Offboarding regressions additionally cover administrators with clinical assignments and concurrent new assignments during deactivation. Clinical work cannot transfer to billing-only staff, and assignment validation waits for the staff lock before checking active status.

Staff role changes reject a move to billing, executive or support while primary clients or future scheduled individual/group encounters remain assigned. Each assignment type is covered independently; rejected changes preserve both the staff version and update-audit count. Reassignment allows the role change. Browser verification also confirms that changing a new note's client clears its title, narrative and source text before another chart can be selected.

A second backup/restore check after the expanded schema restored 24 public tables into `nexus_restore_20261102120013`, leaving the source database unchanged. Backup filenames now include a unique suffix and the helpers preserve a caller's existing database-password environment value.

After migration 009 and the final API restart, the ordinary workspace still returned `setup_required=true` and a healthy database connection. A new hashed backup restored successfully into `nexus_restore_20261102122846_00ab15e6` with 24 public tables. The restore helper now uses a unique destination suffix and checks the inspection command's exit status.

Form-rule browser verification changed an existing authored template from a general suggestion to required for discharged IOP clients. The matching chart displayed the saved rule and its signed completion status, and prior responses remained unchanged. The API tests cover active/discharged episode selection, unrelated signatures, empty matches, inactive templates and tenant/panel access.

The official pinned whisper.cpp CPU build and model checksum passed. A 13.6-second locally generated synthetic recording was transcribed through the actual API and browser. The known verification sentence matched the returned text. The browser then mapped that text into a form answer, saved a draft, and reopened it with the exact source excerpt still linked. Audio remained local. This demonstrates functioning recognition and persistence; it does not validate clinical speech accuracy, live capture or speaker identification.

An actual local backup was hashed and restored into a newly created isolated database; the ordinary workspace was left unchanged. Docker execution on this machine is unavailable because Docker Desktop reports an internal inference-manager socket error. PostgreSQL therefore runs using the official portable EDB distribution. The Docker image is prepared for CI, but a local container runtime check has not passed.

The research HTML report was opened and visually reviewed. The DOCX passed structural validation, but visual rendering could not run because the bundled LibreOffice binary is unavailable on this Windows host; the DOCX is not described as visually verified.

Authentication is intentionally absent. Local actor policies do not establish identity. Public deployment, real patient use, external payer exchange, clinical speech accuracy and production operational controls are outside the evidence above.

## Scope-review corrections, after 981afd0

Verified on November 2 after the owner's review:

- 42 Rust unit tests pass. Five real PostgreSQL/HTTP integration scenarios pass, including the new adult-intake scenario. Strict Clippy and Rust formatting pass. The database scenarios ran only in `nexus_test`; the new intake scenario creates and removes a unique schema.
- Adult intake checks reject minor creation, DOB changes to a minor, client transition into intake, and new/activated episodes for a legacy minor. They accept an eligible adult and preserve the legacy DOB during an unrelated correction, including when the unchanged DOB is submitted. Calendar boundary/leap-year unit checks pass. The rule is scoped product policy, not legal eligibility advice.
- 11 frontend validation tests and the TypeScript/Vite build pass. The three new age tests cover the birthday boundary, leap-day cutoff consistency and differing facility dates at the same instant.
- Browser verification on the existing QA workspace: dark/light/system selection; dark preference persists after reload; refresh produces the visible “Workspace refreshed” confirmation; desktop and mobile Exit workspace return to local operator selection. Dark intake fields and dialog surfaces were visually inspected.
- A new-client form with explicitly synthetic inputs and DOB `2015-01-01` was rejected by the browser date limit; the form retained its inputs and was cancelled without saving. Backend rejection is separately covered by the database scenario.
- At 390 × 844, header theme/refresh/navigation controls fit. DOM inspection reported viewport width 390 and content width 375 (scrollbar accounted for). Viewport restored afterward. This does not certify all screens for accessibility or contrast, and OS theme switching itself was not simulated.
- No existing ordinary or QA client was changed or deleted. API services were restarted on their existing ports after the test checks. No public deployment or GitHub push was performed.

Read-only design screenshots and the corrected dark overview are retained under ignored `.local/design-review`. Neither these visual checks nor the additional tests close the missing dedicated workspaces, full calendar, telehealth, client portal, retention/deletion or complete revenue workflows.
