# Nexus research, planning and review — November 4, 2026

Status: the first research and readiness review completed; the subsequent [$500 expanded pass](2026-11-04/expanded-pass.md) and its final 20-request fleet are closed. The expanded run has **$206.81231595** in confirmed charges across 126 recorded requests, **$278** in unresolved cost reservations and zero active requests. The $500 confirmed-spend target was not reached. Ten substantive first-pass Grok reports were returned and synthesized with coordinator/source checks and OpenAI core review. Findings distinguish inspected source, prior test evidence and proposals; the application itself remains incomplete. Earlier failed/invalid requests and unresolved costs are retained in the run evidence.

## Direction recorded

The owner wants Nexus to work end to end before a separate branding implementation pass. Friday **November 13, 2026** remains the target for all planned prototype features; most external integrations are lower priority. This is an ambitious target, not a validated completion estimate. The [50-capability ledger](../product/acceptance-ledger.md) remains authoritative for scope, with 38 Partial, 11 Missing and 1 Designed at the reviewed baseline.

The first customer priority is **outpatient**. The owner also wants other care settings supported; the second setting, state and payer mix remain open. Existing adult outpatient/IOP/PHP scope must not be confused with implemented residential, pediatric or inpatient capabilities. Payer and setting differences belong in explicit versioned policies, not assumptions embedded in generic forms.

All subordinate research/implementation workers use Grok. OpenAI/Codex coordinates and performs core reviews. The owner raised the research/readiness ceiling from $40 to **$1,500**, explicitly encouraging useful depth without treating the ceiling as a target. The credential is retrieved privately from macOS Keychain. The authenticated model list included Grok 4.7, and API metadata confirmed that key `nvim-grok` belongs to the exact team in the owner's console link. No credential was displayed, saved or included in source packets.

The expanded pass adds [cross-module implementation contracts](2026-11-04/expanded-contracts.md), with rejected worker shortcuts, financial event boundaries, clinical/access/media requirements and branding corrections. Its [checkpoint](2026-11-04/expanded-pass.md) records implementation and validation progress separately from this first-pass synthesis.

See the [final working-session handoff](2026-11-04/final-handoff.md) for the running application, integrated fixes, observed model-quality failures and closeout evidence.

## Detailed reviewed reports

| Report | Contents |
|---|---|
| [Feature reality](2026-11-04/feature-reality.md) | What works locally, what models run, video/recording boundaries, remaining capability acceptance |
| [Engineering and large-commit review](2026-11-04/engineering-review.md) | Architecture/storage, confirmed source risks, rejected worker findings, structural priorities and safe change slices |
| [Billing lifecycle](2026-11-04/billing-lifecycle.md) | Client-to-cash workflow, payer-specific readiness, authorization, documentation, corrections and privacy applicability |
| [Billing/RCM competitors](2026-11-04/billing-competitors.md) | Kipu/Avea, AZZLY/Ease, Behave, Alleva, Waystar, Claim.MD and SimiTree; workflow lessons and demo questions |
| [Outpatient billing contract](2026-11-04/outpatient-billing-contract.md) | Proposed records, deterministic autocomplete, a balanced financial example and acceptance scenarios |
| [Executive observability](2026-11-04/executive-observability.md) | Twenty defined measures, cash versus charges, missing inputs, customization and permission-safe drilldowns |
| [Brand and components](2026-11-04/brand-and-components.md) | Nature/growth direction, exploratory palettes, firmer geometry, component states, typography/icons and motion |
| [Paid logomark research test](2026-11-04/logomark-paid-test.md) | Actual call receipt, concepts, scorecard, variants and validation protocol |
| [xAI run evidence](2026-11-04/xai-run-evidence.md) | Per-call IDs/costs, usable versus invalid reports, failures and unresolved reservations |

