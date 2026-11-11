# Nexus final working-session handoff — November 4, 2026

Status: the final Grok fleet is closed. The local walkthrough is ready for owner review; the full 50-capability prototype is not accepted. This report records actual behavior, applied changes, evidence and remaining work. It does not accept the complete prototype.

The [guided local tour](../../product/local-walkthrough.md) follows the clinical, billing and executive workspaces, settings and access boundaries.

## Open the application

- Synthetic owner walkthrough: <http://127.0.0.1:5174> (API 8091, `nexus_walkthrough`).
- Ordinary local workspace: <http://127.0.0.1:5173> (API 8090, `nexus`).
- Both APIs report healthy database connections. After rebuilding/restarting on the financial-permissions fix, all 52 application-table fingerprints matched before/after in both databases. After a refresh, the full-access Billing page showed $300 recorded charges, $100 net receipts and $50 outstanding. The browser is back on Clinical overview.

Local operator selection remains a development permission mechanism, not production authentication. Services stay on loopback with synthetic data. No commit, push, external payer submission or public deployment was made. Work remains on `codex/prototype-readiness`, based on `453c280`, with existing uncommitted work preserved.

## Applied changes

| Change | Result and boundary |
|---|---|
| Canonical service identity | Migration 021 and shared matching close the reproduced case-variant duplicate-charge path. Collision checks fail the upgrade atomically; signatures/history remain intact. Both local databases run the migration. |
| Exact claim amounts | Money initialization and interactive catalog selection convert integer cents exactly once. The preselected-session path and ordinary form path are covered. |
| Billing with limited permissions | A support operator with a claims-only exception saw $100 outstanding on an already settled $100 claim. The API now returns null when required records are inaccessible and the Billing cards display “Unavailable.” A receipts-only operator can still see net receipts; full access shows the real $0 balance. |
| Scheduling/forms | Blank optional catalog links serialize as null; equivalent RFC3339 timestamps compare as instants. Calendar parent/child grouping uses the displayed window, keeping child details accessible when the parent is outside it. |
| Bottom-left profile | Theme, permission-filtered settings and local operator exit sit beneath navigation. Existing guards for specific dirty workflows and mobile focus behavior are retained. This is not authenticated logout. |
| Worker failure recovery | Drafting and saved-media transcription retry an unsaved failure/audit transaction on the advisory-lock session before taking another job. Cancellation and terminal history are preserved; owner-connection loss cannot fall back to pooled terminal writes. Full claim/publication fencing and automatic takeover remain open. |
| Export regression | Exactly 10,000 in-scope claims export completely; 10,001 rejects the whole download without a partial CSV or extra export audit. Tenant and date exclusions do not count toward the cap. |
| Connections description | Settings now accurately distinguishes consented local WAV transcription and eligible saved visit audio; it no longer says live visit recording is disconnected. |
| Local evaluation | An opt-in test harness runs eight fixed synthetic cases through the actual local drafting engine and saves reviewable results under ignored `.local`. It does not calculate a clinical accuracy score. |
| Durable working practice | Central context, owner observations, orchestration/Git rules, acceptance ledger links, CI coverage and auditable Grok accounting are documented. New repository chats start from `AGENTS.md`. |

## Most important newly observed gap

The local Qwen3-4B run completed eight real calls in 119.19 seconds. Seven outputs passed schema/quote checks and one was rejected as invalid. **That does not mean seven outputs were clinically correct.**

The model converted an instruction embedded in a source into an unsupported diabetes assertion. It also copied symptoms into a medications section without medication evidence. Another statement was true elsewhere in its source but attached to the wrong quotation. These are observed semantic/attribution failures, not hypothetical concerns. A rejected response establishes validator rejection only; it does not prove that the model appropriately refused the request.

