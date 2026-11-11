# Outpatient billing: proposed functional contract

November 4, 2026. Coordinator synthesis of the reviewed Grok billing lifecycle, competitor, executive and `outpatient-billing-contract-final` research. The final contract worker returned 60,942 characters with provider-reported cost $1.243500. This is a proposed data/workflow contract, not a migration, payer rule or completed feature. OpenAI core review checked financial semantics and the worked example.

Outpatient is the first customer priority. Additional settings must fit explicit policies, but the second setting, state, payer products and provider contracts remain unknown. Synthetic examples below use USD and invented service codes; they do not establish billability or coverage.

## Product outcome

A biller can identify delivered care that is not ready to bill, see exactly which fact is missing and who owns it, prepare a packet from verified existing information, record what was sent and what came back, post money and adjustments without double-counting, and reconcile receipts to deposits. An owner can follow the same records from care delivery through receivables and cash without receiving unnecessary clinical narrative.

The workflow is:

**Referral/intake → coverage evidence → authorization if applicable → scheduled service → individual delivery/attendance → required signed documentation → reviewed charge and readiness packet → claim version and release attempt → receiver responses → adjudication → receipts/allocations and corrections → reconciliation and follow-up.**

A scheduled visit is not delivered care. Internal ready is not payer acceptance. Acceptance is not adjudication. A remittance is not cash movement. Allocation is not a new receipt. Reconciliation is a documented match, not two same-period totals that happen to agree.

## Proposed records and invariants

| Record | Purpose and invariants |
|---|---|
| Payer/product and billing-policy version | Effective-dated applicability by setting/service/provider and payer product, with source and reviewer; do not infer rules from a payer name alone |
| Coverage and eligibility evidence | Ordered coverage intervals plus dated verification source/result and limitations; a policy row alone is not verified eligibility |
| Authorization requirement/evidence | Required/not-required/pending/denied/authorized state, dates, service/unit scope, source and follow-up owner; not-required does not need a fabricated authorization number |
| Delivered service | Stable clinical service identity, individual attendance and applicable actual time/units; preserves original documentation links and corrections |
| Charge/charge line | Economic obligation in integer cents with stable lineage; canonical service identity prevents duplicates; distinct legitimate services remain possible |
| Readiness evaluation and packet version | Rule version, input versions, each check/result/owner, generated fields and provenance; retain historical evaluation and recheck current validity at release |
| Claim version and line snapshot | Reviewed representation of one or more charges, with identifiers, provider/coding/context fields and original/replacement/void/secondary lineage |
| Submission attempt/export artifact | Immutable file/content digest, item manifest, destination, actor/time and attempt identifier; replay must not create another economic charge |
| Receiver event | Actual transport/acknowledgment/rejection/status response, source/type/time and external identifiers; manual or synthetic evidence explicitly labeled |
| Adjudication and reason | Line-level allowed, paid, adjusted, denied and patient-responsibility facts, with source/version and applicable reason codes |
| Cash receipt | Actual payer/client funds recorded once with source/reference/date; remains visible even if not allocated |
| Allocation | Application of a receipt to an obligation; moving an allocation does not create or destroy cash |
| Adjustment/responsibility transfer | Non-cash event with reason/evidence; contractual adjustment, write-off and transfer to patient are distinct |
| Reversal/refund/recoupment | Linked correction events; reversing a posting is not proof that money moved back to a payer or patient |
| Reconciliation match | Receipt/remittance/deposit linkage, matched gross/net amounts, fees/offsets and unresolved discrepancies; support many-to-many matching |
| Follow-up/appeal/statement | Assigned work with deadlines and history; statement balance comes from valid responsibility and allocations, not a guessed copay |

Keep organization-qualified relationships, server-side permissions, immutable source versions and audit writes in the same transaction as consequential mutations. Financial idempotency keys must be scoped to the appropriate sender/organization/event identity; a duplicate event with conflicting content is an exception, not a silent success.

Enforce conservation under concurrency: net allocations cannot exceed available receipt funds; cumulative allocation reversals cannot exceed the original allocation; reconciliation matches cannot consume the same deposit or receipt amount twice. Keep unidentified provider-level adjustments in an unresolved queue instead of distributing them across claims by guesswork. Money remains integer cents; service quantity precision and rounding are explicit policy choices, not a universal assumption that every future quantity is an integer.

