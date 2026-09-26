# Entitlement scope ownership

Date: 2026-09-24. State: structural_green.

Scope: `EntitlementGroupScopeBoard.tsx` (394 effective lines in the saved
snapshot). Keep provider selection and aggregation in the board facade, move
the pager into a reusable leaf, and move only the model-scope presentation into
a typed owner. Preserve `deselectedProviderIds` as card-owned state shared with
the expanded account panel, the model-page reset after provider selection
changes, all DOM/class/ARIA/translation contracts, and the existing
`ScopePager` facade export used by `ModelPoolChainBoard.tsx`.

Result: the facade is 197 effective lines, `EntitlementScopePager.tsx` is 44,
and `EntitlementGroupModelScope.tsx` is 208. Structural proof confirms the
moved pager and model JSX, with only owner indentation and the typed page
callback boundary normalized. The focused credential-group suite passes 16/16;
full desktop is 328/328 across 72 files; typecheck, Web build, checker 19/19,
ratchet, development-standard contract, strict accounting, and scoped diff
checks pass. Strict remains red only for the existing browser-profile payload
debt. Evidence: `target/effective-line-evidence/20260924-entitlement-scope-owners/`
and `docs/status/2026-09-24-entitlement-scope-owners.md`.

The slice changes presentation ownership only. Rust/S06, provider/runtime,
Docker, release, and live-provider gates remain outside this lane.
