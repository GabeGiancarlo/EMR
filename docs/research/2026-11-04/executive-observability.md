# Executive observability and dashboard contract

Reviewed November 4, 2026. Synthesized from the actual Grok `executive-observability-stream` research response, repository inspection and OpenAI core review. This is a product/data proposal, not implemented dashboard acceptance. Outpatient comes first; another care setting, state, payer mix and owner operating model remain unspecified.

## What the owner should be able to answer

An executive needs to understand whether people are receiving care, whether staff can keep up, where work is blocked, and where money is moving. An involved owner may need authorized client-level operational detail; an owner overseeing several locations may primarily need trends and exceptions. Neither needs a clinical dashboard with its labels changed.

The proposed home has four regions: **care delivery**, **work needing attention**, **receivables**, and **cash and reconciliation**. Each headline opens the exact contributing records, filtered by the viewer's permitted scope. Put the data period, facility time zone, last update and calculation definition beside the metric. Offer comparison periods only when both periods have comparable coverage. Unavailable inputs produce “Not available,” not zero.

The first useful financial visualization is a set of distinct stages, not one funnel that implies every dollar is conserved between gross charges and cash. Contractual adjustments, denials, responsibility transfers, refunds and timing differences must be visible.

## Current repository evidence

- `api/src/routes.rs` loads authorized organization collections for bootstrap; audit history is capped. This is not a measured large-organization reporting architecture.
- `frontend/src/workspace.ts` calculates the rendered financial totals; `api/src/routes.rs` also computes bootstrap summaries. Recorded charge totals include draft and ready claims. Label that measure **recorded gross charges**, not submitted claims, earned revenue or cash.
- `api/src/clinical.rs` maps individual group attendance into linked session status: attended/completed, no-show/no-show, excused/cancelled. Attendance inputs exist; they need an agreed denominator and a reporting contract.
- `api/src/routes.rs` already supplies a draft-note aggregate to permitted billing/executive contexts. A new “completed sessions awaiting signature” metric is a different, more specific metric, not the first narrative-free count.
- Executive permissions currently include reading clients and claims, and bootstrap serializes whole authorized rows. That exposes fields such as client emergency contact and claim diagnosis code. Narrative restrictions do not establish field-level minimization; a narrower server response contract is proposed work.
- Migration `202611020008_billing_corrections.sql` constrains claim statuses; a partly paid balance can exist without a dedicated `partially_paid` claim status. Derive a payment condition from financial records instead of inventing a stored enum value.
- Current full allocation, including adjustments, can set a claim to `paid`. That status alone does not prove cash collection or payer adjudication.
- Migration `202611020020_program_configuration.sql` and `api/src/programs/mod.rs` include program capacity and active census. These do not establish a clinician-availability or residential bed denominator.
- `frontend/src/RoleWorkspace.tsx` already distinguishes missing data and role views. That is a useful foundation, not evidence that the current executive experience meets the owner's standard.

## Proposed metric dictionary

These definitions are starting contracts to validate with the first operator. “Existing inputs” means relevant records exist, not that the proposed calculation or user journey has been implemented and accepted.

| Metric | Definition and date basis | Inputs and boundary | Action |
|---|---|---|---|
| Active census | Distinct active episodes at an explicit as-of instant, grouped by facility/program; show distinct people separately if concurrent episodes are possible | Existing episode/program inputs; historical as-of requires effective event history | Review capacity and assignment |
| Intake waiting | Open referrals/intakes by stage, owner and age since entry into that stage | Referrals and admission records exist; stage history must support aging | Resolve stalled intake |
| Time to first delivered visit | Median and upper percentile from agreed intake-ready event to first completed eligible session; show incomplete cohort separately | Proposed cohort calculation; scheduling alone is not delivery | Locate access delays |
| Attendance | Completed divided by completed plus no-show sessions in the eligible ended-session cohort; report cancelled/excused and unresolved attendance separately | Existing statuses; specify reschedule/duplicate rules and facility date | Follow up on missed care |
| Group utilization | Attended individual seats divided by documented capacity across ended eligible groups; exclude no-shows/excused from attended numerator | Existing groups/attendance; missing capacity is unavailable. Count linked attendance once, not both group and session | Adjust group scheduling |
| Appointment capacity | Booked or delivered minutes divided by explicitly available clinician minutes; show those as separate measures | Availability/leave inputs needed; a calendar full of appointments is not capacity | Balance staffing |
| Documentation waiting | Distinct completed eligible sessions lacking the required signed documentation, aged from service end | Existing notes/sessions plus service-specific rules | Assign completion work |
| Authorization exposure | Services needing payer-required authorization with missing, pending, expiring or exhausted authorization; distinguish counts, units and estimated amounts | Versioned payer policy needed; “authorization not required” must be explicit | UR follow-up |
| Delivered but unsubmitted | Completed services partitioned into no charge, draft charge and ready charge, with age; submitted excluded | Current linkage/status inputs; ready is still unsubmitted | Release blockers |
| Recorded gross charges | Sum of current non-void charge cents with status breakdown and declared service/posting date basis | Current claim amounts; not expected reimbursement | Review charge capture |
| Submitted gross charges | Sum of released economic charge lineages using explicit supersession rules; exclude both retransmission and original/replacement double-counts | Submission attempts/versions needed beyond manual status/reference | Track release volume |
| First-pass acceptance | First submitted attempts accepted at the stated receiver divided by first attempts with resolved acknowledgments; pending shown separately | External or manually evidenced acknowledgments; distinguish clearinghouse and payer | Reduce preventable rework |
| Rejection workload | Open pre-adjudication rejection events, age, reason and owner; distinguish holds and transport failures | Proposed event records | Correct and resubmit |
| Denial workload | Adjudicated denied lines/amounts and unresolved follow-up, with a defined denominator for any rate | Adjudication, line amounts and appeal/correction history needed | Correct, appeal or resolve |
| Receivable aging | Open collectible balances as of a date, separated into payer/patient, with a chosen aging origin and unapplied credits visible | Financial event model and adjustment rules needed | Work oldest actionable balances |
| Posted receipts | Payer/client cash receipt events, including unapplied cash, less linked cash reversals/refunds counted once; exclude adjustments and show allocations separately | Existing payments/reversals partly support this; distinct receipt/allocation events are needed. Reallocation is not a new receipt; distinguish receipt and posting dates | Inspect posting exceptions |
| Payment variance | Adjudicated allowed/paid amount versus a valid effective-dated contract expectation, with reason | Expected allowed amount is not charge amount; unavailable without contract evidence | Investigate underpayment |
| Reconciliation | Matched deposit/receipt posting IDs and matched amount, plus unmatched receipts and deposits | Matching records needed; same-period bank deposits divided by receipts is **not** a reconciliation rate | Resolve cash discrepancies |
| Retention and transitions | Episode continuation, discharge and step-up/down cohorts with explicit dates/reasons | Existing episodes plus adequate event history; not a clinical outcome by itself | Review continuity |
| Outcomes and follow-up | Selected validated instrument change, completion rate and follow-up cohort, with missingness and exclusions | Measurement instrument, consent/access, scoring and versioned definition needed | Review care quality with clinicians |

