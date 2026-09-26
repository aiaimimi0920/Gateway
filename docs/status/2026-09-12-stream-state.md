# Stream state and terminal cleanup

Accepted on 2026-09-12. The unchanged public regression contract advances from
four passes and nine failures to 13/13. This checkpoint corrects stream arithmetic,
archive truncation and terminal cleanup; the overall Gateway plan remains open.

## Reproduction and correction

Both usage formats eagerly evaluated unchecked fallback addition inside
`unwrap_or`, including when the upstream supplied an explicit total. Four tests
reproduced debug overflow with valid JSON `u64::MAX` counters. Fallback totals now
use saturating addition, matching the existing usage-merge policy. The duplicate
input/output branch was already shadowed by the preceding output-token branch and
has been removed. Normal counts, explicit-total parsing and cache details retain
their existing merge behavior.

Archive capture marked empty chunks as truncated after reaching its byte limit,
including at zero capacity with no incoming data. Two regressions reproduced the
false data-loss flag. Empty chunks now return before changing state. Real overflow
still retains the byte prefix and sets truncation; all chunks still pass through.
Existing lossy UTF-8 snapshot behavior is preserved.

Tracked streams fired completion while retaining their pinned upstream. Three
regressions exposed terminal retention and callback cleanup ordering. The private
inner field is now optional and is released before invoking completion. Clean EOF,
error and early drop invoke the callback exactly once; EOF/error stop subsequent
upstream polling. The first error item remains visible to the caller. This
intentionally strengthens cleanup ordering: callbacks now observe released
upstream state. Nested usage/archive handles retain their snapshots independently,
as the composition regression verifies. The tracker no longer needs unsafe pin
projection.

Only five function bodies and one private field change. Exact proof preserves
19 function signatures, 14 other bodies, eight other structs, module/import
declarations and 95 neighboring source/input files. Production owners measure
109, 97 and 217 effective lines; the public regression owner is 219. All four
remain below 500. Dependency files, policy, baseline and exceptions are unchanged.

## Verification

- Fixed public regression source: 4 passed / 9 failed before, 13/13 after.
- Paired original stream tests: 14/14 before and after.
- Paired HTTP SSE caller tests: 3/3 before and after.
- Fresh default-feature offline, locked all-targets compilation: passed.
- Scoped official formatter, checker tests 19/19 and ratchet: passed.
- Exact source/neighbor proof, UTF-8 without BOM and both Git checks: passed.

The paired tests use `--offline --locked --no-default-features --features
line-lumalabs-web-reverse-api,line-suno-web-reverse-api,line-udio-web-reverse-api`.
The public target is `--test stream_observation_contract`; library filters are
`--lib upstream::stream::tests` and `--lib http::sse::tests`. All use one test thread.
Default-feature compilation is the separate `cargo check --offline --locked
--all-targets` gate. Every final command exited zero and all native gates are
terminal. Reduced-feature warnings remain outside the scoped owners.

An initial harness compile used the package name `gateway` as its library import.
The two imports were corrected to the declared `neuro_gateway` crate before the
actual behavioral baseline. The original compile failure and contract are retained;
`contract.v2.json` pins the unchanged baseline/final source at SHA-256
`363f8851832dd489ccc69af890ecbc39f7a439f0fba252d91359809432134f55`.

Immutable evidence: `target/effective-line-evidence/20260912-stream-state/scope.json`.
Capture time: 2026-09-12 01:15:12 UTC. The prior stream-extraction scope is unchanged.
Strict scans 1,491 files: 31 hard, 41 mandatory and 40 soft; 72 remain above 700.
Clearance stays 73/145 (50.3%). Global formatting reports only the unchanged S06
runtime-mirror source/test files; strict remains nonzero for the recorded debt.

The pre-documentation capture records Gateway HEAD
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`, with 1,621 status entries: 164 modified,
one unstaged deletion, two staged deletions and 1,454 untracked. Neuro HEAD remains
`bf818f0324024634bc890585efb78cc8e603d11a`, with 192 entries: 10 modified and
182 untracked. Inherited changes and staged deletions are retained.

## Remaining boundaries

SSE observation-buffer admission, accumulated frame storage and work per poll
remain unchanged by this checkpoint. It does not impose an overall process-memory
bound or exercise live provider cleanup. S06 retains its implementation and
original cursor; GWP-20260912-01 remains pending. No provider, browser, packaged
runtime or release was exercised by these gates. No new release was built.
