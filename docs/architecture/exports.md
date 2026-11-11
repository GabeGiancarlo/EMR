# Financial exports

These endpoints generate local CSV downloads from stored records. They do not transmit records to an accounting product, clearinghouse, or payer. They do not alter claim status or mark anything submitted. Authentication remains deferred; the local actor policy is the same prototype policy as the API.

## Accounting reconciliation

`GET /api/exports/accounting.csv?from=2026-11-01&to=2026-11-30`

Requires an active local actor with role `admin`, `billing`, or `executive`, plus the existing `payments` read permission. A clinician's temporary payments exception does not grant bulk accounting export.

Both dates are mandatory, use exact `YYYY-MM-DD`, and are inclusive `received_on` business dates. Maximum span is 366 inclusive days. One day is valid. Reversed, malformed, incomplete, or wider bounds fail with a validation error. This is an organization-wide aggregate rather than a facility export.

Columns are `received_on`, `source`, `entry_count`, `cash_received_cents`, `adjustment_cents`. There is one row for each booked day/source combination. A receipt contributes positively on its `received_on` date; its full reversal contributes negatively on `reversed_on`, using the original payment's source. The CSV's `received_on` column therefore means event booking date for reversal entries. Reversals do not rewrite the original period. `entry_count` counts both receipt and reversal events, including events that net to zero. Only payer and client sources contribute to cash; adjustments are reported separately and must not be interpreted as cash receipts. A day can have a negative net total. The current schema has no currency or general-ledger account mapping, so this is a reconciliation extract, not a balanced journal or a QuickBooks import specification.

The output projection has no client, claim, staff, policy, or member IDs; no payment reference or reversal reason; and no names, contact details, diagnosis codes, or narrative. The reversal query joins only the original payment to recover its source, and selects no identifying or free-text fields. This data minimization is not a legal de-identification determination: small counts, dates, and other contextual knowledge can still be sensitive. Review the data and intended recipient before external use.

## Internal claim review

`GET /api/exports/claims.csv`

Optional `from` and `to` parameters must be supplied together. With no parameters, the export includes the past 30 UTC calendar days including today. Date filtering uses claim creation time, not date of service, submission, or last update. The same 366-day limit applies. Matching more than 10,000 claims returns an error requiring a narrower period; no silent truncation occurs.

Only active `admin` and `billing` roles may export claims, even if another role has a temporary claim-reading exception. Existing claims read permission is also checked. Every query includes the actor's organization ID.

Columns: `export_purpose`, `claim_id`, `client_id`, `policy_id`, `session_id`, `authorization_id`, `service_code`, `diagnosis_code`, `units`, `amount_cents`, `status`, `external_reference`, `denial_reason`, `void_reason`, `created_at_utc`, `updated_at_utc`.

The purpose marker is `internal_review_not_payer_submission`. This is a CSV of actual stored workflow fields, not an X12 837, clearinghouse file, claim validation certificate, or proof of external acceptance. Claim records and free-text references can contain sensitive information; the download requires the same careful handling as the underlying billing workspace.

## Transport and audit

Files have `text/csv; charset=utf-8`, explicit attachment filenames, `Cache-Control: no-store, private`, `Pragma: no-cache`, and `X-Content-Type-Options: nosniff`. Filenames contain only endpoint type and validated dates.

CSV fields are always quoted; quotes are doubled and embedded CR/LF stay within quoted fields. Potential formula prefixes (`=`, `+`, `-`, `@`) in text are neutralized with an apostrophe, including after leading Unicode whitespace or a BOM. Leading tab/CR/LF values are neutralized; NULs are removed. Accounting count and cent fields serialize typed integers directly, preserving negative reversal totals as numbers; user-supplied text never takes that path. This deliberately alters dangerous text display values in exported files, leaving stored source values unchanged. Do not remove these protections before opening untrusted text in a spreadsheet.

The read and audit insert share a transaction, and no response is returned unless the audit transaction commits. Audit actions are `export_accounting_csv` on payments and `export_claims_csv_internal_review` on claims, with the actor and organization recorded. Current audit schema records the export event but not its date filters or row count; neither a download event nor a successful HTTP response proves who subsequently opened or forwarded a file.

Empty results produce headers only. No demo rows or fabricated totals are added. Failed requests return errors rather than empty success files.

## Validation

Unit tests cover quoting, embedded line breaks, formula prefixes and whitespace bypasses, calendar syntax, invalid dates, inclusive spans, required bounds, default range, empty files, and the accounting projection's intentionally narrow data shape.

`api/tests/exports_postgres.rs` exercises the configured Actix HTTP routes against real PostgreSQL in a uniquely named schema inside `nexus_test`. It covers missing/invalid/inactive actors, allowed roles, denied roles despite temporary read exceptions, two-organization isolation, header-only files, exact daily cash/adjustment aggregation including reversal dates, private-value exclusion, response headers, stored claim values, UTC creation filtering, and successful audits. It also forces audit insertion to fail and verifies that neither export returns a downloadable success. These are in-process HTTP route checks, not browser or external-network tests. The synthetic schema is removed after a successful test; application data is never seeded.

Run with `TEST_DATABASE_URL` set to the disposable `nexus_test` database: `cargo test -p emr-api --test exports_postgres -- --ignored`. The test checks both the connection-string suffix and actual database name before creating fixtures.