The explicit paid logo test completed November 4 at 4:07:46 p.m. Eastern. xAI reported **$0.927428**, 32 web-search calls and response ID `1ce05b78-b5d4-9292-a122-83f6d807f7ee`. The earlier smoke test only listed models; it was not a paid generation test. At that first-pass checkpoint the console browser was signed out. Later in the expanded pass, Brave access was enabled and console usage plus request-log exports were verified; see the [expanded-run evidence](2026-11-04/expanded-run-evidence.md). API per-request cost evidence is real; interrupted requests remain unreconciled rather than assumed free. Total returned provider charges across the pass are **$7.476206**; nine earlier requests retain **$225 in unresolved internal reservations**, not confirmed spending. No Grok calls from that first pass remained running at its close. The expanded pass has separate accounting.

## What is real today

| Question | Evidence-based answer | What remains |
|---|---|---|
| Where is data stored? | PostgreSQL 17, accessed by SQLx from the Rust API. Twenty-one migrations now govern 52 application tables; the recorded 53-table restore includes SQLx metadata. Both local workspaces run migration 021 with all application-table contents preserved. This Mac's actual database cluster lives in ignored `.local/postgres`. | Production identity, access scope, protected operations/backups/key recovery, pagination, capacity evidence and lifecycle controls. SQLite is not inherently unprofessional, but it is not the database in this application. |
| Is speech our own model? | The adapter/workflow is Nexus code. Speech recognition uses OpenAI's open-source Whisper base.en through whisper.cpp on this machine. It does not require the hosted OpenAI API and is not a Nexus-trained model. | Clinical accuracy evaluation, challenging audio/devices/speakers, and complete Mac saved-media acceptance. [Whisper](https://github.com/openai/whisper), [whisper.cpp](https://github.com/ggml-org/whisper.cpp). |
| What creates note drafts? | Local Qwen3-4B through llama.cpp, from approved source evidence, followed by human review and a separate signing step. The official model card identifies Apache-2.0 licensing. | This is a third-party model, not validated autonomous clinical judgment. Broader accuracy/provenance/lifecycle evaluation remains necessary. [Qwen model card](https://huggingface.co/Qwen/Qwen3-4B-GGUF). |
| How real is video? | Native WebRTC and persisted signaling/participation controls exist. November 2 evidence includes a local three-participant browser media check. | Current Mac hardware/browser/media acceptance, separate-network/TURN behavior, production identity and load testing. |
| Are recordings real? | Consented own-device tracks are encrypted and stored in PostgreSQL; playback, transcript review and restored-track decryption with the same key have prior Windows evidence. | This is not automatic complete capture of every participant. Losing the separate key is not covered by a database restore. Production storage/key rotation/retention and derivative destruction remain open. |
| Is billing real? | Local readiness checks, claim transitions, manually recorded submission references, payments/reversals and CSV exports operate on stored records. | No claim that a payer actually received/adjudicated a submission, no live ERA processing or bank reconciliation. The current claim model has one service per claim. |
| Is a premade UI kit used? | Runtime frontend dependencies are React and React DOM. Components, CSS and inline SVG are in the repo; no shadcn/ui/Radix/Tailwind component system was found. | Custom implementation does not establish usability or accessibility. The owner rejects the current visual treatment. |

The ordinary workspace remains at [local setup](http://127.0.0.1:5173); the [synthetic walkthrough](http://127.0.0.1:5174) uses a separate database. Both frontends and API health endpoints responded during this review. These are local development surfaces, not publicly deployed applications.

## Evidence and architectural priorities

The source baseline is `453c280`, the November 2 program integration commit: 54 changed files, 7,181 insertions and 218 deletions. It connects versioned services, admission decisions, facility adoption, required forms and encounter snapshots to scheduling and billing. The current local branch is `codex/prototype-readiness`; uncommitted work is preserved.

Mac baseline checks passed 83 frontend tests, 93 Rust unit tests and 16 PostgreSQL integration scenarios, plus frontend build and selected browser journeys. Two reproduced regressions have local fixes and focused evidence: optional catalog values serialized as empty strings instead of null; equivalent appointment timestamps compared incorrectly as strings. At that checkpoint the frontend suite grew to 84 tests; the closeout suite has 89 passing. Actual consented synthetic audio and a local evidence-linked draft also passed API checks. See [verification](../development/verification.md) for platform, timing and boundaries; this review does not promote all 50 capabilities to complete.

Core review identified these structural priorities:

1. **Access:** local staff headers are not authentication. Primary-clinician and organization/resource permissions are implemented, but facility, care-team, patient/proxy and disclosure scope need explicit contracts and tests.
2. **Persistence and scale:** PostgreSQL is a sensible foundation. Bootstrap loads authorized collections and computes aggregates in memory; generic mutations serialize by organization. Add measured pagination/query and concurrency work before promising large-organization scale. A microservice rewrite is not justified by these observations.
3. **Financial records:** robust dashboards require claim lines/versions/attempts, adjudication, remittance allocation, corrections and reconciliation records. New cards cannot compensate for absent source events.
4. **Recovery and lifecycle:** restore database plus matching media key; distinguish this from machine/key-loss recovery. Define source/derivative correction, retention, holds and destruction before claiming full lifecycle support.
5. **Delivery controls:** CI previously omitted this integration branch. Local workflow changes add the active branch and `codex/**` pushes plus manual dispatch. GitHub execution, branch protection, the legacy security workflow and publication gating still need their own evidence.

The strongest new financial correctness finding is inconsistent service-code normalization: catalog matching can accept case variants while the live-claim duplicate guard keys the raw code. The later expanded pass reproduced this in disposable PostgreSQL tests, integrated migration 021 and shared matching, and verified upgrade/rollback plus HTTP correction scenarios. Both local workspaces now run the fix with their stored application records preserved. The separate cents double-conversion defect in a preselected claim initializer was fixed and tested; current create callers do not mount that initializer. The [engineering review](2026-11-04/engineering-review.md) explains both and records rejected Grok claims about admission capacity, decision evidence and WAV disclosure.

## Billing: the workflow to build around

The product should help staff move from verified patient/coverage information to documented delivered care, reviewed charges, payer processing and reconciled cash. Each stage needs its own evidence and next action:

**Registration → coverage/eligibility evidence → authorization where required → scheduled care → attendance/delivery → signed documentation → reviewed charge/readiness → submission attempt → acceptance or rejection → adjudication/payment advice → posting and corrections → deposit reconciliation/patient follow-up.**

A readiness packet is an organized evidence bundle, not a universal insurance form and not a payment guarantee. Configure requirements by payer/product, care setting, service, provider and effective date. Reuse verified identity, coverage, authorization, attendance and signed-document facts with source/version/date provenance. Show conflicts or missing facts instead of silently filling them. Model assistance may draft reviewable narrative; it must not invent attendance, signatures, codes, authorization or financial facts.

Record a no-show explicitly and prevent billing it as a delivered clinical service. Any patient missed-appointment fee is a separate policy question. CMS permits certain direct missed-appointment charges under its stated Medicare conditions; this does not authorize billing Medicare for a service that did not occur or establish a universal Medicaid/commercial rule. [CMS Claims Processing Manual, chapter 1, section 30.3.13](https://www.cms.gov/Regulations-and-Guidance/Guidance/Manuals/downloads/clm104c01.pdf).

CSV and document exports are useful handoffs and supporting evidence. They do not replace the structured claims/remittance lifecycle. An ERA explains a payer's claim payment; EFT moves funds, and a trace number supports matching the two. Nexus should distinguish received advice, posted allocation and reconciled bank cash. [CMS EFT/ERA guidance](https://www.cms.gov/priorities/key-initiatives/burden-reduction/administrative-simplification/transactions/health-care-payment-remittance-advice-electronic-funds-transfer).

Public competitor documentation gives useful workflow evidence: Kipu/Avea connects insurance and utilization review with documented services, a billing-audit step, attendance review and claim creation in a work center. Claim.MD exposes remittance files, linked claims, payment identifiers and follow-up states. These are documented product behaviors, not proof of independent performance or a reason to copy their visual design. [Kipu workflow](https://kipuhealth.zendesk.com/hc/en-us/articles/9230944622875-Kipu-EMR-Kipu-RCM-Integration-Workflow), [Claim.MD ERA workflow](https://docs.claim.md/v1/docs/view-era).

## Executive reporting: actionable observability

Separate operational delivery, receivables and actual cash. Every metric needs a definition, population/time basis, source records, freshness and a permission-scoped drilldown. An owner should be able to see where work or money is stuck and who can act. Charge totals are not revenue earned; cash receipts are not profit; readiness is not reimbursement.

Prioritize attendance/no-shows, unsigned documentation, delivered-but-unbilled services, expiring authorization, claim rejection/denial queues, aging receivables, posted receipts and unmatched deposits. Capacity requires real slot/bed denominators; outcomes require a selected measure and valid follow-up cohort. Formal CCBHC measures apply to their specified programs and populations, not automatically every outpatient practice. [SAMHSA quality-measure guidance](https://www.samhsa.gov/communities/certified-community-behavioral-health-clinics/guidance-and-webinars).

Dashboard customization should change layout, filters and saved views within existing permissions. Executive status does not automatically authorize every clinical narrative. HHS describes role-based workforce access in its minimum-necessary guidance. Part 2 applicability and consent/disclosure rules also need explicit review for SUD records; HHS states compliance with the 2024 final rule was required by February 16, 2026. These are design obligations, not a certification claim for Nexus. [HHS minimum necessary](https://www.hhs.gov/hipaa/for-professionals/faq/minimum-necessary/index.html), [HHS Part 2](https://www.hhs.gov/hipaa/part-2/index.html).

## Brand and interaction direction

Preserve the owner's detailed [walkthrough observations](../product/walkthrough-review-2026-11-04.md). The profile goes **bottom left beneath navigation**, with appearance and a clear exit/sign-out action. Establish neutral light/dark surfaces, muted green accents, readable semantic status colors plus labels/icons, firmer corners and a calmer information hierarchy. Logo variants, typography/icon licensing, background motifs, component states and motion guidelines belong in the later brand foundation.

Nature/growth should appear in restrained marks, subtle patterns and short intentional feedback. Avoid placing decoration behind dense data or making critical states depend on animation or color. Person previews need keyboard/touch equivalents and permission-filtered content; desktop hover must not be the only route. A chronological audit remains understandable even if a branch/tree motif is explored.

Target WCAG 2.2 AA and deliberately support reduced motion. The AA pointer-target criterion is 24×24 CSS pixels with specified exceptions; a larger touch design target is a product choice. Interaction-animation disabling is an AAA criterion, which Nexus can choose to exceed AA on. Do not claim conformance from a palette or checklist alone. [W3C target size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html), [W3C interaction animation](https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html).

## Durable working foundation delivered

- [AGENTS.md](../../AGENTS.md) gives future repository tasks a concise entrypoint.
- [Central context](../PROJECT_CONTEXT.md) records product direction, architecture, runtime, evidence and open gaps.
- [Codex/OpenAI + Grok orchestration](../development/orchestration.md) adapts the supplied reference without importing unrelated permissions.
- [Branch/review workflow](../development/git-workflow.md) defines scoped work, shared-file ownership, acceptance and handoff practice.
- [Grok tooling](../development/xai-workers.md) documents private Keychain loading, selected source packets, actual-cost accounting, bounded calls and reviewed proposals.

The review produced working-context documents, source-backed research, reviewed engineering proposals, safer Grok tooling and a local CI trigger correction. It did not implement the proposed billing schema, executive dashboard or branding system, and it did not accept the remaining product capabilities. No commit, push, public deployment, external payer submission or design purchase was made.

## Customer validation

Use the [customer discovery and billing journeys](2026-11-04/customer-discovery-and-billing-journeys.md) to review client-to-cash behavior, a reconciled synthetic money example, biller/executive interviews and the distinction between workspace views and permissions. It captures unresolved payer/state requirements without inventing policies.

## Decisions and next work

Preserve all 50 planned capabilities. The canonical service-code/duplicate-charge issue is fixed locally; next are concrete contracts for the remaining financial events, while parallel clinical/patient-access work follows the existing scope ledger. Build dashboard calculations from those records; do not use visual cards to conceal missing inputs. Complete the functional journeys before the broad branding implementation pass.

Still needed from customer discovery: state, payer/product mix, the additional care setting, actual provider contracts and billing-team handoff preferences. These are open requirements, not a reason to stop generic internal workflow work or silently choose a payer rule. Executive interviews should validate the metric dictionary and exception queues; branding should progress through concrete concept comparisons rather than speculative application-wide restyling.
