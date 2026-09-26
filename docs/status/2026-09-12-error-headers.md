# Error ownership and upstream header contracts

Accepted on 2026-09-12. Error diagnostics/classification and upstream header tests
now have cohesive owners below 500 effective lines. This checkpoint preserves
behavior and does not complete the overall Gateway plan.

## Ownership and preservation

`src/error.rs` decreases from 830 to 278 effective lines. Its diagnostics,
classification and test owners measure 89, 179 and 289. The entry retains public
types, constructors, retry/fallback policy and HTTP publication. Classification
depends directly on diagnostics; no dependency cycle is introduced.

Complete item comparisons preserve 14 moved and two retained free functions,
three public types and their implementation blocks, seven public entry paths,
both production constants, all 29 tests and the JWT fixture. Only three private
helpers gain `pub(super)` visibility for error-family sibling/test consumers:
`kind_from_http_status`, `kind_from_body_keywords` and `truncate`. No crate-wide
visibility expansion or unused re-export suppression is needed.

`src/upstream/headers.rs` decreases from 1,021 to 413 effective lines. Its complete
normalized production prefix is unchanged, including eight functions and all
constants. The fixtures, API authentication, browser/session authentication and
precedence/isolation test owners measure 44, 213, 239 and 117. All 39 original tests
and both fixtures remain unchanged; the three groups contain 18, 14 and seven
cases. The production prefix SHA-256 is
`e273b608b030f2c8ab50e067b564892f53eb161c9c78678512d00d936ea896e7`.

All nine scoped files are at most 413 effective lines. Existing basename module
declarations resolve the new owners; neither shared module file changes. Recorded
neighbor hashes, manifests, line policy, baseline and exceptions are unchanged.
No diagnostic, credential, cookie-chunking, header-precedence or provider behavior
change is mixed into this extraction.

## Verification

- Fresh paired default-feature error tests: 29/29 before and after.
- Fresh paired default-feature upstream header tests: 39/39 before and after.
- Existing media response diagnostic caller contracts: 10/10 after extraction.
- Fresh offline, locked default-feature all-targets compilation: passed.
- Scoped official formatter, checker tests 19/19 and ratchet: passed.
- Exact item/test/neighbor proof, UTF-8 without BOM and whitespace: passed.
- Gateway and Neuro Git diff checks: passed independently.

The final Cargo chain uses `cargo check --offline --locked --all-targets`, followed
by `cargo test --offline --locked --lib` for `error::tests`,
`upstream::headers::tests` and `upstream::media_response_diagnostic_tests`, each
with `--test-threads=1`. Every command exited zero and all native gates are terminal.
Only inherited Gemini warnings remain; none names a scoped source owner.

Global formatting reports only the unchanged S06 runtime-mirror source/test
files. Strict scans 1,486 files: 31 hard, 42 mandatory and 40 soft; 73 remain above
700. Structural clearance is 72/145 (49.7%). Strict intentionally exits one while
the inherited debt remains; this checkpoint does not claim a green strict gate.

Immutable evidence:
`target/effective-line-evidence/20260912-error-headers/scope.json`.
Capture time: 2026-09-11 23:42:35 UTC. Earlier accepted extraction/hardening snapshots
remain unchanged.

The pre-documentation snapshot records Gateway HEAD
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`, with 1,610 status entries: 163 modified,
one unstaged deletion, two staged deletions and 1,444 untracked. Neuro HEAD remains
`bf818f0324024634bc890585efb78cc8e603d11a`, with 192 entries: 10 modified and
182 untracked. Inherited changes and staged deletions are retained.

## Remaining boundaries

S06 retains its Rust/Gemini implementation and original plan cursor. The pending
GWP-20260912-01 request requires an actual executor receipt or user-authorized
transfer; it does not itself grant ownership. Source/docs freeze and the final
shared release-build window remain separate. No provider, browser, packaged
runtime or release was exercised by these gates. No new release was built.
