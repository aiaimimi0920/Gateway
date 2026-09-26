# Routing policy and credential-document test ownership

Date: 2026-09-21. Status: structural_green; full goal remains active.

## Routing candidate policy

`src/routing/candidate.rs` decreases from 541 to 429 effective lines.
`src/routing/candidate/endpoint_policy.rs` owns the complete payload endpoint
capability/execution policy impl and its two private helpers (119 effective).
The parent retains payload/candidate types, serialization, adapter constants,
canonicalization functions and all 15 existing tests. Only the test-local
EndpointKind import and private module declaration change outside extraction.

Exact independently formatted projections match both files. Public methods
remain on ProviderAccountPayload; the new module and helpers remain private.
Endpoint override -> producer video -> account mode -> adapter default order,
OpenAI bridging, forced-stream decisions, search-path support and all endpoint
key strings remain unchanged. No I/O, allocations, locks, tasks, cancellation
or credential lifetime changes are introduced. Existing cooldown timestamp
comparison and payload validation behavior are outside this extraction.

Evidence: `target/effective-line-evidence/20260921-routing-candidate-policy/`.
The saved baseline and `extraction-proof.json` retain source identity and counts.

| Check | Result |
| --- | --- |
| Before/after `cargo test --locked --lib routing::candidate::tests -- --test-threads=1` | 15 passed / 15 passed |
| `cargo check --locked --all-targets` | Passed, three inherited Gemini warnings |
| Exact formatted projection and UTF-8 without BOM | Passed for both files |
| Independent read-only boundary review | No extraction defect found |
| Checker tests / ratchet / scoped rustfmt | 19 passed / passed / passed |
| Separate Gateway and Neuro diff checks | Passed |

The candidate checkpoint strict report has 2227 scanned, 10 hard, 18 mandatory
and 34 soft entries. Global formatting still fails on the two unchanged reserved
Gemini runtime-mirror files, recorded in `format-all.log`. No global-format or
release-gate success is claimed.

## Credential document test owners

The 636-effective-line aggregate test file now retains the four creation and
identity-validation cases. Existing tests move intact to three responsibility
owners; a shared route-document fixture avoids repeated setup. Vitest discovery
and setup remain unchanged. All test descriptions, data and assertions occur
exactly once, with no new production behavior or test configuration.

| File under `apps/desktop/src/features/console/` | Effective lines | Tests |
| --- | ---: | ---: |
| `credentialDocument.test.ts` | 185 | 4 creation/identity cases |
| `credentialDocument.update.test.ts` | 199 | 4 update/scheduling cases |
| `credentialDocument.delete.test.ts` | 80 | 2 deletion/membership cases |
| `credentialDocument.secret-patches.test.ts` | 169 | 3 secret-patch cases |
| `credentialDocumentTestFixtures.ts` | 12 | Shared pure fixture |

Evidence: `target/effective-line-evidence/20260921-credential-document-tests/`.
Exact source projection preserves all 13 test bodies and the fixture, with only
imports/exports and test-owner wrappers changed. Independent review confirmed
the inventory and imports. The fixture creates fresh objects for every call;
no mutable singleton, hook, fake timer, storage operation or resource lifetime
is introduced. Existing credentialDocument.ts production changes predate this
test move and are preserved, not attributed to it.

| Check | Result |
| --- | --- |
| Original `credentialDocument.test.ts` via `npm test --prefix apps/desktop -- --run` | 1 file, 13 passed |
| Same runner with all four owner paths | 4 files, 13 passed |
| `npm run typecheck --prefix apps/desktop` | Passed |
| Exact projection / UTF-8 without BOM | Passed for all five files |
| Checker tests / ratchet | 19 passed / passed |
| Separate Gateway and Neuro diff checks | Passed |

The desktop package provides no formatter script; original test formatting is
preserved, with no formatter dependency/configuration introduced. These tests
prove their existing document-mutation contracts, not live provider or UI
acceptance. No additional assertions are claimed.

## Full-goal continuation

Across both extractions, strict inventory changes 2226 -> 2231 scanned and
35 -> 33 soft entries. Hard/mandatory counts remain 10/18; strict still exits 1
with 28 files above 700. No baseline, policy, exception or runtime profile was
modified. Gateway's inherited dirty worktree and Neuro's Gateway submodule
state remain intact; no sibling source, staging, commit or push was changed.

Next safe candidates are ManagementSessionProvider test ownership (638), then
console telemetry (578). Read-only scouts mapped bootstrap/origin/credential/
expiry test lifecycles and telemetry snapshot/rollup/presentation boundaries;
exact source and paired tests must be read before editing. The complete
acceptance checklist remains in `2026-09-21-refactor-completion-audit.md`.

No new release package or live deployment was performed. S06 source/build
transfer and runtime-artifact governance remain unresolved; safe work remains.
The requested release destination remains
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
