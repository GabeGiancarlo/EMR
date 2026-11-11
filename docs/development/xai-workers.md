# xAI development workers

Grok assists with scoped source review and implementation proposals. It is not a runtime dependency of Nexus and receives no patient records, audio, database exports or credentials. The local application does not call xAI.

November 4 console inspection showed model-specific limits for the selected team/cluster: Grok 4.7 displayed 50M tokens/minute; Grok 4.20 Multi-Agent Beta displayed 2.5M tokens/minute. These are an observed account snapshot, not permanent guaranteed throughput or proof of the cause of every 429. Provider-internal multi-agent work can be much larger than the submitted source packet. Honor cooldowns and bound concurrency by model. More calls do not make incomplete source packets or unreviewed claims reliable.

Source packets must include the schema migrations, authorization helpers and downstream validators relevant to each claim. Record unseen dependencies and treat their behavior as unknown. Several expanded-pass false findings came from inspecting only a foundation migration or parent module; a missing packet file is not a missing feature. Reviewed research belongs in the dated docs; raw output remains a proposal. Narrow implementation/test tasks can use high reasoning while deeper councils use xhigh; the ledger records the actual choice.

`tools/xai-review.mjs` handles bounded reviews. `tools/xai-workers.mjs` runs one to four implementation requests concurrently. Both share an exclusive ledger lock and keep outputs in ignored `.local/xai`. The implementation runner accepts a JSON array with `name`, `instructions`, and a list of repository-relative `files`; only explicitly selected Git-tracked source is included. Environment, local-runtime and Git-internal paths are excluded.

Load the saved Windows user key into the current process without printing it:

```powershell
$env:XAI_API_KEY = [Environment]::GetEnvironmentVariable('XAI_API_KEY', 'User')
node tools/xai-workers.mjs .local/xai/implementation-tasks.json
```

Each request has a unique name, a maximum output size, a bounded deadline, and a $25 reservation for implementation work ($20 for the older review runner). Known billed usage replaces reservations when returned by the API. Failed requests with unknown cost retain their reservation; a network failure is not assumed free. There are no automatic retries. Reservations are accounting holds, not provider-enforced charge caps.

The latest November 4 expanded pass has a **$500 spending target and ceiling**, registered as `deep-pass-2026-11-04`. The earlier $1,500 ceiling and November 2 $5,000 permission are historical. The shared ledger preserves earlier charges/unknown holds separately from the active run. Generation cannot create another run ID to obtain a fresh budget, and an existing run's ceiling cannot be changed by a generation command. A new authorization must be explicitly recorded by the coordinator from a new owner instruction before dispatch. Legacy JS runners refuse to run while a run-scoped authorization is active. `node tools/xai-status.mjs` shows per-run costs and holds; this is not an account-wide invoice.

## macOS research fleet

The [Codex/OpenAI orchestration guide](orchestration.md) defines current coordination. All subordinate research/implementation workers use actual Grok calls; OpenAI is used for the coordinator and core reviews. The new dependency-free Python runner accepts up to 240 scoped tasks with at most 16 concurrent calls and a separate default cap of two multi-agent calls:

```sh
python3 tools/grok-research.py .local/xai/deep-pass/wave2.json --budget-usd 500 --run-id deep-pass-2026-11-04 --dry-run
python3 tools/grok-research.py .local/xai/deep-pass/wave2.json --budget-usd 500 --run-id deep-pass-2026-11-04 --parallel 16 --multi-parallel 2 --watch --resume
node tools/xai-status.mjs
```

The examples refer to a reviewed local task file, not a file present in every clone. Use unique names for each deliberately authorized follow-up. `--resume` skips all names already in the ledger, including failed/unknown requests; it never silently retries them. The watched queue is append-only once a task is prepared: editing or removing that name does not cancel or change its prepared packet. Model IDs were verified against the key: `grok-4.7` for implementation and `grok-4.20-multi-agent-0309` for specialist councils. Both use `xhigh` reasoning in this pass; xAI documents 16 internal agents for the multi-agent configuration. Only returned leader output/usage can be inspected. Example task shape:

```json
[{"name":"unique-review-name","instructions":"Review the supplied source; distinguish verified facts from proposals.","files":["Cargo.toml","docs/architecture/database.md"],"model":"grok-4.7","web":false,"effort":"xhigh","max_output_tokens":24000}]
```

