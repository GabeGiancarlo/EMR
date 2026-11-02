# Local verification record

Verified November 2, 2026 against the local PostgreSQL/API and custom frontend. This records observed behavior, not a production-readiness or compliance certification.

## Isolation

- `nexus`: ordinary workspace; no sample clients or demo fixtures seeded.
- `nexus_test`: explicitly disposable backend integration database. Export tests create and remove an isolated schema inside it.
- `nexus_ui_test`: isolated browser QA database. All names, records, codes, references and amounts are clearly labeled synthetic QA fixtures. No payer submission or real payment occurs.
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

Latest checkpoint: 14 Rust unit tests passed, both comprehensive PostgreSQL/HTTP integration scenarios passed, 3 frontend serialization regressions passed, strict Clippy and formatting passed, and the production frontend build passed. The workflow integration scenario also checks a Pacific-time evening service crossing the UTC date boundary against one-day coverage/authorization and rejects completion of future sessions. Claims readiness and discharge scheduling use the facility calendar date.

## Operational evidence and limits

An actual local backup was hashed and restored into a newly created isolated database; the ordinary workspace was left unchanged. Docker execution on this machine is unavailable because Docker Desktop reports an internal inference-manager socket error. PostgreSQL therefore runs using the official portable EDB distribution. The Docker image is prepared for CI, but a local container runtime check has not passed.

The research HTML report was opened and visually reviewed. The DOCX passed structural validation, but visual rendering could not run because the bundled LibreOffice binary is unavailable on this Windows host; the DOCX is not described as visually verified.

Authentication is intentionally absent. Local actor policies do not establish identity. Public deployment, real patient use, external payer exchange, speech recognition and production operational controls are outside the evidence above.
