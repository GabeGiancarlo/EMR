# Billing lifecycle and readiness packets

November 4, 2026. Developed from Grok 4.7 worker `billing-lifecycle-stream`, then edited by the Codex coordinator; consequential billing/code claims received an OpenAI core review. This is research and proposed implementation work, not new feature acceptance or payer/legal approval. Source locations refer to the reviewed packets; current code and effective payer guidance must be checked before implementation.

The prototype can already stop an incomplete outpatient encounter from being marked internally ready. It cannot yet tell a biller whether a payer will accept, adjudicate, or pay that encounter. Those are different events, and the Nov 13 build should make the difference impossible to miss.

## 1. What the owner should treat as fact

Nexus today implements internal readiness and recorded financial transitions; the end-to-end payer revenue cycle remains incomplete. `api/src/readiness.rs:1-2` and `frontend/src/ClaimReadiness.tsx:111` say the checklist does not transmit a claim. `docs/architecture/data-model.md:77` states the prototype does not implement payer adjudication, remittance allocation, direct submission, refunds, a general ledger, or claim attachments. `docs/architecture/exports.md:3` and `frontend/src/ExportReports.tsx:114-116` state the CSVs are not an X12 837 and do not submit anything.

`docs/product/roles-and-workflows.md:20-21` lists “claim creation and submission” and “appeals” as billing workflows. That is a role description, not evidence those transactions exist. The implemented claim actions in `api/src/workflow.rs:153-241` are local status changes: draft, ready, a manually typed external reference called “submitted,” denied, voided, and a payment reversal that can move a paid claim back to submitted or denied. None of those statuses is payer acceptance or cash in the bank.

First priority is adult outpatient. Existing program configuration supports outpatient, IOP and PHP. Customer-specific payer rules are unverified; residential/inpatient support is not established, and OTP has not been chosen as a target. Launch state, payer mix, licenses, and contracted codes are unknown. Nothing below selects a state, a CPT/HCPCS code, or a claim form for launch. Medicare examples are illustrations of why the rules diverge. They are not the customer’s payer, and they are not a universal behavioral-health rulebook.

Unit tests in `api/src/readiness.rs:340-409` and `api/src/applicability.rs:458-710` are present in source. This review did not execute them. Export route tests are reported in `docs/architecture/exports.md:39-45`, not re-run here.

## 2. Nine different events that staff collapse into “billing”

