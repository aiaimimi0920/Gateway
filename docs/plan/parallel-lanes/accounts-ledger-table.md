# Accounts ledger table ownership

Date: 2026-09-24. State: structural_green.

Scope: the remaining legacy `nt-ledger-table` presentation inside
`AccountsLedgerWorkspace.tsx`. Keep `tableRows` filtering and all callback/state
ownership in the workspace facade; move the table markup into a typed owner.
Preserve row keys, role/class/ARIA contracts, translations, editor locking, and
credential-versus-provider-default action behavior.

Result: the saved workspace was 442 effective lines and is now 377. The new
`AccountsLedgerTable.tsx` owner is 93 effective lines. Structural proof confirms
the moved table section and facade filtering/callback wiring. The focused
accounts-ledger suites pass 18/18; full desktop is 328/328 across 72 files;
typecheck, Web build, checker 19/19, ratchet, development-standard contract,
strict accounting, and scoped diff checks pass. Strict remains red only for the
existing browser-profile payload debt. Evidence:
`target/effective-line-evidence/20260924-accounts-ledger-table/` and
`docs/status/2026-09-24-accounts-ledger-table.md`.

The slice changes presentation ownership only. Provider lifecycle, account
library, Rust/S06, runtime, Docker, release, and live-provider ownership remain
outside this lane.
