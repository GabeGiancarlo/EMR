# xAI development workers

Grok assists with scoped source review and implementation proposals. It is not a runtime dependency of Nexus and receives no patient records, audio, database exports or credentials. The local application does not call xAI.

`tools/xai-review.mjs` handles bounded reviews. `tools/xai-workers.mjs` runs one to four implementation requests concurrently. Both share an exclusive ledger lock and keep outputs in ignored `.local/xai`. The implementation runner accepts a JSON array with `name`, `instructions`, and a list of repository-relative `files`; only explicitly selected Git-tracked source is included. Environment, local-runtime and Git-internal paths are excluded.

Load the saved Windows user key into the current process without printing it:

```powershell
$env:XAI_API_KEY = [Environment]::GetEnvironmentVariable('XAI_API_KEY', 'User')
node tools/xai-workers.mjs .local/xai/implementation-tasks.json
```

Each request has a unique name, a maximum output size, a bounded deadline, and a conservative $25 reservation against the owner's $5,000 total ceiling. Known billed usage replaces reservations when returned by the API. Failed requests with unknown cost retain their reservation; a network failure is not assumed free. There are no automatic retries. The ledger includes earlier source-review spending.

Responses must contain actual new files or exact search/replace edits in a strict JSON schema. A syntactically valid proposal is still unverified code. Never execute a model's test commands automatically, and never treat its statements as evidence that tests ran.

After reviewing every change, check the proposed edits against the current checkout:

```powershell
node tools/apply-xai-proposal.mjs .local/xai/<task-name>.proposal.json
# Only after source review:
node tools/apply-xai-proposal.mjs .local/xai/<task-name>.proposal.json --apply
```

The apply helper validates all paths and exact matches before writing any files. It saves the prior contents locally. Parallel proposals may touch the same entry points; the integrating developer must resolve those overlaps, run the relevant database and frontend checks, and inspect the resulting user flow. No proposal is automatically committed, pushed or deployed.

The user explicitly requested expanded Grok implementation work on November 2, 2026. The first expanded batch encountered long-response connection failures and one plan-only response. The runner now demands structured code output and uses an explicit HTTPS deadline rather than Node fetch's shorter header timeout. Those failed calls remain represented in the ledger.