Do not call collections divided by gross charges a net collection rate. Net collection, days in A/R and profitability require additional definitions and inputs; profitability also needs expenses. Set targets with the operator after a trustworthy baseline, not from a vendor marketing percentage.

## Useful customization

Allow saved layouts, a limited widget catalog, visible filters, date ranges, location/program filters and drilldown columns. Each saved view records its owner, definition version and sharing scope. A role default gives people a useful first screen; users can rearrange authorized information without creating a new permission grant.

Views should support “overview,” “exceptions needing action” and “operational detail.” Each exception has an owner, due date, reason and permitted next action. A count such as “2/12” needs a noun, denominator, period and explanation. Personal layout changes must not alter financial formulas or clinical rules. Formula changes are governed configuration with version history.

Exports preserve filters, as-of time, definitions, currency and access scope. Large exports need controlled server-side generation and audit records. A hidden button is not authorization; the API must enforce the same restrictions for saved views, person cards, drilldowns and CSV exports.

## Clinical privacy and meaningful drilldowns

Executive access is a business role, not universal access to psychotherapy or SUD records. Start with aggregate operational facts and reveal identifiable details only under an applicable permission and purpose. Being an involved owner does not itself establish care-team access.

HIPAA's minimum-necessary treatment exception concerns disclosures to, or requests by, providers for treatment; it is not a blanket exception for every internal use. Separately assess Part 2 applicability and the distinct protections for psychotherapy/SUD counseling notes. These are design requirements, not a claim that Nexus is compliant. [HHS minimum necessary](https://www.hhs.gov/hipaa/for-professionals/privacy/guidance/minimumnecessary/index.html), [HHS Part 2](https://www.hhs.gov/hipaa/part-2/index.html).

Published CCBHC quality definitions are useful examples of explicit populations, exclusions and measurement periods. They are not automatically the right measures for this outpatient customer. Confirm the current applicable specification before implementing a named measure; older worker-cited measure sheets are not launch requirements. [SAMHSA CCBHC guidance](https://www.samhsa.gov/communities/certified-community-behavioral-health-clinics/guidance-and-webinars).

## Acceptance and delivery sequence

1. Agree on metric names, dates and denominators; create synthetic event histories with known answers.
2. Build missing financial/operational records as functional slices. Manual, explicitly evidenced external events can precede live connectors; they must remain labeled manual or synthetic.
3. Implement server-side scoped aggregates and drilldowns with matching counts. Test a second organization, restricted clinician, billing viewer and executive viewer.
4. Test no-shows, missing attendance, duplicate/replaced claims, partial receipts, reversal, unapplied cash, deposit timing differences and empty data. Show unresolved data instead of making the dashboard look complete.
5. Validate representative questions with an executive: “Which work is stuck?”, “Which amount is still collectible?”, “Which cash has actually matched a deposit?”, and “Who owns the next action?”

All planned internal capabilities remain in the November 13 scope ledger. Missing records create work, not permission to remove reconciliation, statements, outcome inputs or other planned features from the target. Live integrations remain lower priority. Branding and chart polish follow trustworthy workflows and definitions.