Existing one-service claims, payment records/reversals and source snapshots are useful starting points. They are not all of these entities. Avoid forcing receiver acceptance, adjudication and cash state into one overloaded `status` column. Current `paid` can include full allocation through adjustments; it must not stand in for cash collected.

## Packet autocomplete

Most completion should be deterministic: retrieve identity, coverage, facility/provider, reviewed diagnosis/coding facts, actual delivery, applicable authorization, signed-document references and effective rate from their authoritative records. Each field should expose source record/version/date and the rule that transformed it.

When two sources disagree, show the conflict. When a fact is absent, name the owner and action needed. Do not guess from a nearby appointment, an old note or a model response. Model assistance may draft a reviewable explanation; it cannot invent clinical delivery, authorization, signatures, coding decisions or money.

Unknown payer/transaction configuration is not “not required.” Internal drafts may remain incomplete, but the release view must say “cannot assess release readiness” when the applicable requirements are unresolved. Do not manufacture a universal requirement or silently waive an unknown one.

Templates can vary required documents, field ordering, display labels and applicable rules. Core integrity checks are not customizable away. A permitted exception records its scope, reason, approver and expiration; it cannot manufacture a payer authorization, override absence as attendance or silently change signed history.

Exports include a packet manifest and stable source/charge identifiers, version, creation time, scope and destination. PDF/CSV are useful reviewed handoffs; they are not automatically claim-standard files or evidence of receipt. A receiver adapter stores the actual acknowledgment format it returns, rather than assuming every vendor returns 999 and 277CA transactions.

## Worked synthetic example

An invented `NXS-OP-SESSION` was delivered and documented. Its reviewed charge is **20,000 cents ($200)**. Synthetic adjudication allows **12,000 cents ($120)**, assigns **9,600 cents ($96)** to the payer and **2,400 cents ($24)** to the patient, and identifies an **8,000-cent ($80) contractual adjustment**.

The adjustment is not a cash receipt. The responsibility split is not a second receivable. The total supported obligation after adjustment is $120.

| Step | Cash received to date | Non-cash adjustment | Allocated cash | Remaining payer | Remaining patient | Key distinction |
|---|---:|---:|---:|---:|---:|---|
| Adjudication recorded | $0 | $80 | $0 | $96 | $24 | $200 charge = $80 adjustment + $120 remaining |
| Payer receipt A: $60, allocated | $60 | $80 | $60 | $36 | $24 | Partial payer settlement; not full payment |
| Patient receipt B: $24, allocated | $84 | $80 | $84 | $36 | $0 | Patient obligation settled; payer still owes $36 |
| Payer receipt C: $36, allocated | $120 | $80 | $120 | $0 | $0 | Economic balance is zero; bank matching is separate |
| Allocation C reversed for correction | $120 | $80 | $84 | $36 | $0 | $36 becomes unapplied cash; no refund occurred |
| Same receipt C correctly reallocated | $120 | $80 | $120 | $0 | $0 | No new cash receipt and no duplicate charge |

At every step, remaining obligation equals charge minus supported non-cash adjustments minus net allocations. Unapplied cash is separately visible. A reversal/refund that actually moves money needs its own cash event and evidence, not the allocation-reversal shortcut.

For reconciliation, suppose the $96 payer funds match a $96 bank deposit and the $24 patient card receipt matches a $23.40 deposit with a separately evidenced $0.60 processor fee. Total cash receipts are $120, net deposits are $119.40 and fees are $0.60. The fee is not a new patient balance or a contractual write-off. Matching must show the component records; matching unrelated totals is insufficient. These values are illustrative, not a fee schedule or accounting policy for a customer.

If an original and corrected claim version represent the same $200 service, submitted economic charges remain $200 under the chosen supersession rules. Attempt volume may increase, but receivables must not double. Show attempt counts separately from economic amounts.

## Proposed acceptance scenarios

