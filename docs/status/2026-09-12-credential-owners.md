# Credential cache ownership and routing contract acceptance

Accepted structural checkpoint, captured at 2026-09-12 02:14:44 UTC.
This checkpoint does not complete the Gateway plan or create a release.

## Scope and preservation

The coordinator split the current dirty credential cache and routing-test sources.
The immutable pre-edit snapshot contains 113 inputs. All 111 neighboring inputs,
including Redis keys, module declarations, route/pipeline callers, credential
runtime and store code, refresh/keepalive callers and Gemini helpers, are unchanged.

The cache entry retains persistence, index maintenance, model lookup and expiry
parsing. Private owners hold credential serialization/accessors, refreshed runtime
material and selection/affinity. Exact comparisons preserve 19 free functions,
two types, two implementation blocks and one constant: 15 items moved and nine
retained. All 14 public entry paths remain available. CredentialKind::priority
changes from private in the entry to pub(super) in the private type owner, retaining
access within the same cache family.

Routing's normalized production prefix and all five functions are unchanged.
Its 26 tests now have API/preset, search, media/session and metadata owners with
7/7/6/6 cases. The Qwen feature condition and shared fixture are preserved. All
14 cache tests and their fixture are preserved, including the ignored live-Redis
test. The default-feature pair ran 39 tests successfully and ignored that one test.

| Source | Before effective | Accepted effective |
| --- | ---: | ---: |
| src/redis/credential_cache.rs | 976 | 287 |
| src/redis/credential_cache/entry.rs | new | 124 |
| src/redis/credential_cache/runtime_material.rs | new | 190 |
| src/redis/credential_cache/selection.rs | new | 123 |
| src/redis/credential_cache/tests.rs | moved tests | 271 |
| src/routing/credential_routing.rs | 866 | 216 |
| src/routing/credential_routing/tests.rs | moved fixture | 36 |
| src/routing/credential_routing/tests/api_presets.rs | moved tests | 151 |
| src/routing/credential_routing/tests/search.rs | moved tests | 158 |
| src/routing/credential_routing/tests/media_sessions.rs | moved tests | 167 |
| src/routing/credential_routing/tests/metadata.rs | moved tests | 144 |

All 11 files are at most 287 effective lines. The five evidence scripts measure
43-102 effective lines with the repository's C-like lexer.

## Verification and review

Fresh default-feature baseline and final Cargo gates both pass:

    cargo test --offline --locked --lib redis::credential_cache::tests -- --test-threads=1
    cargo test --offline --locked --lib routing::credential_routing::tests -- --test-threads=1

The cache pair is 13 passed / one ignored; routing is 26/26. Fresh default-feature
all-targets compilation, scoped official formatting, checker tests 19/19, ratchet,
strict UTF-8/no-BOM and whitespace validation, source/neighbor proofs, and separate
Gateway/Neuro Git diff checks pass. All native sessions are terminal.

The type owner retains serialization and model/runtime metadata behavior. The
runtime-material owner retains patch merging, timestamps, getters/setters and
error context. The selection owner retains isolation, priority, affinity TTL,
Redis command order and checkout accounting. Persistence/index pipelines and
expiry parsing remain byte-equivalent after normalized formatting. The routing
production prefix and synthetic test inputs are preserved. No new I/O, lock,
task, full-body buffer, external service access, secret output or dependency was
introduced by extraction.

The approximate timestamp implementation, expiry/index consistency, unbounded
model-index scans and live-Redis integration coverage remain separate audits.
The ignored test still targets 127.0.0.1:6379 and was not enabled or run.

## Inventory and handoff

Strict scans 1,500 files: 31 hard, 39 mandatory and 40 soft; 70 remain above 700.
The accepted clearance is 75/145 (51.7%). Strict still exits 1. Global formatting
still reports only the unchanged S06 runtime-mirror source/test files.
No policy, baseline, exception or runtime-profile change was made.

At acceptance capture, before this report was added, Gateway HEAD was
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,634 status entries: 166 modified,
one unstaged deletion, two staged deletions and 1,465 untracked. Neuro HEAD was
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and
182 untracked. Inherited changes and staged deletions are preserved.

Evidence: target/effective-line-evidence/20260912-credential-owners/scope.json,
before.json, exact extraction proofs and separate baseline/final gate logs.
The earlier stream-state and structural evidence remains immutable. SSE observer
buffer admission remains the next separate hardening boundary.

S06 retains its implementation and original plan cursor. GWP-20260912-01 remains
pending; GWP-20260908-06 still lacks its source/docs freeze and release-build
transfer receipt. No new release was built and no runtime service was changed.
