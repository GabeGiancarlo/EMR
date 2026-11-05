# Customer discovery and billing journeys

November 4, 2026. Proposed discovery and acceptance material, not a claim that these workflows are all implemented. Outpatient is first; additional settings, state, payer mix and contracted rules remain open. Preserve the full acceptance ledger.

## Demonstrate one service from intake to cash

Use a wholly synthetic outpatient case and a configured placeholder service. Do not imply that a placeholder is a valid payer billing code.

1. Register the person, resolve identity/duplicates and record the episode/program. Record the responsible team and the scope of each user's access.
2. Record coverage and the evidence used to check eligibility. Eligibility and authorization are distinct; neither is a payment guarantee. Show effective dates, last checked time, source and unresolved questions.
3. Schedule the service using the applicable program/service version. Show missing requirements before delivery while distinguishing informational warnings from true blocking policy.
4. Record what actually happened. A scheduled appointment is not evidence of attendance; a group attendance row and its linked individual session represent one person's delivery event. Preserve cancelled/no-show records without converting them into delivered services.
5. Complete the required documentation and human review/signatures. Keep author, version, source references and any subsequent correction. A generated draft is not a signature.
6. Prepare the billing packet from structured verified values. Copy known demographic/coverage/service information, identify missing or conflicting fields, and request the minimum manual work. Do not infer diagnosis, units or clinical facts from prose. Permit role-appropriate evidence review without exposing restricted note bodies.
7. Run readiness against a versioned rule set and the current evidence versions. Present each blocker, owner, source and action. An unsigned note and a delivered service with no claim both belong in the work queue.
8. Freeze an export/submission manifest. Its generated file, source versions, rule version, author, creation time and hash remain immutable. Later corrections produce linked versions; they do not rewrite the old packet.
9. Track the actual exchange boundary. An exported CSV is a handoff; it is not proof of clearinghouse or payer receipt. Separate transmission, acknowledgement, rejection, payer adjudication and appeal/corrected-claim paths. Record manual external references as manual evidence.
10. Record receipts and allocations separately. A receipt may be unapplied, partially applied or associated with several claims. A contractual adjustment changes a balance without adding cash. Patient responsibility requires supported adjudication/policy evidence; it is not inferred from a patient's payment.
11. Match remittance advice, the movement of funds and bank evidence. Keep unmatched money and timing differences visible. A remittance statement alone is not reconciled bank cash. [CMS ERA/EFT guidance](https://www.cms.gov/priorities/key-initiatives/burden-reduction/administrative-simplification/transactions/health-care-payment-remittance-advice-electronic-funds-transfer).
12. Handle corrections explicitly. Preserve earlier service, packet, claim, adjudication and payment versions. Show what changed, who changed it, why, what downstream evidence became stale, and which work item resolves it.

The current Nexus prototype implements portions of this journey: stored intake/program/service/documentation records, local readiness/transitions, manually recorded references, payments/reversals and exports. It does not establish live payer exchange, full adjudication, a financial ledger or bank reconciliation. See [feature reality](feature-reality.md).

## One numeric acceptance example

All amounts below are synthetic integer cents. They describe an agreed test contract, not a fee schedule or coverage rule.

| Event | Amount | Meaning |
|---|---:|---|
| Delivered service charge | 10,000 | Original gross charge; not cash or profit |
| Supported contractual adjustment | 1,000 | Non-cash reduction; evidence retained |
| Supported payer responsibility | 7,000 | Payer balance after adjudication |
| Supported patient responsibility | 2,000 | Patient balance after adjudication |
| Payer receipt allocated | 7,000 | Reduces payer balance; distinguish receipt from bank reconciliation |
| Patient receipt allocated | 2,000 | Reduces patient balance |
| Remaining balance | 0 | 10,000 − 1,000 − 7,000 − 2,000 |
| Cash received | 9,000 | Payer plus patient receipts, excluding the adjustment |

Acceptance must also prove denied, partial-payment, duplicate-import, concurrent-allocation, void/corrected-claim and reversal paths. A reversal preserves the original payment and posts the linked correction. A recoupment must not silently recreate a payer receivable without the supported accounting/adjudication event. A corrected claim must not consume the same delivered service twice or release authorization consumption merely because a claim version changed.

## Billing-team discovery session

Ask the first customer's biller to demonstrate a real workflow using deidentified examples or a synthetic reenactment. Capture the process, identifiers and rule evidence; do not copy real patient data into research-worker prompts.

- Which state, organizations, facilities, settings and provider types are involved? Which payer products and contracts account for most work? Which services are time-based, group-based or otherwise unit-sensitive?
- Who checks coverage, authorization and provider credentialing, and where is the evidence stored? What is checked again before each service? Which exceptions require human judgment?
- What specifically makes a service billable? Which note, plan, signature, attendance, supervision and other requirements come from each authoritative source? What changes by payer/product and effective date?
- What is today's handoff: a billing packet, CSV, document bundle, clearinghouse queue or RCM service? Which fields get retyped, and which documents are repeatedly requested?
- Where do rejected claims, denials, missing information and unmatched payments arrive? Who owns them? What are the actual filing/appeal deadlines, evidence sources and escalation rules? Separate those from internal service targets.
- How are corrected claims, replacements, voids, partial payments, coordination of benefits, contractual adjustments, patient responsibility, refunds and recoupments recorded? Which action changes which balance?
- What is the source of bank truth? What makes the biller confident that a receipt is allocated and reconciled? Which reconciliation differences are expected timing differences?
- Which three screens/exports are opened first each morning? Which recurring task is most repetitive? What information is needed to finish it without opening a clinical note?

Record sample field mappings, versioned rule sources, an exception catalogue, responsible roles and concrete success/failure demonstrations. Do not invent payer policy to fill an unanswered question.

## Executive discovery session

Start with a business question and trace it to an action, not a preferred chart shape.

- What decision should this view support today? For example: where is delivered care waiting for documentation, where is cash held up, or where is staffing capacity constraining access?
- Which financial measures are trusted now, and what do their dates/denominators mean? Who reconciles discrepancies? Which number is gross charge, allowed amount, receivable, cash, expense or profit?
- Which client/clinical outcomes have reliable inputs and a defined follow-up cohort? Which cases require identifiable drilldown, and what independent clinical authority permits it?
- What does an involved clinical owner need compared with an owner reviewing only operations and finance? Business ownership alone should not silently choose a clinical access policy.
- What filters and saved layouts are useful: facility, program, payer, clinician, service, cohort or period? Which changes are personal layout preferences, and which alter governed metric definitions?
- Which exception requires an owner, due date and next step? Can the dashboard link to the exact authorized queue behind the number and explain excluded/missing data?

For each proposed tile, complete: question; definition; population; units; event date/timezone; source records; known/missing inputs; freshness; allowed viewers; drilldown; action owner. A tile with no reliable input should show the missing prerequisite rather than a fabricated zero or forecast. An authorized empty, valid dataset may correctly show zero.

## Clinical expert review

Use the owner's available clinical expertise to review the whole encounter, not only note wording: intake burden, meaningful treatment planning, schedule/attendance exceptions, record correction, supervision, patient communication, and the transition into billing readiness. Evaluate whether autocomplete preserves clinical intent and makes uncertain/missing information obvious. Test names, negation, medication quantities, chronology and conflicting source evidence in synthetic model evaluations; human review remains explicit.

## Separate view choice from permissions and organizations

The current Clinical/Billing/Executive selector changes the presentation available to the selected local staff role. It does not authenticate the user or grant additional record access. The separate walkthrough is a synthetic local database created for review; it is not evidence that every customer needs multiple workspaces. Validate multi-organization membership with actual customer operating arrangements before making it an onboarding requirement. Production onboarding, invitations, identity verification and role assignment need their own acceptance contract.