Existing explicit statement review, unsigned-note creation and separate signing are real safeguards. Exact quotation matching proves where text appears, not that it supports a statement. The next drafting slice should strengthen clinician evidence review/correction, preserve original and reviewed versions, and regression-test these observed failure cases. Generated text must remain separate from coded diagnoses, medication and billing facts. A prompt change alone is not a validated remedy.

## Validation

See the dated [verification log](../../development/verification.md) for exact commands, platforms and limits. Evidence includes:

- 89 frontend tests and a successful production build; the existing bundle-size warning remains.
- 103 Rust unit tests after the worker repair, strict all-target Clippy, formatting and an API binary build; five opt-in/disposable tests are ignored by default and were exercised separately.
- The 16 earlier PostgreSQL scenarios, plus five upgrade/Unicode/rollback cases, four claim-identity HTTP cases, the added export-cap case, and four worker terminal-recovery cases. These were performed at the documented change checkpoints, not represented as one simultaneous final run.
- Existing drafting and transcript consent/review PostgreSQL journeys also passed again after the worker repair. Worker tests use real audit rollback, targeted test-backend termination and HTTP cancellation of the same unsaved job between attempts. They do not run the infinite scheduler or establish full failover.
- Desktop/mobile browser checks for the profile; unit coverage for calendar grouping. The main PostgreSQL workflow now verifies claims-only, receipts-only and full-access metrics against the same paid claim.
- Nineteen mocked-network runner tests; they do not call xAI or load a real key.

All destructive integration fixtures were confined to `nexus_test`; the new focused suites used unique schemas. The earlier workflow suite recreates `public` inside that disposable database. The ordinary and owner walkthrough databases were not reset.

## Research and architecture conclusions

Nexus uses PostgreSQL through Rust/SQLx, with a React/TypeScript frontend and custom UI components. No shadcn/ui, Radix or Tailwind component library is present. Local speech uses Whisper through whisper.cpp; drafting uses Qwen through llama.cpp. These are third-party self-hosted models, not Nexus-trained models or required hosted OpenAI/xAI runtime APIs. Development Grok calls remain separate from patient workflows.

Video/signaling, consented own-device recording, transcript review and stored billing records exist. Remote-network/TURN/device acceptance, production identity, media-key loss/recovery, broader model quality and full financial reconciliation remain incomplete. A screen, build or local happy path does not establish production readiness.

Reviewed [billing research](billing-lifecycle.md), [competitor evidence](billing-competitors.md), [executive metrics](executive-observability.md), [customer discovery journeys](customer-discovery-and-billing-journeys.md), and [brand/component brief](brand-and-components.md) are preserved. Financial observability requires distinct charges, payer decisions, responsibility, receipts, allocation, corrections and bank reconciliation. Readiness is not payer approval; cash is not profit. Proposed financial records are not implemented by the current claim/payment tables.

Core review rejected material errors in final worker proposals, including an incorrect allowed-amount equation, cash claimed unchanged after an added receipt, allocation sealing that prevents later use of unapplied cash, recursive clinical source resolution and nonexistent amendment-parent uniqueness. [Reviewed contracts](expanded-contracts.md) record the corrections. Provider output was never automatically applied.

Brand implementation remains a later pass: neutral light/dark surfaces, restrained earthy green, firmer corners, custom stateful components, subtle growth motifs and motion. The existing mark and visuals are not owner-approved branding.

## Next working session

Preserve the [50-capability ledger](../../product/acceptance-ledger.md): its baseline is 38 Partial, 11 Missing and 1 Designed, with none accepted. Friday November 13 remains a target, not a delivery guarantee. Outpatient is first; the additional setting, state and payer products still need customer decisions.

The executive bootstrap can return whole authorized rows containing diagnosis or contact fields. Prioritize field minimization and internal access/disclosure contracts, the demonstrated drafting-quality failures, explicit current-evidence/amendment rules, and the financial event/receipt/allocation contract. Verified identity and professional deployment controls remain gates before external/live use; lower-priority integrations need not displace internal prototype workflows. Resolve task-assignment visibility versus completion authority and explicit task-timezone semantics. Continue the remaining clinical, scheduling, patient/proxy, attachments, messaging, medication/lab, outcomes and lifecycle capabilities without silently dropping scope. Keep broad branding implementation after functional acceptance.