On macOS the Python runner always retrieves generic-password service `xai-api-key` privately in memory for that run. It uses `XAI_API_KEY` only on non-macOS hosts. If the named Mac item is unavailable, ask for the item name, never the secret. It does not print or persist the credential. Selected tracked source is line-numbered and hashed; resolved paths exclude local runtime data, environment files, keys and databases. Model tools are limited to optional native web search. `store: false` is sent; this is not a claim about all provider retention policies. Source prompts and responses stay in ignored `.local/xai`; no patient records, audio or runtime database contents are included. Request artifacts omit authentication headers. The runners also reject common audio/video, CSV export, database-dump and credential-file extensions, including case variants. This is defense in depth; the coordinator still reviews every selected source packet for sensitive content.

The scheduler reserves budget immediately before dispatch and refills available slots without waiting for slower siblings. Actual concurrency may be lower than 16 because of budget exposure, queued work or per-model limits. HTTP 429 and classified stream rate-limit errors pause only the affected model; HTTP-date and numeric Retry-After are respected, and a later error cannot shorten an existing cooldown. The default response timeout is 3,600 seconds. Safe progress timestamps and error categories are recorded without raw error messages. Per-task reservations are estimates, not hard charge caps; unknown outcomes keep their holds.

There is no model shell, filesystem browsing, autonomous code execution or automatic application of proposals. Research reports require coordinator review, source verification and reconciliation into durable docs. An interrupted request retains its reservation; inspect its outcome before deliberately creating a new task name. Do not remove another live runner's lock or start a separate ledger to evade the ceiling.

Set `proposal: true` for the Python implementation schema, or `structured: true` for a five-section research report; do not combine them. Multi-agent councils currently use Markdown, with substantive coordinator review rather than a schema-based acceptance claim.

Implementation-proposal responses must contain actual new files or exact search/replace edits in a strict JSON schema. Research reports use Markdown and do not claim code execution. A syntactically valid proposal is still unverified code. Never execute a model's test commands automatically, and never treat its statements as evidence that tests ran.

After reviewing every change, check the proposed edits against the current checkout:

```powershell
node tools/apply-xai-proposal.mjs .local/xai/<task-name>.proposal.json
# Only after source review:
node tools/apply-xai-proposal.mjs .local/xai/<task-name>.proposal.json --apply
```

Review the apply helper schema before using it with a newer Python proposal; runners may use different metadata field names. Apply only reviewed file/edit content. The apply helper validates all paths and exact matches before writing any files. It saves the prior contents locally. Parallel proposals may touch the same entry points; the integrating developer must resolve those overlaps, run the relevant database and frontend checks, and inspect the resulting user flow. No proposal is automatically committed, pushed or deployed.

The user explicitly requested expanded Grok implementation work on November 2, 2026. The first expanded batch encountered long-response connection failures and one plan-only response. The runner now demands structured code output and uses an explicit HTTPS deadline rather than Node fetch's shorter header timeout. Those failed calls remain represented in the ledger.

## Cost and failure checks

Provider receipt cost is recorded before report parsing or artifact writes. A local output failure cannot replace a known charge with a smaller reservation. Invalid or incomplete reports still cost money. A failure before dispatch is recorded as zero; a sent request with an unknown outcome retains its hold. HTTP 401 stops queued calls, retains in-flight holds and is not retried. The coordinator should report that the credential may be expired or revoked.

Run `python3 -m unittest discover -s tools -p 'test_grok_research.py' -v` for synthetic accounting/failure regressions. These tests never read a real credential or contact xAI. Reservations are not hard provider caps: one request can exceed its reservation, so updated exposure stops subsequent queued launches. Do not infer actual spend from requested tokens, agent count or reservations.

For the final November 4 fleet, back-to-back high-context multi-agent responses still caused token-rate HTTP 429s at concurrency one. The runner now accepts `--multi-gap-seconds 65` to space completions for that model while Grok 4.7 continues independently. This is an observed operational adjustment, not a guarantee against provider throttling. Graceful coordinator interruption drains already-running calls before releasing the ledger lock; resumption skips every recorded task, including failures, and does not retry charged or unknown outcomes.
