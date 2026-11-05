# Nexus working context

Start by reading `docs/PROJECT_CONTEXT.md`. For current decisions and research, read `docs/research/research-planning-review-2026-11-04.md`; then open only the module contracts/source needed for the task. Repository evidence and current user instructions take precedence over stale checkpoint summaries.

## Owner direction

- Target: Friday November 13, 2026, all planned prototype features; external integrations are lower priority. This is a target, not verified completion or a delivery guarantee.
- Functional workflows first; branding implementation afterward. Outpatient is the first customer priority; support for additional care settings is desired, but the second setting/state/payers remain unspecified.
- Profile/settings/theme/sign-out belong at the **bottom left beneath navigation**. Brand direction: restrained nature/growth, muted earthy green with neutral light/dark palettes, firmer corners and subtle motion. Existing visuals are not accepted.
- Preserve the 50-capability scope in `docs/product/acceptance-ledger.md`. A screen, successful build, worker claim or test count does not establish acceptance.

## Coordination

Follow `docs/development/orchestration.md` and `docs/development/git-workflow.md`. OpenAI/Codex coordinates and performs core reviews; subordinate research and implementation workers use actual Grok API calls. Do not substitute native OpenAI workers without a new user instruction. Review all worker proposals before applying them; never execute model-suggested commands blindly.

The latest November 4 expanded pass has a separate **$500 spending target and ceiling**, superseding the earlier $1,500 not-a-target direction for new work in this pass. Use it for useful depth, implementation proposals and independent follow-up reviews; never claim reservations as spend. Run ID: `deep-pass-2026-11-04`. Keep prior reported charges and unknown reservations separate within the shared ledger. Record reported charges and unresolved reservations in the shared ignored ledger. Do not treat this run's authorization or older RoadRunner/November 2 budgets as blanket permission for unrelated work. Never print, persist or include API credentials in model prompts. Grok receives selected source/public research only, never patient data, audio, database dumps or local runtime files.

## Engineering boundaries

- Inspect `git status` before editing; preserve existing uncommitted work. Use `codex/` branches, small coherent changes and integrated validation. Do not rewrite shared history or publish/deploy merely because a plan mentions it.
- Active application: Rust `api/` (Actix/SQLx/PostgreSQL) and React/TypeScript `frontend/`. `core/`, `fhir/`, `jobs/` are parked. There is no premade UI component library.
- Local operator headers exercise permissions but are not authentication. Keep local mode loopback-only with synthetic data. Do not expose it as a production service.
- Preserve tenant/patient scope, immutable signatures and source versions, integer-cent money, audit transactions and explicit corrections. Serialize migration numbering and changes to shared routing/entry files.
- Use `nexus_test` for disposable integration tests. Do not reset the ordinary `nexus` or owner walkthrough database. `.local/` contains runtime data and keys; do not include it in source packets or commits.
- Verify relevant failure/permission paths, persistence and user journeys. Record what actually ran, on which revision/platform, and what remains unverified in `docs/development/verification.md`. Update central context when architecture, accepted scope or operating instructions change.
