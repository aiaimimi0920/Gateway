# Credential group test fixture owner

Date: 2026-09-24 UTC. Status: `complete`.

## Scope

The coordinator owns the existing `CredentialGroupsWorkspace.test.tsx` contract
suite. Keep all 16 behavior tests, assertions, test names, and mocked callback
semantics unchanged. Move only shared setup into a dedicated test fixture
owner: render wrapping, empty/populated workspace builders, account fixture
data, and the account-card bridge factory.

This is a test-maintainability slice. It does not change production
`CredentialGroupsWorkspace` code, public props, or runtime behavior.

## Result

`CredentialGroupsWorkspace.test.tsx` now contains the behavior suite and imports
setup from `CredentialGroupsWorkspace.fixtures.tsx`. The fixture owner contains
only reusable test setup and remains below the repository's acceptable ceiling:

| File | Before | After |
| --- | ---: | ---: |
| `CredentialGroupsWorkspace.test.tsx` | 496 effective / 580 physical | 290 effective / 366 physical |
| `CredentialGroupsWorkspace.fixtures.tsx` | n/a | 216 effective / 225 physical |

The test suite stays below the preferred maintenance range's upper boundary and
the new fixture owner has one clear responsibility. No production source or
effective-line baseline/exception changed.

## Evidence

Evidence root: `target/effective-line-evidence/20260924-credential-group-test-fixtures/`.
It contains before/after test snapshots, the new fixture snapshot,
`line-counts.json`, `structural-proof.json`, and the source-level verifier.

## Verification

- Focused `CredentialGroupsWorkspace.test.tsx`: 1 file, 16/16 passed.
- Full desktop Vitest: 71 files, 328/328 passed.
- TypeScript typecheck: exit 0.
- `npm run build:web`: exit 0.
- Effective-line checker tests: 19/19 passed.
- Adoption ratchet: exit 0; no baseline or exception changed.
- New and changed source files are UTF-8 without BOM and have no trailing
  whitespace.
- Gateway and Neuro `git diff --check` remain clean.

No Rust, release, runtime, Docker, or live-provider gate ran for this test-only
continuation. Existing S06 Rust/Gemini ownership and the dirty shared worktree
remain unchanged; no files were staged, committed, reverted, or removed.
