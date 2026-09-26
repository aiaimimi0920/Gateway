# Accounts ledger table owner

Date: 2026-09-24 UTC. Status: structurally verified.

The coordinator continued the existing accounts-ledger composition work by
moving the legacy account-table presentation out of
`AccountsLedgerWorkspace.tsx` into `AccountsLedgerTable.tsx`. The workspace
facade keeps `tableRows` filtering, provider-card state, account-library state,
all lifecycle callbacks, and the empty-state branches. The new owner receives
only typed rows, lock state, translation, and the two table action callbacks.

The saved pre-edit workspace was 442 effective / 471 physical lines. The
current owners are:

- `AccountsLedgerWorkspace.tsx`: 377 effective / 405 physical lines;
- `AccountsLedgerTable.tsx`: 93 effective / 98 physical lines.

`target/effective-line-evidence/20260924-accounts-ledger-table/verify-structure.mjs`
compares the moved table section against the saved snapshot, normalizing only
owner indentation and the typed `rows` prop name. It also proves that
`tableRows` filtering and edit/add callback wiring remain in the facade. No new
request, secret read, persistence path, timer, or resource lifecycle was
introduced.

Verification for this continuation:

- the two existing accounts-ledger suites: 18/18 passed;
- full desktop Vitest: 72 files, 328/328 tests passed;
- desktop TypeScript typecheck and Web build passed;
- effective-line checker tests passed 19/19 and the ratchet passed;
- strict audit remains intentionally red for the existing 12 browser-profile
  payload violations (2,479 files scanned; 0 first-party violations);
- Neuro development-standard contract passed;
- Neuro root and Gateway scoped `git diff --check` passed;
- the source owners and evidence script are UTF-8 without BOM and have no
  trailing whitespace.

No desktop formatter is configured. No Rust/Cargo, Docker, provider, runtime,
or immutable release gate was run for this presentation-only extraction.
