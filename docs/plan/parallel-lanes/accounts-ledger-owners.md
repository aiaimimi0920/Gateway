## Provider-card composition closure (2026-09-09)

Provider-card composition is now split by responsibility. `AccountsLedgerWorkspace.tsx` is 439 effective lines and retains state, callback ownership, table rows and composition wiring. `ProviderLedgerCard.tsx` is 421 effective lines and owns the two-face provider card frame and provider actions. `ProviderCardFrontBody.tsx` is 185 effective lines for pool, metrics, availability and quota display. `ProviderAccountLibrary.tsx` is 105 effective lines for source-library grouping/paging. `ProviderLifecycleBack.tsx` is 309 effective lines and `ProviderStorageEndpoints.tsx` is 249 for lifecycle/storage controls. `ProviderLifecycleActionDialog.tsx` is 104 and `accountsLedgerTypes.ts` is 97.

Behavior-preservation proof compared the saved pre-extraction card/library/dialog JSX, public types, hook order, callback initializers and grouping helper; 25 checks passed. The focused Ledger/accounts/pool batch passed 28/28 and the full desktop suite passed 311/311 across 61 files. Desktop typecheck, web build, effective-line checker tests 19/19, ratchet and `git diff --check` passed. All new files were verified UTF-8 without BOM. Evidence: `target/effective-line-evidence/browser-console-owners/ledger-frame-final.json`, `ledger-full-final.json` and the corresponding source snapshots.

All completed owners are below 500 effective lines; the parent is below 500, so no new soft-limit exception is needed. The old `CredentialPoolPolicyControl.tsx` was removed after its responsibility had already been integrated into lifecycle ownership; no remaining source imports reference it. Structural clearance is now 42/145 (29.0%), leaving 103 files above 700 effective lines. Native packaging remains blocked on an explicit GWP-20260908-06 source/docs freeze and Cargo transfer receipt; the latest immutable release remains historical.

# Lane W: accounts ledger presentation ownership

## Test contract split (2026-09-10)

`AccountsLedgerWorkspace.test.tsx` was split by behavior boundary into
`AccountsLedgerWorkspace.provider-card.test.tsx` and
`AccountsLedgerWorkspace.library.test.tsx`, with shared render/fixture helpers
in `AccountsLedgerWorkspace.fixtures.tsx`. The two suites preserve all 18
provider-card, lifecycle, library and pool assertions and pass 18/18 together;
the consolidated desktop suite passes 314/314 across 63 files, and desktop
typecheck, checker tests 19/19, ratchet and diff also pass. The two test owners
are 358 effective lines each, and the removed 863-line test entry no longer
contributes to the 701-1500 tier.

Coordinator scope: `AccountsLedgerWorkspace.tsx`, its cohesive presentation owners
and associated UI tests. Existing Rust/Cargo ownership remains external. The
starting entry has 1900 effective lines and remains structural debt until all
completed owners meet the repository thresholds.

## Credential pool policy control

This section records the superseded intermediate boundary; its implementation
was later folded into the provider lifecycle owner during the composition closure
above. The current source has no CredentialPoolPolicyControl.tsx import.

The category pool policy renderer now belongs to `CredentialPoolPolicyControl.tsx`
(252 effective lines). The existing capacity input, refill/prune switches, driver
and queue status, manual refill and automation controls were moved together. The
parent retains the shared target-draft state and commit callback because provider
lifecycle controls use that same state. A narrow six-prop boundary passes the
policy options, draft map, setter, commit callback, lock and translator; no new
state, timer, request or secret copy is introduced.

The entry decreased from 1900 to 1677 effective lines. It remains above the hard
ceiling and is not counted as cleared. Further provider lifecycle/card/library
and ledger-table decomposition is required. The new control is below 500.

Before extraction, the existing ledger suite passed 18/18. After extraction,
ledger plus pool integration passed 22/22, followed by typecheck and web build.
The final indentation-only pass preserved the code and passed ratchet/diff checks;
UTF-8 without BOM was verified. Evidence prefix:
`target/effective-line-evidence/browser-console-owners/ledger-policy-*`.

Capacity Escape/blur behavior and locked-input behavior are inherited, not newly
validated keyboard contracts. Any follow-up behavior correction should first
reproduce the failing interaction and cover both category/provider editors.
Native packaging remains subject to the pending explicit GWP-06 handoff.

## Provider lifecycle and storage endpoints

This section records the earlier intermediate count; the final lifecycle owner
is 309 effective lines after the composition closure above.

The provider-card back-side lifecycle renderer is now split into
`ProviderLifecycleBack.tsx` (315 effective lines) and its credential storage
endpoint/password renderer `ProviderStorageEndpoints.tsx` (249 effective lines).
The parent keeps the provider-card state, target draft map, lifecycle confirmation
actions and callback ownership; the child receives explicit typed values and
setters. Archive/prune/driver controls, endpoint copy feedback, password edit
semantics and the existing provider-specific busy IDs remain in their original
order. No API request, secret read, or retry behavior was added.

The ledger entry decreased from 1677 to 1228 effective lines. It remains legacy
structural debt above 700, while both new owners are below 500. The existing
ledger/accounts/pool focused batch passed 28/28 after extraction, followed by
typecheck, web build, ratchet and diff checks. The first lifecycle run exposed
missing child icon/copy dependencies; those were repaired before the final green
gate. Evidence prefix: `target/effective-line-evidence/browser-console-owners/ledger-lifecycle-*`.
