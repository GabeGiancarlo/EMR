# Billing and RCM competitor research

Reviewed November 4, 2026. Based on actual Grok web research (`billing-competitors-stream`), primary vendor documentation and OpenAI core review. Public documentation establishes advertised/documented behavior, not independent performance. No competitor tenant, pricing agreement, BAA or live claim was tested. These are relevant comparison candidates, not a verified market-share ranking.

## Market map

| Candidate | Category | Useful evidence | What remains unknown |
|---|---|---|---|
| Kipu / Avea | Behavioral-health EMR plus specialty RCM | Detailed public utilization-review, billing-audit, rejection/correction and payment workflows | Customer-specific configuration, usability and performance |
| AZZLY / Ease Health | Integrated clinical/practice-management/RCM positioning | Autofill, claim worklists and configurable-rule product descriptions | Current contracting/brand relationship and actual task sequence |
| Behave Health | Behavioral-health clinical/RCM product | Authorization, documentation, denial and appeal positioning | Execution of these workflows in a tenant |
| Alleva | Behavioral-health EMR-to-billing workflow | Encounter review, transmission, claim status and payment/secondary-claim procedures | Customer configuration and connector behavior |
| Waystar | Clearinghouse and revenue-cycle platform | Claim edits, exchange and rejection/denial terminology | Fit, enrollment, pricing and division of support responsibility |
| Claim.MD | Clearinghouse with public integration API | Claim response, polling, remittance retrieval and matching contracts | Customer enrollment and actual integration acceptance |
| SimiTree | Outsourced billing/RCM/UR services and analytics | Defines the work an outside billing team may own | Their actual intake/export requirements and service terms |

The product opportunity is not merely another claim screen. It is a reliable handoff from care evidence to a billing work queue, with fewer retyped fields, explicit blockers and visible financial history.

## Workflow lessons supported by public documentation