Start future repository chats with [central context](../../PROJECT_CONTEXT.md), then the [research/planning review](../research-planning-review-2026-11-04.md), and only the source/contracts for the selected slice. The framework does not automatically give unrelated chats filesystem context.

## Provider accounting and closeout

The fixed final review scope is 20 tasks, including one dependent release review. Actual receipts, failures, invalid output and unknown reservations are tracked in the [expanded-run evidence](expanded-run-evidence.md). The current $500 target/ceiling is separate from the earlier research authorization. Reservations are not spending and provider response completion is not product acceptance.

Four stale streams from the interrupted controller had no progress since about 7:08 p.m. Eastern and were later closed with their holds retained. They were not retried. The remaining planned requests were dispatched with per-model completion spacing and bounded timeouts. A console receipt recheck encountered a blank logs page; no new billing evidence was inferred from it.

**Expanded run closed:** 126 requests; **$206.81231595** in provider-reported charges, **$278** in unresolved reservations across 18 requests, and **0** active. The $500 target was not reached in confirmed spending. Known charges plus reservations account for $484.81231595 of the local ceiling; $15.18768405 remains unreserved. These are local control amounts, not the provider invoice. The earlier research pass ($7.476206 confirmed, $225 unresolved reservations) is separate.

The final 20 comprised 12 format-valid completed reports, 2 invalid reports with receipts, and 6 failures without a known charge (four interrupted streams and two throttled requests). The twentieth review returned under ID `91c56a2c-c131-9e4b-8705-44b974cc975b` at **$4.53160840**. No further calls were queued; the controller exited and released its lock. The outstanding costs cannot be determined by subtracting unrelated whole-key dashboard totals.

## Reviewed release decision

**Ready for an internal synthetic walkthrough. Not ready for real patient use or an all-features-complete claim.** The November 13 target remains high risk. Keep the 50 original capabilities and the additional care setting visible. Preserve human review of drafts and demonstrate each stage on its own evidence.

| Gate | Next evidence required |
|---|---|
| Clinical evidence | One synthetic outpatient encounter with the same patient, episode and source versions: attendance or consented media → corrected transcript → reviewed unsigned draft → sign/amend → readiness recompute. Exercise unsupported diagnoses, competing signed amendments and stale/revoked inputs. No single current-Mac run establishes the whole chain. |
| Biller workflow | A documented, delivered service can become ready; an unattended service cannot. Freeze the readiness inputs in a versioned packet, identify the blocker and responsible person, and show an explicit correction without rewriting the historical claim. Receipt, adjustment, allocation, reversal, remittance and bank match must remain separate. Live exchange is lower priority, but must never be labeled verified by a manual reference. |
| Executive and access | Demonstrate that each total drills into the same authorized records, that missing denominators stay unavailable and that a restricted role never receives unnecessary fields. Current executive bootstrap can include contact/diagnosis fields; a safe field-limited drilldown remains a design and test requirement. |
| Release evidence | Record branch/revision, role, positive and denied path, persisted history after reload and a meaningful correction/failure case per capability. Verify identity, media/key recovery and the remaining ledger items before external use. Keep the broad branding pass after functional evidence. |

Core review rejected the final worker's aggregate PostgreSQL test count, its implication that eight drafts passed validation (seven did), and its suggestion that the full synthetic chain or a minimized executive drilldown had already been verified. The partial-grant result is an HTTP test with a **support** operator, not an executive browser demo. The paid-claim reversal reopens the existing balance; a general replacement for an already submitted/paid claim is not implemented. Migration fingerprints match before and after **within each database**; the two databases are different workspaces. No proposed commit or publish action was performed.
