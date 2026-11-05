# xAI request evidence and cost accounting

Snapshot: November 04, 2026, 04:22:18 PM Eastern. Source is the ignored shared ledger `.local/xai/usage.json` plus saved provider responses. No API credential is present in this document or those response/request artifacts.

## Totals

- Authorized ceiling: **$1,500**, not a spending target.
- Confirmed provider-reported charges: **$7.476206**.
- Provider-reported web-search calls across returned responses: **152**.
- Substantive completed reports: **10**. Completed acknowledgment-only/invalid reports: **3**; these are charged but are not research deliverables.
- Unresolved-cost reservations: **$225.00**. These are internal holds, **not confirmed charges** and not a provider upper bound.
- Still-running reservations at this snapshot: **$0.00**.

The authenticated `/v1/api-key` check matched `nvim-grok` to the exact team in the owner's console URL. The key and team were not blocked/disabled. The key came privately from macOS Keychain service `xai-api-key`. The `/v1/models` response listed `grok-4.7`. That model-list smoke check itself was not a paid generation test.

The explicit [paid logo research test](logomark-paid-test.md) returned a complete report and $0.927428 provider-reported cost. The console browser was signed out; no account-wide billing display, date filter, balance or invoice was verified. Do not invent a dashboard delay or assume interrupted calls were free. The owner can correlate the key name, Eastern/UTC date and response IDs with the provider's usage/support records.

xAI documents `usage.cost_in_usd_ticks` as per-request billed cost, including tool usage and discounts. Divide integer ticks by 10,000,000,000; do not add tool costs a second time. [Official cost tracking](https://docs.x.ai/developers/cost-tracking), [official key metadata endpoint](https://docs.x.ai/developers/rest-api-reference/inference/other).

## Request outcomes

| Task | Outcome | Reported cost | Report characters | Provider response ID |
|---|---|---:|---:|---|
| `repo-context` | failed-cost-unknown | Unresolved | — | `not captured` |
| `feature-reality` | failed-cost-unknown | Unresolved | — | `not captured` |
| `architecture-readiness` | failed-cost-unknown | Unresolved | — | `not captured` |
| `billing-lifecycle` | failed-cost-unknown | Unresolved | — | `not captured` |
| `billing-competitors` | interrupted-cost-unknown | Unresolved | — | `not captured` |
| `executive-observability` | interrupted-cost-unknown | Unresolved | — | `not captured` |
| `brand-foundation` | interrupted-cost-unknown | Unresolved | — | `not captured` |
| `repo-context-stream` | completed-invalid-report | $0.043456 | 177 | `d8859e53-aaa3-98d3-a17d-63cdbaf4e8a6` |
| `feature-reality-stream` | completed-invalid-report | $0.221284 | 154 | `d1580b7b-c510-9745-88e2-399865ea82ea` |
| `architecture-readiness-stream` | completed-invalid-report | $0.148932 | 130 | `2166f705-a48d-9c2e-afe5-55118666f632` |
| `billing-lifecycle-stream` | completed | $0.846056 | 29001 | `1f5c4540-dc96-91af-a2ea-174272c5ac21` |
| `billing-competitors-stream` | completed | $1.259212 | 36983 | `f8584a9f-26b7-9fbb-9c3a-d8a6ae2339b4` |
| `executive-observability-stream` | completed | $0.997508 | 35441 | `5c811804-8dc5-9c48-88d8-7ec5994e302c` |
| `brand-foundation-stream` | completed | $0.936452 | 27604 | `39b4ee88-4942-9164-96ed-a8d1b60fc36c` |
| `program-change-review` | interrupted-cost-unknown | Unresolved | — | `eaa292f2-7d67-90f6-a9f8-49f9098d408d` |
| `outpatient-billing-contract` | interrupted-cost-unknown | Unresolved | — | `9d44d216-fe42-9894-9c05-ec2fd3381457` |
| `logomark-paid-test` | completed | $0.927428 | 30332 | `1ce05b78-b5d4-9292-a122-83f6d807f7ee` |
| `repo-context-final-report` | completed | $0.163882 | 38413 | `8de93f7b-b9dc-9fe0-82b1-2f5e85225e2e` |
| `feature-reality-final-report` | completed | $0.247706 | 36303 | `abbbe32d-8800-9b35-861c-ed13d3913f37` |
| `architecture-readiness-final-report` | completed | $0.173002 | 34343 | `20f29248-f3ce-9468-a3fc-c6718b9e8b81` |
| `program-change-review-final-report` | completed | $0.267788 | 28758 | `47213642-a596-9507-9713-d61efdba1ee0` |
| `outpatient-billing-contract-final` | completed | $1.243500 | 60942 | `a01a7d2e-6e29-95d5-962c-6aebaf3eb816` |

## Failure and recovery history

The initial non-streaming fleet timed out or was interrupted without recoverable output. A later streaming fleet returned four substantive web-research reports, three inadequate acknowledgment-only source reports, and two subsequently interrupted calls. Unknown costs retain their reservations. New task names identify deliberate follow-ups; there were no automatic generation retry loops.

The logo test exposed concatenated interim JSON progress objects preceding the final report. The runner initially marked it invalid, then the final complete object was recovered from the already saved response. The parser was fixed and checked against incomplete final objects, trailing contamination and missing output. That recovery did not make another API call.

The final source workers use a complete-report schema. Schema/length validation only establishes that a report was returned; coordinator/core review still checks its conclusions. Material false positives were rejected rather than implemented. Source packets contain selected tracked working-tree bytes with hashes, not necessarily pristine HEAD content; `.local`, runtime data, credentials, audio and patient records were excluded. New central documents were explicitly supplied as reviewed text for the context audit.

## Reconciliation limits

The ledger is local to this research pass, not the team's account-wide invoice. Calls with no returned usage cannot be reconciled from this file alone. Where a response ID was captured it is retained above; the original non-streaming failures have no captured ID. Provider usage records are required to resolve these charges. Keep the holds until that evidence exists.

Raw provider responses, request source manifests and full unedited worker reports are ignored local review artifacts under `.local/xai/`. Published research documents are edited syntheses, with important claims checked against repository source or primary references. No clinical data, audio or database dumps were sent to Grok.
