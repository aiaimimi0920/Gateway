# Upstream stream observation ownership

Accepted on 2026-09-12. Stream metrics and callback lifecycle stay in the entry;
token usage, completion semantics and bounded archive capture each own their
wrapper and state in a private leaf. This structural checkpoint preserves behavior
and leaves the overall Gateway plan in progress.

## Scope and preservation

`src/upstream/stream.rs` decreases from 802 to 106 effective lines. The usage,
completion, archive and original test owners measure 243, 165, 94 and 215. All five
scoped files are below 500. The complete original source and tests were read before
editing; caller sampling covered pipeline wrapper order and HTTP SSE publication.

Complete comparisons preserve 25 moved and five retained production items:
11 structs, nine implementation blocks and ten free functions. All ten public
paths, 14 original tests, two fixture functions and one test type alias remain
unchanged. Each private leaf owns its state, avoiding cross-leaf dependencies or
new visibility promotions. The upstream module declaration stays unchanged.

Byte forwarding, callback timing, metrics, locks, usage merging, completion
mapping and archive admission are preserved. All 93 neighboring source/input files
match their pre-edit hashes, including caller modules, the accepted error/media
owners, dependency manifests, policy, baseline, exceptions and S06 formatting files.

## Verification

- Paired default-feature stream tests: 14/14 before and after.
- Paired default-feature HTTP SSE caller tests: 3/3 before and after.
- Fresh default-feature all-targets compilation: passed.
- Scoped official formatter, checker tests 19/19 and ratchet: passed.
- Full source/item/test/neighbor proof, UTF-8 without BOM and whitespace: passed.
- Gateway and Neuro Git diff checks: passed independently.

The test commands are `cargo test --offline --locked --lib upstream::stream::tests`
and `cargo test --offline --locked --lib http::sse::tests`, both followed by
`-- --test-threads=1`. Compilation uses `cargo check --offline --locked --all-targets`.
All native gates are terminal and every listed command exited zero. Only inherited
Gemini warnings remain; none names a scoped owner.

Global formatting reports only the unchanged S06 runtime-mirror source/test files.
Strict scans 1,490 files: 31 hard, 41 mandatory and 40 soft. The 72 remaining files
above 700 keep strict at its expected nonzero exit. Clearance is 73/145 (50.3%).

Immutable evidence: `target/effective-line-evidence/20260912-stream-owners/scope.json`.
Capture time: 2026-09-12 00:13:05 UTC. Earlier accepted scope artifacts remain
unchanged; subsequent hardening requires a new evidence directory.

The pre-documentation capture records Gateway HEAD
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`, with 1,617 status entries: 164 modified,
one unstaged deletion, two staged deletions and 1,450 untracked. Neuro HEAD remains
`bf818f0324024634bc890585efb78cc8e603d11a`, with 192 entries: 10 modified and
182 untracked. Inherited changes and staged deletions are retained.

## Follow-up boundaries

Review identified unbounded SSE observation buffers, unchecked usage-token
arithmetic, and archive/terminal lifecycle coverage gaps. Their reproduction and
correction belong to separate hardening checkpoints. This extraction adds no
resource-limit or semantic changes.

S06 retains its implementation and original plan cursor. The GWP-20260912-01
transfer request remains pending. No provider, browser, packaged runtime or
release was exercised by these gates. No new release was built.
