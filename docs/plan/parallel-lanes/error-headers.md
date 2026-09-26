# Error classification ownership and upstream header contracts

Owner: parallel coordinator. State: `accepted`.
Started: 2026-09-12.

Scope is `src/error.rs`, its new private production/test directory,
`src/upstream/headers.rs` and its new test-only directory. Original effective
counts are 830 and 1,021. Error diagnostics and classification move to cohesive
owners; typed errors, constructors, fallback policy and HTTP publication stay in
the entry. Preserve all existing public paths and serialization contracts.

Header production code measures 411 effective lines and remains unchanged.
Separate its 610-line inline tests into API authentication, browser/session
authentication and header precedence/isolation groups, retaining the two shared
fixtures. Keep test bodies and public header functions unchanged. Do not change
provider behavior, URL/header policy, cookie chunking or error handling.

The complete error and header sources/tests were read before moving code. Preserve
exact source snapshots before edits, establish fresh default-feature baseline
tests and require complete item/test comparisons afterward. Every resulting owner
must stay below 500 effective lines.

Evidence: `target/effective-line-evidence/20260912-error-headers/`. Final gates
include the same error/header tests, accepted media diagnostic caller contracts,
default-feature all-targets, scoped formatter, checker/ratchet/strict, encoding and
independent Git checks. This is structural work; no new behavior or hardening.

S06 keeps its implementation/cursor and runtime-mirror formatting. The pending
GWP-20260908-06 source/docs freeze and shared release-build transfer remain separate.
No release build follows from these scoped Cargo gates.

Fresh default-feature baselines passed: error 29/29 and headers 39/39. The source
extraction is now complete. Error entry/diagnostic/classification/test owners are
278/89/179/289 effective lines; header entry/fixtures/API/browser/precedence owners
are 413/44/213/239/117. All nine files remain below 500.

Exact proofs retain 14 moved and two retained error functions, all three public
types and their implementation blocks, seven public error paths, both constants,
29 tests and the JWT fixture. Three helpers gain only error-family visibility for
their sibling/test consumers. Header production text is unchanged: eight functions,
39 tests and two fixtures are preserved. Test groups contain 18/14/7 cases.
Module declarations already resolve by basename; no shared module file changes.
Fresh final default-feature tests pass: error 29/29, headers 39/39 and the media
response diagnostic caller contracts 10/10. All-targets compilation, scoped
formatting, checker 19/19, ratchet, source/neighbor proof, encoding and both Git
checks pass. Global formatting reports only the two unchanged S06 runtime-mirror
files. All native gates are terminal.

Immutable capture: `target/effective-line-evidence/20260912-error-headers/scope.json`,
2026-09-11 23:42:35 UTC. Strict scans 1,486 files: 31 hard, 42 mandatory and 40 soft;
73 remain above 700. Clearance is 72/145 (49.7%). No dependency, policy, baseline
or exception changed. The pending GWP-20260912-01 request does not transfer S06
ownership. No release was built.

[Acceptance report](../../status/2026-09-12-error-headers.md).
