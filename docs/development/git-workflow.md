# Branches and review

Updated November 4, 2026. The active source baseline is `453c280` on `nexus/integrated-prototype`; current local review work is `codex/prototype-readiness`. `origin` is `https://github.com/GabeGiancarlo/EMR.git`.

## Start a change

Inspect status, current branch, worktrees and remote tracking before editing. Reuse a suitable existing checkout; do not create a worktree just to rename the task. Preserve uncommitted work and do not pull over it blindly. Fetch is read-only with respect to the working tree; inspect upstream changes before integrating them. For new work, use `codex/<short-purpose>` from the agreed integration branch, with an explicit owner for each shared file and migration number.

Keep a branch focused on one reviewable behavior or dependency-coherent vertical slice. Good examples are recurrence with exceptions and its permissions, remittance allocation with reversals and reconciliation, or a tested profile menu. Avoid combining a brand overhaul, billing schema rewrite and unrelated cleanup. Old demos may be moved when useful, but preserve history and update references instead of deleting work opportunistically.

## Integrate worker proposals

Give each Grok task a base revision, source hashes, objective, allowed files, constraints and acceptance scenarios. Workers return proposals; the coordinator owns the working tree. Before applying, compare the current source with the supplied packet and rebase the proposal conceptually if it is stale. Review every diff, especially migrations, permissions, money, timezones and immutable records. Shared `App.tsx`, routes, catalog contracts and migration numbering have one integrator.

Do not automatically execute returned shell commands. Run the smallest meaningful checks for the changed behavior; broaden when integration risks justify it. Include a denied/error path, correction path and persistence evidence for financial/clinical transitions. An isolated worker's successful test does not prove the integrated revision.

## PR and merge evidence

Target `nexus/integrated-prototype` unless the owner changes integration policy. A PR describes the concrete problem, resulting behavior, migration/backfill implications, validation and unresolved limits. Link the relevant capability/contract and review evidence. Do not count a draft PR as accepted functionality.

The local CI update now covers pushes to `main`, `develop`, `nexus/integrated-prototype` and `codex/**`, PR targets `main`, `develop` and `nexus/integrated-prototype`, and manual dispatch. This configuration has not yet run on GitHub for these unpushed changes. Repository branch protection and required-status rules are remote settings, not established by the YAML change. The separate legacy security workflow still needs a current action/tool compatibility and trigger review.

The manual container-publication workflow is independent of CI; it is not deployment and is not automatically gated by successful checks. Before publishing, require evidence that the exact intended revision passed applicable checks. Do not describe the repository as protected or production-deployed until remote policy and deployment evidence exist.

After accepted integration, update the capability ledger, verification log and central context. Keep historical checkpoint evidence dated; replace stale architectural inventories without rewriting what an earlier checkpoint actually tested.
