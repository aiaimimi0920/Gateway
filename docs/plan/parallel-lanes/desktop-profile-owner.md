# Desktop profile state ownership

Date: 2026-09-21. State: structural_green.

Scope: useGatewayDesktopState.ts (627 effective), new useGatewayProfileState.ts
and focused desktopNotice.ts, plus existing state tests. Move profile names,
selected-name ref, draft/saved snapshots, transfer/import text, path result,
validation and profile lifecycle actions into one hook. Parent retains process,
probes, logs, API tests, diagnostics, busy/notice state, polling and auto-start.
Pass stable notice/busy/runtime-info setters and a stable runtime invalidation
callback; the child clears its path result before invalidating parent probes.

Preserve action bodies and dependency identities. Synchronize the selected-name
ref in the child effect and retain auto-start reset in the parent effect, in
the same order. Parent startGateway keeps saving/starting/updating snapshots
through the child's existing profile setters and reload action. Do not merge
sidecar lifecycle into profile persistence or add a reverse module dependency.

Three behavior contracts were added before moving: save/invalidation, failed
save retaining dirty draft, and default reset without deletion. Run all five
existing/new state tests before and after, typecheck, web build, source mapping,
independent review, checker/ratchet/strict, encoding and separate Git checks.
Evidence: `target/effective-line-evidence/20260921-desktop-profile-owner/`.
All resulting owners must be <=500; full-goal release acceptance remains open.

Result: 421/265/10 effective source owners; five paired contracts, typecheck,
web build, saved-baseline independent review and exact source projections pass.
See `docs/status/2026-09-21-catalog-profile-owners.md`.
