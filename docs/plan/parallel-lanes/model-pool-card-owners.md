# Model pool card rendering owners

Date: 2026-09-21. State: implementing.
Owner: resumed coordinator for conversation 01a0c09c-fd27-7c20-af43-d8979ca443c8.

Scope: `apps/desktop/src/features/console/ModelPoolWorkspace.tsx`, new metric
and action presentation owners, focused component/browser contracts, and this
lane's evidence/status documentation. S06 source and final build ownership stay
reserved until an explicit transfer is received.

The parent has 624 effective / 662 physical lines. Its pre-edit source is saved
in `target/effective-line-evidence/20260921-model-pool-card-owners/` together
with separate Gateway and Neuro Git status snapshots. The inherited Gateway
worktree has 2752 changed/untracked paths; this task does not reset or stage them.

Extract the front-face aggregate metrics and card actions as presentation
components. Keep expansion, provider deselection, both menu hooks, flip state,
announcements, ref ownership and focus effects in the workspace. Preserve DOM
wrappers, attributes, classes, labels, handler arguments and callback ordering.
Keep the existing public workspace props and import path.

No focused workspace interaction suite exists in the current desktop tree.
Establish behavioral characterization before production edits: aggregate and
missing metrics, action/lock behavior, menu Escape/focus, flip focus, and shared
provider-selection/account-panel state. Run the same suite after extraction,
plus typecheck, browser proof, web build, checker tests, ratchet, strict inventory,
encoding and separate Git diff checks. Desktop has no configured formatter.

Default-role scout/reviewer dispatch failed with HTTP 503 before execution.
Main-thread review is available; no independent approval is claimed.

Pre-change characterization is green: 7 component/editor tests and 2 real
Chromium cases (desktop and mobile). The initial test-only selector ambiguity
and account-summary fixture mismatch are corrected, with their failed logs
retained. Production source was unchanged throughout baseline verification.
