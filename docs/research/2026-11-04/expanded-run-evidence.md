# Expanded Grok pass — provider evidence

Snapshot 2026-11-05T03:51:01+00:00; local run closed. This is not a provider invoice. Run `deep-pass-2026-11-04`, $500 target and ceiling. All amounts below are USD. Prior-pass $7.476206 confirmed and $225 unresolved nominal holds are separate.

- Provider-reported charges matched to this run: **$206.81231595**.
- Unresolved failure reservations: **$278.00**, not confirmed spending.
- Active request reservations: **$0.00**, not confirmed spending.
- Recorded requests: **126**. The updated scheduler supports 16 total requests, initially two multi-agent calls, and reduces affected-model concurrency after throttling. Slots refill independently.

The final fixed fleet recorded all 20 planned names: 12 returned reports that met the format check, 2 returned invalid/incomplete reports with receipts, and 6 closed with unknown cost (four interrupted streams and two throttled calls). The final release reviewer returned under response ID `91c56a2c-c131-9e4b-8705-44b974cc975b` for **$4.53160840**. There are no pending requests, and the controller released its lock. The run did not reach $500 in verified spending. Known charges plus nominal outstanding holds total **$484.81231595**, leaving **$15.18768405** of accounting room, not an xAI credit balance or final liability. The final fleet was bounded; unresolved calls were not retried to spend the difference. See the [reviewed final decision](final-handoff.md).

## Console verification and recovered receipts

After the owner enabled macOS permissions, the coordinator accessed Brave's xAI console for the selected `nvim-grok` key. The displayed October 29–November 4 total was $162.53 and 2,015 requests; the subsequent hourly export totaled $162.776429. Those are whole-key totals including earlier work, not the expanded run's bill. A later visible total was $167.29. Console exports remain ignored local evidence, not source packets or committed application data.

The request-log export recovered seven interrupted requests with explicit terminal usage costs. IDs, models, timestamps, exact usage, source hash and costs were independently core-reviewed. Their charges total **$6.11225750**, replacing $175 in nominal holds and releasing **$168.88774250** of reserved capacity. Original failed-stream outcomes and reservation history remain in each ledger entry's reconciliation record; recovering cost does not make a missing report successful.

Seven older HTTP 429 request-only logs had no usage object. They remain unknown, not zero. A newer throttle may also be present in the table. The hourly bucket boundaries did not reconcile with request-start cohorts, so no aggregate-based hold release was made. API ticks and downloaded log ticks are exact request-level evidence; dashboard totals must not be added to them.

## Requests

`completed` describes a provider response that met the runner's output-format check. It does not mean its findings were correct, its code was applied, or a product capability passed acceptance. Material errors and rejected proposals are recorded in the reviewed [implementation contracts](expanded-contracts.md). Invalid/truncated outcomes are still charged when the provider reports a cost.