| Event | What it actually answers | What it does not answer |
|---|---|---|
| Eligibility | Was this person a member of this plan on this date, and which product? | Whether this service is covered, authorized, or payable. CMS’s Medicare HETS companion guide says a 271 must not be read as a guarantee of payment. [CMS reference](https://www.cms.gov/files/document/current-hets-270-271-companion-guide.pdf) |
| Benefits | What limits, network rules, copay, deductible, coinsurance, and visit or day caps apply to this benefit? | Whether today’s note will survive medical-necessity review. |
| Authorization | Did this payer grant units, days, or a date span for a stated service? | Eligibility, and not the clinician’s necessity judgment. Some outpatient plans do not require it at all. |
| Medical necessity | Is the service reasonable and necessary, or otherwise within that payer’s criteria, and does the record show it? | A signed note. Medicare excludes services that are not reasonable and necessary for diagnosis or treatment. [eCFR reference](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-B/part-411/subpart-A/section-411.15) |
| Documentation | Is the encounter record complete enough for this organization’s packet and, later, this payer’s audit? | Payer acceptance. |
| Internal readiness | Did Nexus’s configured checks pass? | Transmission, acceptance, or payment. This is the only gate implemented. |
| Payer acceptance | Did the file parse (999/TA1) and did the claim pass front-end edits into adjudication (277CA)? | Payment. Transaction acceptance in a 999 can be followed by claim-level rejection in a separate 277CA. Medicare EDI instructions distinguish translator acknowledgments from claim acknowledgments. [CMS reference](https://www.cms.gov/files/document/r13954cp.pdf) |
| Adjudication | What was allowed, adjusted, denied, and assigned to the patient, with standard reason codes? | That the money is in the operating account. |
| Cash | Did an EFT, check, card, or patient payment hit the bank, and does it match the remittance? | That the clinical service was appropriate. |

HIPAA adopted transactions, for covered entities that conduct them electronically, are X12 5010: 837 for claims, 270/271 for eligibility, 278 for referral/authorization, 276/277 for claim status, and 835 plus NACHA CCD+ for remittance and EFT. Claims have no federally mandated operating rules. Eligibility, claim status, and EFT/ERA do. [CMS reference](https://www.cms.gov/priorities/key-initiatives/burden-reduction/administrative-simplification/hipaa/adopted-standards-operating-rules) and [CAQH reference](https://www.caqh.org/core/operating-rules). Code sets are separate: ICD-10-CM for diagnosis, CPT for outpatient physician services, HCPCS for supplies and many non-CPT services. CPT text is licensed; do not embed a scraped descriptor library. Customer-supplied mappings do not establish product licensing rights. Confirm the actual use case, including development/test, and use internal synthetic codes for demonstrations pending review. [AMA CPT licensing guidance](https://www.ama-assn.org/practice-management/cpt/cpt-licensing-frequently-asked-questions-faqs).

An 835 explains payment. It is not the deposit. Health plans that pay through ACH must put the 835 reassociation trace number in the EFT addenda so the provider can match the file to the bank credit. [CMS reference](https://www.cms.gov/priorities/key-initiatives/burden-reduction/administrative-simplification/transactions/health-care-payment-remittance-advice-electronic-funds-transfer) and [CMS reference](https://www.cms.gov/files/document/guidance-letter-vcc-eft-era.pdf). Virtual cards are permitted and break that match unless someone records them by hand. Patient responsibility on an 835 is an adjudicated amount, not cash collected.

## 3. The path, in operating order

**Referral and intake.** A referral is an operational lead, not coverage. The chart needs a person, an episode, a facility, and a program. Source already separates client from episode and stores level of care (`docs/architecture/data-model.md:49-57`). Referral type/name/contact/reason are already stored by migration 019 and the intake workflow. A dedicated dated benefits-verification result and supporting payer evidence remain separate gaps; absence from the worker packet alone did not prove absence from the application. Adult age 18 on the facility calendar is an API invariant (`data-model.md:9`). That is a product rule, not a billing rule.

**Eligibility and benefits.** Staff confirm member ID, subscriber, plan, effective and termination dates, other insurance, in-network status, and behavioral-health carve-outs. A card image is not a 271. Carve-outs matter in recovery care: the medical plan on the card may not be the entity that adjudicates substance-use counseling. Store the answer as a dated verification with who checked it, which source, and which service category. Do not copy that answer onto every future date of service.

**Authorization.** Only if a configured rule says this payer, plan, service, and care level require it. Capture number, policy, service or revenue scope, start, end, approved units, and status. Nexus already matches a selected authorization to policy, service code, active status, and facility date, and reserves units on ready, submitted, denied, and paid claims (`api/src/readiness.rs:80-124`, `292-294`). Denial does not release units. That matches `data-model.md:71`: a denial does not undo delivered care. The unique live session/service index in migration 008 blocks a same-service replacement beside a denied claim. A future versioned replacement needs explicit lineage and reservation transfer; do not replace the current guard with a blanket one-claim-per-session restriction.

A payer-configuration gap exists in the other branch: if no authorization is selected, both authorization checks pass (`readiness.rs:254-269`). For a payer that requires prior authorization, internal “ready” is then a false comfort. Authorization required versus not required must be an effective-dated rule, not a silent pass.

**Medical necessity and the chart.** Necessity is a clinical and payer-criteria question. Medicare outpatient psychiatric compliance guidance expects an individualized plan and services that are reasonable and necessary; it is aimed at physicians and non-physician practitioners billing outpatient psychiatry, and it points billers to local MAC articles rather than one national code list. [CMS reference](https://www.cms.gov/training-education/medicare-learning-networkr-mln/compliance/medicare-provider-compliance-tips/outpatient-psychiatric-care). A commercial or Medicaid plan may use its own hour thresholds, ASAM or equivalent placement criteria, or a state manual. Those instruments are licensed or state-specific. Nexus must not infer them. The existing form engine already refuses that inference (`api/src/applicability.rs:4`, `13`). Billing packets should copy that disclaimer, not invent a new one that sounds like a coverage determination.

**Attendance.** For the current prototype, billable readiness requires a completed, ended encounter. Existing `clinical.rs:10–15` maps attended to completed, no-show to no-show and excused to cancelled. Preserve and test this connected gate; a missing separate attendance query in this packet does not establish absence of attendance protection. `readiness.rs:140-172` requires session status completed and an ended session. Group care already has separate attendance (`data-model.md:51`). A group note must not create a charge for a person marked absent, excused, or cancelled. Discharge already cancels future scheduled sessions (`workflow.rs:270-271`). That cancellation must remain non-billable.

**Signed note.** Readiness requires any signed note on the session (`readiness.rs:175-192`). It does not require time in and out, modality, participants, a diagnosis supported by the note, the billing clinician’s credential, or a treatment-plan link. Signing is blocked until the session is completed and ended (`workflow.rs:33-47`). Amendments create a new draft linked to the signed original (`workflow.rs:85-104`). Nothing in the provided workflow reopens or blocks the claim when the note is amended. Proposed improvement: a materially relevant signed amendment should create a billing-impact review task while preserving the original and any submitted claim history. Merely opening an unsigned amendment draft does not invalidate the signed original.

**Service coding.** The claim stores one service code, one diagnosis code, units, and amount in cents (`data-model.md:69`, `77`). Presence is a non-empty string (`readiness.rs:43-45`, `217-251`). There is no code-set validation, modifier, place of service, rendering provider, or second diagnosis. If a program service snapshot exists, the claim code must match it (`readiness.rs:233-237`). That is the right direction: clinical snapshot first, billing interpretation second, human review before ready. Do not let a clinician edit the payer code just to clear a warning.

**Claim construction, not submission.** Professional outpatient therapy is usually a CMS-1500 / 837P with a place-of-service code. Institutional PHP, hospital outpatient, and many CMHC programs are a UB-04 / 837I with type of bill, revenue code, and condition code. CMS maintains place-of-service codes for professional claims. [CMS reference](https://www.cms.gov/medicare/coding-billing/place-of-service-codes). Official descriptions distinguish office (11), partial hospitalization (52), CMHC (53), residential substance-abuse treatment (55), psychiatric residential treatment (56), non-residential substance-abuse treatment (57), and non-residential opioid treatment (58). [CMS data reference](https://bluebutton.cms.gov/resources/variables/line_place_of_srvc_cd/). Place of service does not replace type of bill. A freestanding clinic and a hospital outpatient department can deliver similar therapy and still be different claim types, enrollment records, and fee schedules.

NUCC maintains the 1500 instruction manual; the current published release noted on its site is Version 13.0, July 2025. [NUCC reference](https://www.nucc.org/index.php/1500-claim-form-mainmenu-35/1500-instructions-mainmenu-42). NUBC owns the institutional data set. Do not hardcode either manual into the prototype. Store “claim form family” as configuration.

**Submission and acceptance.** For the prototype, “record submission” may remain a human-entered external reference (`workflow.rs:206-224`). Label it “recorded by staff,” not “accepted.” A clearinghouse adapter should store outbound control numbers and the actual receiver-specific acknowledgment type, payload and semantics. Where supplied, 999 transaction acknowledgments and 277CA claim acknowledgments are separate; some receivers, including Claim.MD, return other formats instead. Replacement and void, after a payer has the claim, are not the local void in `workflow.rs:168-190`. Institutional frequency code 7 means replacement of a prior claim and 8 means void/cancel; a replacement generally must carry the payer’s claim control number. [CMS reference](https://www.cms.gov/Regulations-and-Guidance/Guidance/Transmittals/Downloads/R1915CP.pdf). Payers differ on when they accept a frequency code versus an appeal. Local void of an unsubmitted draft must never be sent as an 837.

**Adjudication, posting, bank, patient balance.** A denial reason typed by staff (`workflow.rs:226-240`) is not a CARC/RARC pair. HIPAA requires standard adjustment and remark codes on electronic remittances, not proprietary payer essays. [HHS reference](https://www.hhs.gov/guidance/document/operating-rules-eft-and-remittance-advice). Posting should allocate payer paid, contractual adjustment, patient responsibility, and other adjustments in cents, then match the 835 trace number to a bank deposit. `workflow.rs:106-151` can reverse a whole payment and reopen status. The data model is explicit that a reversal does not refund or move money (`data-model.md:73`). Accounting CSV aggregates cash and adjustments without client identifiers (`exports.md:7-15`). That extract is a reconciliation aid, not a balanced journal and not a de-identification determination.

**Appeals and refunds.** An appeal disputes an adjudicated decision. A corrected claim replaces bad data. Medicare’s first appeal is a redetermination, generally 120 days from receipt of the initial determination, sent to the contractor that decided the claim; minor errors are reopenings, not appeals. [CMS reference](https://www.cms.gov/medicare/appeals-grievances/fee-for-service/first-level-appeal-redetermination-medicare-contractor). Commercial and Medicaid appeal clocks differ. Overpayment return is a separate legal duty: the applicable identification, investigation, reporting and return deadlines must be evaluated under the current rule and contractor process; do not turn a simplified research summary into a universal timer. [eCFR reference](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-A/part-401/subpart-D). Do not build a “refund button” that marks a claim paid-reversed and calls that compliance.

## 4. What the packet must be allowed to vary

No single outpatient packet is lawful or payable everywhere. Configure by payer, plan, state, care level, claim form, and effective date. At minimum, these artifacts change:

- Eligibility and benefits evidence: portal, 271, or attested phone call. A 271 is Medicare-specific when it comes from HETS; other payers have companion guides.
- Authorization: none, notification only, initial, concurrent review. Units may be visits, hours, days, or dollars.
- Clinical documents: assessment, treatment plan, progress note, group individualization, attendance or hours log, physician certification. Medicare IOP, for hospital outpatient departments, CAHs, and CMHCs, is not ordinary office therapy. Effective January 1, 2024, Medicare uses condition code 92, pays qualifying OPPS providers a per diem, and expects a physician certification, at least nine therapeutic hours per week on the plan of care, and multimodal treatment. PHP claims use condition code 41 and must not overlap IOP. [CMS reference](https://www.cms.gov/files/document/mm13222-new-condition-code-92-billing-requirements-intensive-outpatient-program-services.pdf) and [Medicare reference](https://www.medicare.gov/coverage/mental-health-care-outpatient-intensive-outpatient-program-services). Medicare PHP is covered only in specified facility types and requires certification that the person would otherwise need inpatient treatment. [Medicare reference](https://www.medicare.gov/coverage/mental-health-care-partial-hospitalization). Those rules do not define commercial HCPCS per diems and must not be applied to a freestanding outpatient launch by default.
- Claim identity: 837P versus 837I; rendering, supervising, and billing provider; taxonomy; place of service versus type of bill, revenue code, and condition code.
- Consents: HIPAA treatment, payment, and operations is not the same as a Part 2 consent, a recording consent, or a consent for legal proceedings.
- Financial liability notices: Medicare has assignment and limitation-on-liability rules. Other payers use contracts. Do not show one “ABN” for every patient.
- Attachments: many payers still want a PDF through a portal. The electronic attachment transaction is not a substitute the prototype should pretend to send.
- Medicaid: behavioral health is not one federal benefit. States must cover mandatory categories and may cover optional ones, so rehabilitative and many SUD services vary by state plan, waiver, and managed-care contract. [Medicaid reference](https://www.medicaid.gov/medicaid/benefits/mandatory-optional-medicaid-benefits). EPSDT covers eligible Medicaid enrollees under 21. Nexus’s adult-first population includes ages 18–20, so relevant payer discovery cannot defer this issue until a minor workflow exists. Apply the actual eligibility/category/state requirements: [Medicaid reference](https://www.medicaid.gov/medicaid/benefits/early-and-periodic-screening-diagnostic-and-treatment).
- OTP and residential: Medicare OTP has its own institutional and professional billing paths, including POS 58 for professionals, and office-based clinicians cannot use OTP codes for office treatment. [CMS reference](https://www.cms.gov/medicare/payment/opioid-treatment-program/billing-payment). Residential room and board is often outside medical coverage even when counseling is covered. Leave both unconfigured.

## 5. Missed visits are not claims

Do not release or submit a clinical charge for a service not furnished. Preserve no-show/cancellation/excused attendance as records. A draft administrative record is distinct from a billable service; any permitted missed-appointment fee must remain a separately classified patient-fee workflow. CMS policy allows a physician or supplier to charge a Medicare beneficiary for a missed appointment only as a missed business opportunity, only if non-Medicare patients are charged the same amount under the same policy, and the charge must not be billed to Medicare. Claims submitted anyway are denied. [CMS reference](https://www.cms.gov/outreach-and-education/medicare-learning-network-mln/mlnmattersarticles/downloads/mm5613.pdf).

That is not a national patient-fee rule. A commercial contract or a state Medicaid manual may forbid the fee. Contract-specific restrictions and the Medicare policy conditions need reconciliation by the operator’s billing/legal reviewers; do not infer a universal all-payer fee rule here. The prototype needs a separate patient-fee record: policy version, notice date, attendance status, amount, waiver reason, and a hard block against claim generation. Do not put the fee in `claims` or in the claims CSV.

## 6. Readiness packets: reuse the form-rule pattern

`applicability.rs` is the right design seed: organization-authored rules, stage and care level, required versus suggested, inactive rules visible as configuration blockers, snapshots preserved, and wording that does not say “legally required.” Billing readiness should become a versioned packet, not the fixed eight checks in `evaluate`.

Proposed model, not implemented:

- Packet version has payer, plan, state, care level, claim form, effective and termination dates, and a source citation the customer or a billing lead entered.
- Each check has a code, severity (block, warn, info), owner role, deterministic inputs, and a message template that says “required by this organization’s rule” unless a reviewed payer manual is attached.
- Evaluation is pure over locked rows, as `evaluate` already is. Same function for display and for the ready transition (`workflow.rs:345-354`).
- Autofill may copy subscriber ID, facility NPI, service snapshot, diagnosis, and units. A human confirms before ready. Autofill never selects a code the program snapshot does not contain.
- Override is allowed only for warn-level, non-clinical checks, with reason, actor, and expiry. Session completed, ended, and individually attended cannot be overridden. A missing signed note cannot be overridden.
- Changing a packet version does not rewrite a claim already marked ready. The claim stores the packet version and evidence snapshot it passed; current validity is re-evaluated at release without rewriting that history.
- “Ready” remains internal. Add explicit later states only when the evidence exists: recorded submission, file accepted, claim accepted, adjudicated, posted, deposit matched. Do not collapse these into `submitted` and `paid`.

Wording now overclaims. “The policy must cover the session date” (`readiness.rs:74-75`) sounds like a coverage determination. Safer: “This organization’s rule requires an active policy whose effective dates include the facility service date. This is not a payer benefits decision.” Legal review still has to approve customer-facing text. This report is not that approval.

## 7. Entities absent from the provided source

Present and useful: organization, facility, staff, client, episode, session, group attendance, note and amendment, policy, authorization, claim, payment, payment reversal, consent attestation, program service snapshot, form rules, audit, CSV export.

Absent for a correct billing workflow: payer and plan; trading-partner and companion-guide version; eligibility and benefits verification; coordination-of-benefits order; authorization requirement rule; claim version and frequency code; service line as its own entity; modifier, place of service, type of bill, revenue code, condition code; rendering and supervising provider; diagnosis rank beyond one code; attachment metadata; 999, 277CA, 276/277 status, 835, and CARC/RARC; EFT trace and bank deposit; patient responsibility and patient payment plan; no-show fee policy and charge; appeal and reconsideration; refund and overpayment case; packet version and override; secondary claim. `data-model.md:77` also confirms no separate psychotherapy or SUD counseling-note store.

## 8. Nov 13 acceptance scenarios

Live 837/835 connectivity is lower priority for this deadline; the owner has not removed internal claim-file/version history, appeals, manual allocation or reconciliation from the planned prototype scope. These scenarios are in scope because they are workflow correctness.

1. Completed outpatient session, ended, signed note, active policy covering the facility date, non-empty service and diagnosis codes, with the service code matching the program snapshot: checklist passes; only a writer can mark draft ready; UI still says this does not transmit.
2. Same chart with the session scheduled, no-show, cancelled, or not yet ended: no claim can become ready. A no-show fee, if configured, never creates a claim.
3. Group session: only attended members can reach readiness. Excused and absent members cannot release a charge as delivered care; any administrative draft remains distinct from a billable delivered service.
4. Authorization rule off: missing authorization is informational, not a silent pass that reads like coverage. Authorization rule on: missing, wrong policy, wrong code, inactive, out of dates, or insufficient remaining units blocks ready.
5. Two claims cannot duplicate the same delivered service/charge lineage or reserve its authorization units twice; distinct valid service lines must remain possible. A denied claim keeps its unit consumption unless a documented replacement path explicitly transfers it.
6. Sign a materially relevant amendment after readiness: create an impact-review task naming the changed evidence. Preserve submitted claim history; do not automatically return submitted claims to draft or treat an unsigned amendment draft as invalidating the original.
7. Record submission stores a staff-entered reference and does not change the checklist into “accepted.” Deny, pay, and reverse use cents, require reasons, and do not describe the result as an 835 or a bank deposit.
8. Billing and executive roles still cannot read note narratives. Accounting CSV still has no client identifiers. Claims CSV still carries the internal-review marker.
9. A second care level, even if only “IOP, unconfigured,” does not reuse the outpatient packet. The screen says the packet is not configured, rather than passing office-visit checks.
10. Restart the app. Ready status, packet version, override, and payment reversal are still there. Restricted roles fail at the API.

Coverage must be mapped against existing tests before adding cases. Existing PostgreSQL scenarios already cover authorization exhaustion, reversals and ready/draft/void/replacement paths (`api/tests/postgres.rs`). The worker did not execute them, and its limited packet could not establish blanket absence of coverage.

## 9. Privacy obligations, without any certification claim

Part 2 applies to federally assisted substance-use programs and to some recipients of their records, not automatically to every counseling note. [HHS reference](https://www.hhs.gov/hipaa/part-2/index.html). Compliance with the 2024 final rule was required by February 16, 2026. A single applicable consent may cover future treatment, payment and operations. HIPAA covered entities and business associates receiving records under that consent may redisclose under the applicable HIPAA/Part 2 conditions, with restrictions on proceedings against the patient. Separately maintained SUD counseling notes generally require separate consent, subject to enumerated exceptions such as the originator’s treatment use; that consent cannot be a condition of treatment. [eCFR reference](https://www.ecfr.gov/current/title-42/chapter-I/subchapter-A/part-2/subpart-B/section-2.11) and [eCFR reference](https://www.ecfr.gov/current/title-42/section-2.31). The product must be able to keep a progress note used for billing apart from a voluntarily separated counseling note. It cannot do that today (`data-model.md:77`).

Design obligations for the prototype: consent scope, recipient category, purpose, date, revocation, and redisclosure notice are recoverable; payment disclosure is a purpose, not a waiver; billing users do not receive counseling-note text; exports do not claim de-identification; `x-nexus-actor` remains a non-production identity (`data-model.md:47`). No screen should say Nexus is HIPAA-compliant or Part 2-compliant.

## 10. Priorities

Priority proposals for the functional plan: configure when authorization is required; preserve and verify existing non-attendance readiness gates; surface material amendment impact; label manual external events honestly; distinguish configured and unconfigured payer packets for every enabled program. These are proposed changes, not completed fixes.

Next, still before any payer connection: packet versions, line-level fields held as data even if unused, patient-fee ledger, and a manual posting worksheet that records paid, adjustment, and patient responsibility without parsing an 835.

Separate connectivity from internal scope. Clearinghouse enrollment, live eligibility/submission/remittance exchange, payment movement and external calendar synchronization remain lower priority. Local claim/version/file history, appeals, manual remittance allocation, statements and reconciliation remain in the owner’s planned capability scope; this report cannot unilaterally defer them beyond November 13.

## 11. Unresolved questions

Which state, licenses, and claim form will the first outpatient site actually use? Which payers, and which of them carve behavioral health out? Does any current payer require authorization for ordinary outpatient counseling? Who bills: the clinic, a clinician group, or an outside biller? Are notes progress notes only, or will anyone keep separate psychotherapy or SUD counseling notes? May the center charge no-show fees under every contract it holds? What does “paid” mean in the owner’s weekly meeting: charges, allowed amount, posted payments, or bank deposits?

## Primary sources

Applicability is limited to the program each document governs. Medicare manuals do not bind commercial or Medicaid plans. Companion guides add payer-specific constraints on top of the TR3s, which are licensed from X12 and are not reproduced here.

1. [CMS reference](https://www.cms.gov/priorities/key-initiatives/burden-reduction/administrative-simplification/hipaa/adopted-standards-operating-rules)
2. [eCFR reference](https://www.ecfr.gov/current/title-45/subtitle-A/subchapter-C/part-162/subpart-K)
3. [eCFR reference](https://www.ecfr.gov/current/title-45/subtitle-A/subchapter-C/part-162/subpart-P/section-162.1602)
4. [CMS reference](https://www.cms.gov/priorities/key-initiatives/burden-reduction/administrative-simplification/transactions/health-care-payment-remittance-advice-electronic-funds-transfer)
5. [HHS reference](https://www.hhs.gov/guidance/document/operating-rules-eft-and-remittance-advice)
6. [CMS reference](https://www.cms.gov/files/document/guidance-letter-vcc-eft-era.pdf)
7. [CAQH reference](https://www.caqh.org/core/operating-rules)
8. [CMS reference](https://www.cms.gov/files/document/current-hets-270-271-companion-guide.pdf)
9. [CMS reference](https://www.cms.gov/files/document/r13954cp.pdf)
10. [CMS reference](https://www.cms.gov/medicare/coding-billing/place-of-service-codes)
11. [CMS data reference](https://bluebutton.cms.gov/resources/variables/line_place_of_srvc_cd/)
12. [NUCC reference](https://www.nucc.org/index.php/1500-claim-form-mainmenu-35/1500-instructions-mainmenu-42)
13. [CMS reference](https://www.cms.gov/Regulations-and-Guidance/Guidance/Transmittals/Downloads/R1915CP.pdf)
14. [CMS reference](https://www.cms.gov/training-education/medicare-learning-networkr-mln/compliance/medicare-provider-compliance-tips/outpatient-psychiatric-care)
15. [eCFR reference](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-B/part-411/subpart-A/section-411.15)
16. [Medicare reference](https://www.medicare.gov/coverage/mental-health-care-outpatient-intensive-outpatient-program-services)
17. [Medicare reference](https://www.medicare.gov/coverage/mental-health-care-partial-hospitalization)
18. [CMS reference](https://www.cms.gov/files/document/mm13222-new-condition-code-92-billing-requirements-intensive-outpatient-program-services.pdf)
19. [CMS reference](https://www.cms.gov/medicare/payment/opioid-treatment-program/billing-payment)
20. [CMS reference](https://www.cms.gov/outreach-and-education/medicare-learning-network-mln/mlnmattersarticles/downloads/mm5613.pdf)
21. [CMS reference](https://www.cms.gov/medicare/appeals-grievances/fee-for-service/first-level-appeal-redetermination-medicare-contractor)
22. [eCFR reference](https://www.ecfr.gov/current/title-42/chapter-IV/subchapter-A/part-401/subpart-D)
23. [Medicaid reference](https://www.medicaid.gov/medicaid/benefits/mandatory-optional-medicaid-benefits)
24. [HHS reference](https://www.hhs.gov/hipaa/part-2/index.html)
25. [eCFR reference](https://www.ecfr.gov/current/title-42/chapter-I/subchapter-A/part-2/subpart-B/section-2.11)
26. [eCFR reference](https://www.ecfr.gov/current/title-42/section-2.31)