**Kipu/Avea has particularly detailed operational documentation among the reviewed sources.** Its integration separates clinical/charge records from insurance, utilization plans and billing. Nexus can preserve one connected product while maintaining equally explicit ownership of facts. [Kipu integration overview](https://kipuhealth.zendesk.com/hc/en-us/articles/8136104930587-Kipu-EMR-Kipu-RCM-Integration-Overview).

Kipu utilization plans carry payer, service, provider, dates, units and authorization state. Its documented pending-authorization submission gate and over-unit warnings are product rules for that workflow, not universal payer rules. Nexus should represent required, pending, denied, authorized and not-required evidence explicitly. A permitted exception cannot manufacture authorization or make undelivered care billable. [Kipu utilization plans](https://kipuhealth.zendesk.com/hc/en-us/articles/8949246488219-Add-Utilization-Plans-with-Kipu-RCM-Integration).

Avea's biller procedures revolve around attendance, rejection and correction queues. This supports a task-oriented Nexus home: who needs to act, why, by when, and on which record. A scheduled service is insufficient evidence that care occurred. [Avea biller practices](https://aveaoffice.zendesk.com/hc/en-us/articles/16510387772955-Best-Practices-for-Billers).

Kipu's billing-audit tool exposes patient/service/coding information, rule history, do-not-bill reasons and batch results. Nexus should show the source and rule version for every filled packet field and distinguish batch transport success from per-item acceptance. [Kipu Billing Audit Tool](https://kipuhealth.zendesk.com/hc/en-us/articles/8136143945755-Billing-Audit-Tool-Report-Send-Billable-Items-Integrated-Billing).

Payment posting includes matching and review; mismatches and overbalances need explicit handling. The lesson is to retain remittance, allocation, reversal and resolution evidence rather than silently editing original amounts. [Avea automatic posting](https://aveaoffice.zendesk.com/hc/en-us/articles/16459840755355-Automatic-Payment-Posting), [Avea balance resolution](https://aveaoffice.zendesk.com/hc/en-us/articles/16459342384795-Resolve-Insurance-Claims-Out-of-Balance).

**AZZLY/Ease:** the reviewed AZZLY billing URL redirected to Ease Health. Ease describes automated claim preparation, rule checks and worklists; these remain product claims until demonstrated. The redirect alone does not establish corporate ownership or the user's future contract. [Current RCM product page](https://easehealth.com/products/rcm).

**Behave:** public pages describe authorization dates/units, review ownership, documentation gaps and denial follow-up. Use them as demo questions, not proof that a specific screen performs the task well. [Utilization review](https://behavehealth.com/utilization-review), [Billing and collections](https://behavehealth.com/billing-and-collections), [Denials](https://behavehealth.com/denials).

**Alleva:** its support documentation distinguishes service/encounter processing, claims and payment management. Secondary claim preparation and release are explicit steps. Nexus should retain original/secondary/replacement lineage and a preview/release boundary, even before a live clearinghouse connector. [Alleva service and claim management](https://support.helloalleva.com/service-and-claim-management).

**Waystar:** configurable edits and connected claims management are relevant future integration capabilities. Its published terminology reinforces the distinction between pre-adjudication rejection and adjudicated denial. Marketing clean-claim rates are not Nexus acceptance criteria. [Waystar Claim Manager](https://www.waystar.com/our-platform/claim-management/claim-manager/), [Waystar terminology](https://www.waystar.com/blog-rev-cycle-101-healthcare-revenue-cycle-terminology-denial-prevention-glossary/).

**Claim.MD:** public docs describe claim-level responses, polling cursors and ERA retrieval/matching. Its API returns JSON/XML rather than a 999; documented 277 exchange is through SFTP, and 278 is unsupported. A Nexus adapter therefore needs receiver-specific acknowledgment semantics instead of pretending every connector returns the same X12 objects. [Claim.MD transactions](https://docs.claim.md/docs/which-x12-claim-status-and-acknowledgment-transactions-are-supported-by-claimmd), [Test-account guide](https://docs.claim.md/docs/test-account-quickstart-guide), [ERA workflow](https://docs.claim.md/v1/docs/view-era).

**SimiTree:** outsourced RCM/UR services illustrate another integration boundary: a billing team may work outside Nexus. Provide controlled packets, stable identifiers, return-status ingestion and task ownership. Routine access need not expose clinical narrative; supporting documentation may still be necessary under an authorized, limited disclosure workflow. [SimiTree RCM services](https://simitreehc.com/capabilities/services/revenue-cycle-management).

## Proposed Nexus billing experience

Start with a work queue, not a clinical face sheet. The primary columns are client, service date, blocker, assigned owner, age and next action. Separate lanes for incomplete care documentation, coverage, authorization, coding/configuration, export/submission errors, rejections, denials and payment exceptions. Preserve client/episode context when opening a task.

A readiness packet is a reviewable projection of stored evidence. It is not a universal insurer form or a reimbursement guarantee. Its requirements vary by payer/product, service, setting, provider and effective date.

| Packet fact | Safe completion source | Required handling |
|---|---|---|
| Identity, episode and facility | Versioned stored records valid for service date | Flag conflicts and missing relationships |
| Coverage/eligibility | Coverage interval plus separately recorded verification evidence | Do not equate having a policy row with verified eligibility |
| Diagnosis and coding | Authorized structured chart facts and applicable versioned billing rules | Preserve order, pointers and reviewed overrides; do not infer a diagnosis |
| Attendance, time and units | Individual delivery record plus the applicable unit rule | Schedule alone cannot establish delivered time or units |
| Authorization | Applicable authorization evidence or explicit not-required policy | Never invent an authorization number for not-required care |
| Rendering/billing provider | Recorded provider and applicable credentials/enrollment rules | Missing billability evidence remains a blocker where required |
| Charge | Effective rate or authorized explicit amount | Charge is not payer allowed amount |
| Patient responsibility | Applicable adjudication/contract/self-pay evidence | An estimated intake copay is not automatically a final balance |
| External status | Stored receiver response with source/time/identifiers | Manual status entry is labeled manual; a file receipt is not payer acceptance |

Autocomplete should be mostly deterministic retrieval and rule application. Show source record/version/date, conflicts and why a value changed. Model assistance can draft reviewable explanations; it must not invent attendance, signatures, authorization, coding decisions or financial facts. Preserve immutable exports/claim versions and explicit corrections. Retransmission must not duplicate receivables.

No-show records must not release charges as delivered clinical services. A draft billing work item or a separate missed-appointment fee has different semantics and policy requirements. Do-not-bill needs a reason and history. Closing an episode must not erase outstanding financial work.

Configurable customer mappings do not establish rights to use proprietary code sets. Confirm the actual CPT licensing use case, including development/testing and customer-supplied content. Use internal synthetic service codes for demonstrations pending that review. [AMA CPT licensing FAQ](https://www.ama-assn.org/practice-management/cpt/cpt-licensing-frequently-asked-questions-faqs).

## Acceptance scenarios and sequencing

Demonstrate unsigned-note blocking, missing required authorization, not-required authorization, exhausted units, a no-show, a reviewed packet with provenance, export replay, per-item rejection, adjudicated denial, correction/replacement, partial payment, unmatched remittance, reversal, patient responsibility and bank matching. Include restart persistence and API permission denials. A hidden control is not a permission test.

The full internal financial scope remains in the acceptance ledger, including statements, remittance/reconciliation and follow-up workflows. Manual evidence and synthetic receiver fixtures can exercise those features before live exchange. Live eligibility/claims/ERA, payment processing and calendar connections remain lower priority; no worker recommendation changes the owner's scope without a decision.

Choose a future clearinghouse from the first customer's enrollment, payer support, test environment and support model. Public API quality is one input, not a sufficient purchase decision. No account, contract or integration purchase was made in this research.

## Interview and demo checklist

1. Walk through one ordinary claim and one rejection/denial using de-identified examples.
2. Show pending and exhausted authorization; identify the decision owner and override limits.
3. Schedule a visit without attendance and confirm what can and cannot become billable.
4. Change a diagnosis or signed note after claim creation; inspect version and correction history.
5. Submit a mixed-success batch and locate every item outcome.
6. Post partial, duplicate and mismatched remittances; show reversal and unapplied cash.
7. Explain expected reimbursement, patient responsibility and actual bank reconciliation separately.
8. Demonstrate clinician, biller, executive and outside-biller access at the API and UI.
9. Export the packet required during downtime and re-import a status without duplicating money.
10. Confirm actual payer/state rules, contract entity, enrollment ownership, support and licensing.

No state Medicaid manual or payer contract was selected because those details remain unknown. Additional care-setting support is desired; existing residential/detox labels do not establish completed workflows for those settings.
