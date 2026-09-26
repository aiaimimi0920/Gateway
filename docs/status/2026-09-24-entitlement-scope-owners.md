# Entitlement scope owners

Date: 2026-09-24 UTC. Status: structurally verified.

The coordinator continued the credential-group card-back split by separating
the reusable `ScopePager` presentation and the model scope band from
`EntitlementGroupScopeBoard.tsx`. The board remains the facade and keeps
provider selection, model aggregation, provider and model page derivation,
provider-toggle behavior, and the card-owned deselected-provider contract.
`ScopePager` is re-exported from the facade so the existing `ModelPoolChainBoard`
import path remains public and unchanged.

The saved pre-edit board was 394 effective / 433 physical lines. The current
owners are:

- `EntitlementGroupScopeBoard.tsx`: 197 effective / 234 physical lines;
- `EntitlementScopePager.tsx`: 44 effective / 48 physical lines;
- `EntitlementGroupModelScope.tsx`: 208 effective / 214 physical lines.

`target/effective-line-evidence/20260924-entitlement-scope-owners/verify-structure.mjs`
compares the pager implementation and model-scope section against the saved
snapshot, normalizing only the owner indentation and the callback that now
passes through the typed `onPage` prop. It also proves the facade re-export and
provider-selection/model-page reset wiring. No new request, secret read,
persistence path, timer, or resource lifecycle was introduced.

Verification for this continuation:

- `CredentialGroupsWorkspace.test.tsx`: 16/16 passed;
- full desktop Vitest: 72 files, 328/328 tests passed;
- desktop TypeScript typecheck and Web build passed;
- effective-line checker tests passed 19/19 and the ratchet passed;
- strict audit remains intentionally red for the existing 12 browser-profile
  payload violations (2,478 files scanned; 0 first-party violations);
- Neuro development-standard contract passed;
- Neuro root and Gateway scoped `git diff --check` passed;
- the three source owners and evidence script are UTF-8 without BOM and have no
  trailing whitespace.

No desktop formatter is configured. No Rust/Cargo, Docker, provider, runtime,
or immutable release gate was run for this UI presentation extraction.
