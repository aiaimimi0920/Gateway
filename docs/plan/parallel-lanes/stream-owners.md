# Upstream stream observation ownership

Owner: parallel coordinator. State: `accepted`.
Started: 2026-09-12.

Scope is `src/upstream/stream.rs` and its private `stream/` production/test owners.
The complete 1,005-physical-line source and tests were read before extraction. The
entry measures 802 effective lines. Caller sampling covers the HTTP SSE response
and pipeline placement of usage, completion semantics, archive and final callbacks.

Keep metrics and tracked-stream lifecycle in the entry. Move token usage,
completion semantics and bounded archive capture into independent leaves, each
owning its wrapper, parser/capture state and public snapshot functions. Preserve
all public paths, concrete type definitions, method bodies, byte forwarding,
callback timing, parser ordering, locks and archive limits. No visibility promotion
or shared module change is planned. Preserve all 14 original tests and fixtures.

Evidence: `target/effective-line-evidence/20260912-stream-owners/`. The immutable
pre-edit snapshot includes 94 sources/inputs. Projected owners measure
106/243/165/94/215 effective lines. Final acceptance requires complete item/source
proof, paired default-feature stream and HTTP SSE tests, all-targets compilation,
scoped formatting, checker/ratchet/strict, encoding and independent Git checks.
Fresh default-feature baselines passed: stream 14/14 and HTTP SSE 3/3. Extraction
is complete with the projected counts. Exact comparison preserves 25 moved and
five retained items: 11 structs, nine implementations and ten free functions,
including ten public paths. All original tests, two fixture functions and one
test type alias remain unchanged. No visibility promotion or shared module edit
was needed. All 93 neighboring files match the snapshot. Fresh final default-feature
stream 14/14 and HTTP SSE 3/3, all-targets, scoped formatting, checker 19/19,
ratchet, encoding and both Git checks pass. All native gates are terminal.

Immutable scope: `target/effective-line-evidence/20260912-stream-owners/scope.json`,
captured at 2026-09-12 00:13:05 UTC. Strict scans 1,490 files: 31 hard, 41 mandatory
and 40 soft; 72 remain above 700. Clearance is 73/145 (50.3%). Global formatting
still reports only the unchanged S06 runtime-mirror source/test files.

[Acceptance report](../../status/2026-09-12-stream-owners.md).

This batch is structural. Existing SSE observation buffer growth, arithmetic
bounds and additional archive/lifecycle coverage remain separate follow-ups.
S06 retains its implementation and original cursor. GWP-20260912-01 remains a
pending transfer request; no shared release build or runtime-profile edit follows.