| Task | Model | Outcome | Reported charge | Response ID |
|---|---|---|---:|---|
| deep-architecture-council | grok-4.20-multi-agent-0309 | completed | $4.02311990 | bca6c41d-8ced-986a-b439-fb31fad0854a |
| deep-canonical-service-proposal | grok-4.7 | completed | $0.29999200 | c7c0e25e-2443-9ac9-8d5b-5d1dcc3f0f6c |
| deep-claim-money-proposal | grok-4.7 | failed-stream-cost-reconciled | $0.58382600 | e291f896-d78c-9fe8-9251-41b673932fe5 |
| deep-canonical-service-implementation-v2 | grok-4.7 | completed | $0.29121600 | 21d5c3ce-ed81-99a1-b1fc-d419b6ea6f1a |
| deep-financial-ledger-council | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep-privacy-authorization-council | grok-4.20-multi-agent-0309 | completed | $4.97179380 | 22159ac5-493b-9ffc-9881-3d3d5e6b298e |
| deep-clinical-documentation-council | grok-4.20-multi-agent-0309 | completed | $4.50706500 | bb456c13-861a-9ba0-af48-32a07839f473 |
| deep-patient-chart-clinical-data-council | grok-4.20-multi-agent-0309 | completed | $4.42264130 | 5d3d24ed-3431-9cb8-9253-6427dfec6528 |
| deep-scheduling-capacity-council | grok-4.20-multi-agent-0309 | completed | $5.50153290 | 14e1415f-c531-946b-af5b-8d235651e153 |
| deep-speech-drafting-council | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep-telehealth-media-council | grok-4.20-multi-agent-0309 | completed | $4.83018150 | 56cc3fb6-44ee-9472-b454-bc689d7eb7dc |
| deep-portal-messaging-proxy-council | grok-4.20-multi-agent-0309 | completed | $5.45528080 | 71791000-c3bd-98df-9311-4447873d5ba6 |
| deep-attachments-lifecycle-council | grok-4.20-multi-agent-0309 | completed | $4.82332730 | 4cf7ad60-ffb1-939b-a393-d4c7bfa2709f |
| deep-executive-reporting-council | grok-4.20-multi-agent-0309 | completed | $5.67596570 | 2cd09d20-946c-9f34-bb86-869abfa621d5 |
| deep-operations-delivery-council | grok-4.20-multi-agent-0309 | completed | $4.63404200 | 0debf0f6-fe6b-9db1-8e58-57e58365b258 |
| deep-forms-task-ux-council | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep-billing-interchange-packets-council | grok-4.20-multi-agent-0309 | failed-stream-cost-reconciled | $0.83752000 | 217ae8aa-d3bd-9c22-817f-026e856c7374 |
| deep-brand-system-foundation-council | grok-4.20-multi-agent-0309 | completed | $5.20929180 | 9a98b1af-befe-91ce-9196-0237e931c181 |
| deep-job-recovery-implementation-council | grok-4.20-multi-agent-0309 | completed | $2.40462730 | df89ce1b-4ddb-9e65-a52c-bfba1a150351 |
| deep-financial-ledger-council-frontier | grok-4.7 | completed | $1.46316800 | 3d7e4725-ed6d-9095-9ddc-e3a6be8069c4 |
| deep-speech-drafting-council-frontier | grok-4.7 | completed | $0.29065200 | c5c4c7cd-414c-9818-bfc9-befef993cb0e |
| deep-forms-task-ux-council-frontier | grok-4.7 | completed | $0.23523800 | cbf09978-7b97-9144-8b8a-c0e7d0ef4a68 |
| deep-claim-money-implementation-v2 | grok-4.7 | failed-stream-cost-reconciled | $0.74675200 | 11216a09-e51a-93fa-a4ab-552a63a11e7d |
| deep-privacy-authorization-adversarial-review | grok-4.20-multi-agent-0309 | failed-stream-cost-reconciled | $1.04606725 | 7c1ab14b-6afa-9427-9230-a4b8e5fff37e |
| deep-clinical-documentation-adversarial-review | grok-4.20-multi-agent-0309 | completed | $5.75096730 | 58886c87-3619-9f09-a6ff-dad68e49b658 |
| deep-patient-chart-clinical-data-adversarial-review | grok-4.20-multi-agent-0309 | completed | $4.81573220 | 713df2a5-2217-9264-abb1-87291ab6afa5 |
| deep-scheduling-capacity-adversarial-review | grok-4.20-multi-agent-0309 | completed | $5.63878750 | 9db8dbd7-dcfe-93fa-99e1-f4dfd9a2cdeb |
| deep-telehealth-media-adversarial-review | grok-4.20-multi-agent-0309 | completed | $5.28223820 | e0281692-e08c-9ece-8da4-daacd9dd9cc9 |
| deep-portal-messaging-proxy-adversarial-review | grok-4.20-multi-agent-0309 | failed-stream-cost-reconciled | $1.46592225 | eba3bd01-cc10-9be3-bbfb-d6d8abfc2316 |
| deep-attachments-lifecycle-adversarial-review | grok-4.20-multi-agent-0309 | completed | $4.52732470 | 702d47ba-14f5-99e9-979f-22725659a887 |
| deep-executive-reporting-adversarial-review | grok-4.20-multi-agent-0309 | completed | $5.02847180 | 53f97337-2cd2-90e5-80aa-328a0625dda1 |
| deep-operations-delivery-adversarial-review | grok-4.20-multi-agent-0309 | completed | $4.77722870 | 55e687fc-6363-9047-ba7d-5290f77bfc29 |
| deep-brand-system-foundation-adversarial-review | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep-job-recovery-implementation-adversarial-review | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep-profile-menu-implementation | grok-4.7 | completed-invalid-report | $0.17607000 | e50a81dd-90df-99da-af54-a9cbaa1938c0 |
| deep-group-calendar-implementation | grok-4.7 | completed | $0.68590600 | 259bec6a-a8c8-93ed-b612-65c95f152702 |
| deep-identity-http-regressions | grok-4.7 | failed-stream-cost-reconciled | $0.65637600 | 554d41d6-a5b7-9523-8f07-900791944b84 |
| deep-identity-upgrade-regressions | grok-4.7 | failed-stream-cost-reconciled | $0.77579400 | 0738d9bd-01bd-9370-8d6f-240d9f3f6c67 |
| deep-profile-menu-implementation-v2 | grok-4.7 | completed | $0.41438600 | 0d02ee9b-0a1b-9813-aa27-eca02f121a4f |
| deep-cross-module-identity-contract | grok-4.7 | completed | $0.34663800 | 73773172-8b40-943c-a9d9-62423b143f9e |
| deep-financial-packet-contract | grok-4.20-multi-agent-0309 | completed | $1.69413485 | 934343ef-08d3-9870-af8e-89d2a7c82b84 |
| deep-financial-adjudication-contract | grok-4.20-multi-agent-0309 | completed | $4.55342610 | e9f23cfd-02c8-93d5-a256-81db47584e11 |
| deep-clinical-supervision-contract | grok-4.20-multi-agent-0309 | completed | $1.31669845 | e42b9872-916d-9e8f-8f24-0683fd10b8d1 |
| deep-worker-supervision-corrected-contract | grok-4.20-multi-agent-0309 | completed | $1.49909145 | 00db4e78-cbc1-9c61-967e-aa81517b9d9f |
| deep-clinical-records-first-slice | grok-4.20-multi-agent-0309 | completed | $3.71498270 | 7fdedd76-4133-9a4d-a4f2-67460addd3ed |
| deep-attachments-minimum-slice | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep-brand-interaction-review | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep-claim-money-integrated-review | grok-4.7 | completed | $0.24863000 | 46cd29c7-2d77-908d-b4b9-64ddf6963116 |
| deep-profile-integrated-review | grok-4.7 | completed | $0.29248600 | e1afa486-5d42-9e33-bc3d-60e32535c9cb |
| deep3-migration-release-review | grok-4.7 | incomplete-invalid-report | $0.44523200 | 2864821b-73a9-9093-a51f-9611d82bd519 |
| deep3-clinical-financial-integrated-journeys | grok-4.7 | incomplete-invalid-report | $0.27858800 | c510c6e7-e2ec-9f02-8bc6-91fb418c4b09 |
| deep3-executive-minimum-data | grok-4.7 | incomplete-invalid-report | $0.10781400 | 2799fce8-d04f-9a8d-9f12-85cdea5a50bf |
| deep3-clinical-care-team-scope | grok-4.7 | incomplete-invalid-report | $0.21965200 | 9298d597-58e5-9914-9658-94c9e620fd58 |
| deep3-billing-packet-autocomplete | grok-4.20-multi-agent-0309 | completed | $0.85478175 | 940e5592-38d4-94db-a660-267fc43aa7e6 |
| deep3-financial-event-state-machine | grok-4.20-multi-agent-0309 | completed | $0.83350485 | d3f74849-b228-9f2a-9948-7439bada17b2 |
| deep3-recurrence-exception-implementation | grok-4.7 | completed | $0.05727200 | 5749465b-7633-9448-a511-37a378246b1f |
| deep3-tasks-workflow-redesign | grok-4.7 | completed | $0.13488400 | f7c36e86-32f1-9b22-948b-bc9c3709a24e |
| deep3-clinical-diagnoses-records | grok-4.7 | incomplete-invalid-report | $0.26738600 | 7c45dcff-e21e-9dfe-bf7d-c0d420406681 |
| deep3-attachments-storage-slice | grok-4.7 | incomplete-invalid-report | $0.19019800 | 41db9f17-abd4-9f65-adb1-abb666568146 |
| deep3-job-recovery-hardening | grok-4.7 | incomplete-invalid-report | $0.19811000 | 57985e46-51a2-9236-af02-18e187170919 |
| deep3-draft-provenance-eval | grok-4.7 | completed | $0.17099200 | d29d04cd-7fa7-94a9-b85b-36147b85d343 |
| deep3-patient-proxy-onboarding | grok-4.7 | completed | $0.17429200 | dd592bca-0936-9ed0-b652-04bfc9e23f51 |
| deep3-messaging-minimal-vertical | grok-4.7 | incomplete-invalid-report | $0.17170000 | 761401f3-c9b8-9134-b24f-b4dce9d0ea97 |
| deep3-portable-mac-bootstrap | grok-4.7 | incomplete-invalid-report | $0.16744800 | a312828e-b91a-9c97-994b-2ce3289199ed |
| deep3-query-pagination-plan | grok-4.7 | completed | $0.10319000 | 044579a5-9d5f-90af-a221-90d2e50276d6 |
| deep3-billing-denial-workbench | grok-4.20-multi-agent-0309 | completed | $1.81072555 | 4cb7b8b3-1082-9dd9-96fe-905ec727345e |
| deep3-clinical-supervision-state | grok-4.7 | completed | $0.16801200 | 07a5c193-7666-9558-b1de-d9cfc7628696 |
| deep3-executive-metric-dictionary | grok-4.20-multi-agent-0309 | completed | $1.49740630 | 3e80bcc2-71b3-9983-980e-f28e55c9c93d |
| deep3-release-acceptance-gates | grok-4.7 | incomplete-invalid-report | $0.09466200 | 9fbed038-5c3a-9ed5-a178-1e097646f4c1 |
| deep3-calendar-provider-adapter | grok-4.20-multi-agent-0309 | completed | $1.18739785 | 4df844ef-7ecf-97b0-8e31-88f35956a0ca |
| deep3-medications-labs-boundaries | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep3-speech-quality-eval | grok-4.20-multi-agent-0309 | completed | $1.82214065 | 37bb6eae-f6da-9d91-9a46-8c373abd2fd3 |
| deep3-brand-component-contract | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep4-financial-contract-corrections | grok-4.7 | completed | $0.37372600 | 9668c6ca-de93-9fba-a1f1-115b8c8f6b87 |
| deep4-packet-contract-corrections | grok-4.7 | completed | $0.26489400 | d73505d3-aae0-9235-aaf3-c51bb953057b |
| deep4-metric-contract-corrections | grok-4.7 | completed | $0.30643200 | 23830fd6-06ac-9b4d-b370-90435934533d |
| deep4-denial-workbench-corrections | grok-4.7 | completed | $0.16092600 | 415afadd-c240-9256-b1b4-e598a6adbd79 |
| deep4-identity-api-regression-code | grok-4.7 | completed | $0.60700600 | 9bdd04ba-f189-9064-addb-feb24e1252e4 |
| deep4-worker-recovery-code | grok-4.7 | completed | $0.47759000 | 49e18ac3-fe66-9f78-92a0-0f639f1be636 |
| deep4-csv-export-regression-code | grok-4.7 | completed | $0.74799400 | 4e990036-f0fa-9af2-b3f1-b10794c628af |
| deep4-form-version-integrity | grok-4.7 | completed | $0.39854200 | fa38a1dd-fd76-9fb7-aea2-ecf5694df72d |
| deep4-production-identity-cutover | grok-4.7 | completed | $0.19882200 | 92d0a652-3814-9193-bebf-394eb667838f |
| deep4-supervision-schema-corrections | grok-4.7 | completed | $0.08482800 | d91be12a-444a-9173-bd84-f56fa6174db2 |
| deep4-medication-lab-boundaries-frontier | grok-4.7 | completed | $1.11337200 | be10ff9e-47c5-9a58-ad02-175305ca161d |
| deep4-brand-assets-frontier | grok-4.7 | completed | $1.23244800 | d8ae34ee-e116-9a1b-b5b9-c3713a129cff |
| deep4-telehealth-recovery-frontier | grok-4.7 | completed | $1.34989600 | ff3ed444-78e4-98bb-98d7-41ac33f39787 |
| deep5-drafting-evaluation-corrected | grok-4.20-multi-agent-0309 | completed | $1.93170190 | fc71fe8a-5435-9199-83d2-aa6cdfc7163d |
| deep5-portable-macos-bootstrap-code | grok-4.7 | failed-cost-unknown | Unknown / reserved | 79848f41-2948-9a75-a330-6b1e2ed3f88e |
| deep5-task-workflow-state-tests | grok-4.7 | completed | $0.34812600 | 3cc9a438-6cc1-9200-9aaf-5f69a23a46c6 |
| deep5-frontend-accessibility-contract | grok-4.7 | completed | $1.36292400 | 2f6869ca-b01e-9bdf-b3e4-4c0ff9e248ea |
| deep5-end-to-end-acceptance-scenarios | grok-4.7 | completed | $0.36602600 | c59987aa-58f1-96ba-9429-6c2f7cb9c76f |
| deep5-supervision-lifecycle-full-source | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| deep5-portal-authority-transaction | grok-4.20-multi-agent-0309 | completed | $1.45339045 | 1155566a-2527-954f-b03a-fbdf14448071 |
| deep5-attachments-vertical-design | grok-4.20-multi-agent-0309 | completed | $1.26046600 | 27156764-84cf-9e4c-9f44-920c89a8ac2f |
| deep5-care-team-field-minimization | grok-4.20-multi-agent-0309 | completed | $1.81962230 | bf67ad14-905e-9d52-9672-2c99d67c3728 |
| deep6-export-size-boundary-code | grok-4.7 | completed | $0.48532800 | 9a29a6b8-3e32-994b-a668-8566f37114a4 |
| deep6-local-drafting-evaluation-code | grok-4.7 | completed | $0.49699200 | a88b82a1-99e9-9d2a-8bb2-0aa32bab0ec7 |
| deep6-bootstrap-measurement-code | grok-4.7 | incomplete-invalid-report | $0.57416800 | 16b8cfa8-33da-926f-9cca-74b900753a31 |
| deep5-recurrence-timezone-contract | grok-4.20-multi-agent-0309 | completed | $1.26194405 | 1040687d-364f-9b14-a424-f4b8d2f8fa27 |
| deep5-calendar-sync-privacy-corrected | grok-4.20-multi-agent-0309 | completed | $1.73195530 | 880779c8-e76d-9066-9bd2-cd17ef827f60 |
| deep5-clinical-diagnosis-observation-contract | grok-4.20-multi-agent-0309 | completed | $1.40928815 | 88b50b6b-c300-9f77-99f2-c6878c2f06c3 |
| deep7-owned-terminal-recovery-code | grok-4.7 | completed | $0.79972000 | 457495b5-a53d-95d5-9ed4-38067019df58 |
| deep5-reporting-customization-boundary | grok-4.20-multi-agent-0309 | completed | $1.17925100 | 23557d97-c11b-9d56-aecb-1739eaa7ed5b |
| deep5-record-lifecycle-derivative-graph | grok-4.20-multi-agent-0309 | completed | $1.75569135 | d817c185-bb1d-9b8b-953e-10df38a79ea2 |
| deep5-outpatient-payer-discovery | grok-4.20-multi-agent-0309 | completed | $1.99403170 | cbb060d3-fbc8-9d2d-9127-0b4481056d38 |
| deep5-operations-professional-readiness | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| final-clinical-provenance | grok-4.20-multi-agent-0309 | completed | $4.91534890 | d4a26566-fe47-97d0-9075-1ea5a61e2431 |
| final-canonical-code-final | grok-4.7 | completed-invalid-report | $0.30029200 | 85cbc25b-e390-96c0-adc9-f5156fa92c37 |
| final-money-calendar-profile-final | grok-4.7 | failed-cost-unknown | Unknown / reserved | 1cee7bca-8e71-9e0d-93b6-de3e3cbd1cc2 |
| final-export-tests-final | grok-4.7 | failed-cost-unknown | Unknown / reserved | cb208384-377e-95f2-9945-eba40517a5d4 |
| final-local-evaluation-and-bootstrap-final | grok-4.7 | failed-cost-unknown | Unknown / reserved | a01de99b-25e0-9389-8df8-44e249345fd5 |
| final-orchestration-handoff-final | grok-4.7 | failed-cost-unknown | Unknown / reserved | 0e0adaf8-3bfe-9f33-ad39-278c2651983d |
| final-recurrence-and-capacity | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| final-financial-records | grok-4.20-multi-agent-0309 | completed | $4.40629180 | 7cac525c-91c5-96ea-b222-38e563bb5ad8 |
| final-executive-observability | grok-4.20-multi-agent-0309 | failed-cost-unknown | Unknown / reserved | Not returned |
| final-identity-portal-disclosure | grok-4.20-multi-agent-0309 | completed | $5.29203460 | fd29b361-e8e1-9454-8029-713a5374ac61 |
| final-worker-recovery-final | grok-4.7 | completed-invalid-report | $0.27810600 | 213d2950-82ed-96d3-a5c5-0d56cc46f4c9 |
| final-clinical-records | grok-4.20-multi-agent-0309 | completed | $1.81420975 | dfa6fc02-3ca0-9b6d-8284-7232127f2a5e |
| final-operations-recovery | grok-4.20-multi-agent-0309 | completed | $4.90097210 | 76bcbce6-dd8f-9df8-b927-63012d1fc708 |
| final-program-intake-discharge | grok-4.20-multi-agent-0309 | completed | $2.19013360 | 2a0a32ac-1ec4-9603-95c8-d60d3eaa42ee |
| final-staff-tasks-communications | grok-4.20-multi-agent-0309 | completed | $4.96196810 | aae0861b-0129-9391-ad84-e2bb5f774f12 |
| final-telehealth-recording | grok-4.20-multi-agent-0309 | completed | $5.12000230 | aeecbb14-6f62-9177-a314-83b24bede07e |
| final-attachments-lifecycle | grok-4.20-multi-agent-0309 | completed | $1.73183210 | 66be979d-1ead-9b2e-889e-1b4775b090fc |
| final-brand-components | grok-4.20-multi-agent-0309 | completed | $6.10151090 | e70cc86a-b51c-9de2-b99a-2b45931a5d52 |
| final-money-access-adversarial | grok-4.20-multi-agent-0309 | completed | $5.85092150 | f1aa98de-a2a9-91c7-a34e-aac983df8a97 |
| final-acceptance-critical-path | grok-4.20-multi-agent-0309 | completed | $4.53160840 | 91c56a2c-c131-9e4b-8705-44b974cc975b |

## Evidence handling

The shared ignored ledger is `.local/xai/usage.json`, protected by one exclusive runner lock. Console evidence includes the hourly exports and `console-requests-expanded-2026-11-04.jsonl`; SHA-256 provenance is recorded in reconciled entries. No key was printed or saved. Grok receives selected repository source and public research only; console logs, credentials, patient/runtime data and this accounting export are not included in its prompts. Model reports are proposals until source review and local validation establish their claims.

Independent accounting review found no basis to release the remaining HTTP 429 holds from the existing exports. A terminal log without usage is not an explicit zero charge. A finalized whole-key total minus distinct exact receipts included in the same finalized interval could provide a conservative group upper bound, even with unrelated activity, but these snapshots lack a completeness/finalization signal. Keep the holds until request-level costs or sufficiently complete provider billing evidence is available. Neither nominal reservations nor observed rate limits are provider-enforced dollar caps.
