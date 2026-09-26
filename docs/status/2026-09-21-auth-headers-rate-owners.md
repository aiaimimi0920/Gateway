# Authentication, request headers and rate-rule extraction

Date: 2026-09-21. Status: structural_green; full goal remains active.

## Ownership and preservation

| Owner | Before effective lines | After effective lines |
| --- | ---: | ---: |
| `src/console/auth.rs` | 617 | 359 |
| `src/console/auth/types.rs` | New | 86 |
| `src/console/auth/tests.rs` | New | 177 |
| `src/http/request_headers.rs` | 518 | 165 |
| `src/http/request_headers/tests.rs` | New | 341 |
| `src/rate_limit.rs` | 516 | 268 |
| `src/rate_limit/rules.rs` | New | 255 |

Authentication runtime retains token verification, grants, crypto and atomic
persistence. The nested types owner contains the context, actor, wire views and
stored records. Context fields remain private; only actor/record fields needed
by the original owner gain parent-only visibility. Public type paths and serde
contracts remain unchanged. Existing tests move intact into the nested owner.

The request-header parent retains all production code and the shared
`TestConsoleAuthFixture`, including its original path and Drop cleanup. Only
the existing tests move. Trusted forwarding headers and account-group selectors
still require a valid management token; the selector path remains fail-closed.

The rate-limit parent retains types, Redis admission, timeout/error mapping and
the fixed-window Lua. The rules owner selects policy and constructs canonical
tenant-scoped keys. Four public builders/identity functions are re-exported at
their original paths. Rule order, direct-before-alias matching, hash component
order, enforcement-version gate and nonpositive-rule omission remain unchanged.

Exact independently formatted source projections pass for all seven files;
the raw fixed-window Lua is byte-identical. Both read-only boundary reviews
found no semantic regression. This is structural work without new behavior,
dependencies, baseline changes, exceptions or changes to the reserved S06 scope.

## Verification

Evidence: `target/effective-line-evidence/20260921-auth-headers-rate-owners/`.
Saved source baselines, before/after logs and `extraction-proof.json` record
hashes, counts, projections, Lua identity and encoding. All seven source files
are valid UTF-8 without BOM and below 500 effective lines.

| Check | Before | After |
| --- | --- | --- |
| `cargo test --locked --lib console::auth::tests -- --test-threads=1` | 6 passed | 6 passed |
| `cargo test --locked --lib http::request_headers::tests -- --test-threads=1` | 13 passed | 13 passed |
| `cargo test --locked --test rate_limit_enforcement_contract -- --test-threads=1` | 11 passed, 2 ignored | 11 passed, 2 ignored |
| `cargo test --locked --test phase3_access_reliability_contract rate_limit -- --test-threads=1` | 2 passed, 6 filtered | 2 passed, 6 filtered |

`cargo check --locked --all-targets` passes with three inherited Gemini
warnings. Checker tests pass 19/19, ratchet passes, scoped rustfmt passes and
both Gateway and Neuro `git diff --check` pass. Fresh global formatting still
fails on the two unchanged reserved Gemini runtime-mirror files; see
`format-all.log`. No global formatter success is claimed.

Strict scans cover 2222 -> 2226 files. Hard/mandatory counts remain 10/18;
soft entries decrease **38 -> 35**. Strict exits 1, with 28 files still above
700. Runtime browser-profile entries remain counted; no policy exclusion or
profile edit was introduced.

## Safety review and limitations

Each new owner preserves its original input validation and error behavior.
Authentication secrets are not logged; token hashing, expiry/origin/IP binding,
lock ownership and filesystem writes remain in the runtime. Header filtering
preserves its denylist, fail-closed selector gate and original fixture cleanup.
The rate-limit move adds no Redis operation, allocation pass, retry, task or
queue, and preserves unavailable fail-open versus indeterminate fail-closed
handling. Source projection verifies unchanged function and test bodies.

Existing tests do not establish live Redis concurrency/failover acceptance:
two Redis-dependent enforcement tests require `GATEWAY_RATE_LIMIT_REDIS_URL`.
Additional golden-hash, empty-component and endpoint-alias edge cases were
identified as potential future coverage; this extraction introduces no change
to those behaviors. Existing payload bounds, blocking filesystem/crypto work
and operation-deadline questions remain separate hardening work.

## Continuation and repository state

Gateway remains an inherited dirty repository (873 status entries at this
checkpoint). The scoped status contains the three modified parents and their
three new child directories; Neuro separately reports its dirty Gateway
submodule. No sibling source, staging, commit, push or live service was changed.

Next unreserved candidate: `src/routing/candidate.rs` (541 effective lines).
A scout identified endpoint capability/execution policy methods at lines
405-527 as a cohesive extraction; exact source and paired tests must be read
before editing. Full completion remains tracked by
`2026-09-21-refactor-completion-audit.md`.

No new release package is claimed. The final shared build window and S06
source transfer remain unresolved; other safe work remains available. The
requested output directory stays
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
