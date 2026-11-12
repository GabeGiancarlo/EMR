# Security Analysis workflow

`.github/workflows/security.yml` runs on pushes to `main` and `dev`, on pull requests into them, every Monday at 02:00 UTC, and on manual dispatch. It is separate from `Continuous Integration`. A green run means only what the table says; none of it is a HIPAA assessment or a penetration test.

| Job | Fails when | Reports without failing | Does not cover |
| --- | --- | --- | --- |
| CodeQL Analysis | The analysis cannot run. | Findings, on the repository's code scanning page. | The frontend. Findings never fail the job. |
| Dependency Vulnerability Scan | `cargo audit` finds a vulnerability anywhere in `Cargo.lock` that is not listed in `.cargo/audit.toml`; or `cargo deny` finds an advisory, a licence outside the allow list, or a crate that does not come from crates.io, in the graph compiled for the targets in `deny.toml`. | Duplicate crate versions; unmaintained transitive crates. | npm dependencies. The bans step cannot fail today: `deny.toml` bans no crate and allows wildcard version requirements. |
| Docker Security Scan | The image cannot be built or the report cannot be uploaded. | Operating-system CVEs in the API image, on the code scanning page. | Pull requests (skipped; CI builds the same image). Trivy does not read the crates inside the Rust binary. |
| Static Application Security Testing | The `emr-api` crate contains `unsafe`, or cargo-geiger produces no report. | How much `unsafe` each compiled dependency uses. | Whether a dependency's `unsafe` is sound. |
| Secret Scanning | Gitleaks finds a secret pattern in the scanned commits: the commits a pull request or push adds, or all history on the weekly and manual runs. | | Secrets that match no Gitleaks rule. |
| License Compliance | A crate in `Cargo.lock` can only be used under a GPL-family licence. | The licence inventory, as an artifact. | The precise allow-list check, which is `cargo deny check licenses` above. |
| HIPAA-Oriented Controls Check | A `password =` or `secret =` assignment, or a log line naming a password or SSN, appears in Rust or TOML files; or non-test API code gains an `unwrap()` beyond the recorded baseline. | The list of existing `unwrap()` call sites. | Anything about TLS or audit behaviour: those two steps only check that the words appear in the sources. |
| Security Scorecard | The Scorecard run or upload fails. | Repository practices (branch protection, pinned actions, token permissions), on the code scanning page. | Every branch except `main`. Scorecard rejects other branches, so the job is skipped there and on pull requests. |

## When a job fails

**A new advisory.** Read the advisory, then check whether the crate is compiled: `cargo tree --workspace --target all -i <crate>`. If it is compiled and a fixed version exists, update only that crate (`cargo update -p <crate> --precise <version>`) and run the tests. If the crate is locked but never compiled, or the vulnerable path cannot be reached, add the advisory ID with the reason to `.cargo/audit.toml`, and to `deny.toml` as well when the crate is in the compiled graph. Prefer removing the crate: `h2` 0.3 (RUSTSEC-2026-0258) had no fixed release, so actix-web's unused `http2` feature was turned off instead of ignoring the advisory. An entry without a reason that someone else can check is not acceptable.

**A new `unwrap()`.** The step lists every call site clippy finds in the code compiled into the API; tests are not counted. Return an error instead, or, when the value cannot be absent, raise `UNWRAP_BASELINE` in the workflow in the same pull request and say why. Lower the baseline when the count falls.

**A licence outside the allow list.** Adding a licence to `deny.toml` is a product decision. Permissive licences (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode) have been added as needed; copyleft licences have not.

## Current exceptions (November 11, 2026)

Both are in `.cargo/audit.toml`. `deny.toml` has no exceptions, because neither crate is in the compiled graph.

| Advisory | Crate | Why it is ignored | Revisit when |
| --- | --- | --- | --- |
| RUSTSEC-2023-0071 | `rsa` 0.9.10 | Locked through sqlx's optional MySQL driver; never compiled. | The API enables sqlx's `mysql` feature. |
| RUSTSEC-2026-0459 | `libsqlite3-sys` 0.30.1 | Locked through sqlx's optional SQLite driver; never compiled. | The API enables sqlx's `sqlite` feature. |

The `unwrap()` baseline is 29: 14 convert a fixed two- or four-byte slice into an array while parsing WAV headers (`speech.rs`, `media_transcription.rs`), 14 call `as_object()` on a `json!({...})` literal, and one (`patient_identity.rs`) calls `as_object_mut()` on a row that the query builds with `jsonb_build_object`. None was changed.