1. Missing or conflicting identity/coverage fields block the applicable packet checks without losing entered work.
2. Policy effective dates and verification evidence are distinct; stale verification is shown under the configured policy.
3. Required authorization missing/pending/denied blocks applicable release; explicit not-required evidence passes without a fake number.
4. Authorization exhausted, wrong service or wrong date fails, including concurrent reservations and corrected claim lineage.
5. Scheduled/no-show/excused care cannot release a delivered-service charge. Any administrative draft or separate permitted missed-visit fee stays distinct.
6. Group care records attendance per person; shared group text cannot create identical bills for absent members.
7. Required unsigned documentation blocks release; a signed material amendment creates impact review without rewriting prior submitted evidence.
8. Every completed packet field has inspectable source/rule provenance; conflicts remain visible.
9. Differently cased equivalents of the same service cannot create duplicate live economic charges; distinct valid services remain possible.
10. Concurrent ready/release actions preserve authorization reservations and create no duplicate attempts or obligations.
11. Export/retry preserves an immutable version and manifest; a changed packet produces an explicit new version.
12. Mixed batch results show each item outcome; transport acceptance does not become payer adjudication.
13. Pre-adjudication rejection, adjudicated denial, correction, appeal and void have distinct history and permitted next actions.
14. Duplicate receiver events do not duplicate money; conflicting duplicates enter an exception queue.
15. Partial payment, adjustment and patient responsibility follow the worked example with exact integer-cent arithmetic.
16. A receipt not allocated to an obligation remains unapplied; bank-reconciliation status is independent. A bank-unmatched receipt may already be allocated. Allocation reversal/reallocation preserves actual cash totals and provenance.
17. Refund/recoupment and posting reversal are separate. A supported reversal of an already received amount can create a refund liability until the actual return/offset is evidenced; do not silently consume unrelated unpaid receivables or claim that money moved.
18. Deposit matching handles timing differences, combined payments, split deposits and fees without inventing patient balances.
19. Restricted clinician/biller/executive/patient actors fail unauthorized API reads/writes/exports; another organization cannot access any records.
20. Restart/restore retains versions, financial histories, reconciliation links and audit evidence; dashboards and exports match the same scoped records.

These are proposed tests, not executed results. Map them to the existing integration coverage before adding tests; several underlying authorization, payment/reversal and ready/draft/void cases already exist.

## Delivery and open decisions

First close the canonical service identity/duplicate-charge risk. Then agree on record contracts and implement the example as a complete local workflow, with manual evidenced receiver events and synthetic fixtures clearly labeled. Add queues and executive measures from those records. Live eligibility, clearinghouse, payment and bank connectors remain lower priority; internal statements, allocations, reconciliation and appeals remain in the planned scope.

Customer discovery must determine state, payer products, provider enrollment/contracts, applicable code-set rights, unit/time rules, requirements for authorization/documentation, patient-fee policy, statement practices, accounting calendar and outside-biller handoffs. Medicaid eligibility under age 21 can matter for adults aged 18–20; adult-first scope does not remove that discovery. No single generic packet can substitute for these decisions.

## Migration and worker-review decisions

Adopt additive records and explicit backfills. Preserve existing claim IDs, payment references, reversals and historical snapshots. Compare old and new financial projections per claim and in aggregate before switching reads; any unexplained difference blocks that change. Missing allowed amounts or external evidence remain unknown, not zero-filled. Test restart/rollback handling and serialize migration numbering.

If scheduling authorization holds are introduced, distinguish them from delivered-service consumption and transfer them atomically; do not subtract both for the same service. Replaced claim versions must not reserve units again. Keep existing denied-claim consumption unless an explicit supported correction transfers or reverses it.

Voiding a billing representation does not by itself establish that delivered care stopped consuming authorized units. Hold release and reversal of actual consumption need separate policies and evidence. A refund liability may be settled by an actual return or an evidenced offset against a later payment: preserve the actual net deposit and link the offset, rather than inventing a negative bank deposit.

The final worker added useful separation of identity/version/attempt, idempotency conflict handling, partial recoupment liabilities and backfill comparisons. Several recommendations were not adopted: outpatient does not universally mean professional claims; current payments can partially allocate a balance; bank matching is not the first moment cash can be recognized as received; a facility-local accounting calendar is not yet selected; and internal claim-file, multiline, statement and reconciliation work cannot be deferred merely because external connections are lower priority. Existing no-show readiness blocking does not prove that administrative draft creation is prohibited. These distinctions preserve the owner's scope and source-confirmed behavior.
