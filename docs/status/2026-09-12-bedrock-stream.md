# Bedrock stream correctness and bounded admission

Verified: 2026-09-12, Asia/Shanghai. Owner: parallel coordinator.
State: `hardened_verified`.
Scope: [Bedrock stream lane](../plan/parallel-lanes/bedrock-stream.md).

## Reproduced defects and changes

The unchanged 13-test stream suite first produced five passes and eight failures
against the accepted structural extraction. After this patch, all 13 pass.
Streaming tool arguments now retain whitespace and empty fragments verbatim;
complete-argument normalization previously corrupted both cases.

The stream reuses `BoundedSseDecoder`, draining the output of one input event
before decoding the next. It enforces the existing 64 MiB SSE frame limit,
64 MiB queued output, 8,196 queued output events per input event, 4,096 retained
tool calls and 4 MiB aggregate retained tool ID/name bytes. Replacing an identity
releases its old byte budget. Empty identities and fallback UUID behavior remain
unchanged. Encoder length checks precede conversion and fallible reservation;
tool indices must fit both `usize` and the signed `i64` output field.

Admission and transport errors are terminal. The `unfold` continuation drops
upstream, decoder and pending output before yielding an error. Failed input
events publish none of their partially queued output; completed earlier events
retain their ordering. Static admission diagnostics do not echo input. EOF still
does not flush an unterminated pending event. No task, lock, client or dependency
was added.

## Scope and preservation evidence

| File | Before effective lines | After effective lines |
| --- | ---: | ---: |
| `src/protocol/bedrock_converse/eventstream.rs` | 266 | 329 |
| `tests/bedrock_eventstream_contract.rs` | 143 | 269 |

Both files stay below 500 effective lines. The stream owner adds 63 lines for
admission accounting and terminal cleanup within the existing resource owner.
No baseline or exception was changed.

The final immutable [scope record](../../target/effective-line-evidence/20260912-bedrock-stream/scope.json)
records hashes, counts and independent Git states. `capture-scope.mjs` verifies
five neighboring Bedrock source/test files against the accepted extraction,
the unchanged tool identity type, all eight original stream test/helper items
including four tests, and the exact paired 13-test source used at baseline.
The paired test SHA-256 is
`2f57c1eb3a028b8e10c480156d47542afc932c260df6ac7bf3bc8e24f741cb09`.

The earlier [structural acceptance](2026-09-12-bedrock-protocol.md) and its
snapshots remain immutable. Its stream body comparison describes that earlier
extraction; intentional changes here are verified by this separate evidence.

## Fresh verification

| Gate | Observed result |
| --- | --- |
| Paired stream baseline | 5 passed, 8 failed |
| Final public stream contracts | Same 13 passed |
| Request/response contracts | 5 passed |
| Original Bedrock unit tests | 3 passed |
| Shared bounded-decoder tests | 9 passed |
| `cargo check --offline --locked --all-targets` | Passed |
| Scoped official Rust formatter | Passed |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | Passed |
| Scoped UTF-8 without BOM and whitespace | Passed |
| Gateway and Neuro `git diff --check` | Passed independently |

Cargo used `--offline --locked`, the two Bedrock integration targets, and the
`protocol::bedrock_converse::tests` and `protocol::stream_decode::tests` library
filters with `-- --test-threads=1`. Final logs are `final-contracts.log`,
`final-unit.log`, `decoder-tests.log` and `final-all-targets.log` under
`target/effective-line-evidence/20260912-bedrock-stream/`.

The contracts exercise framing/CRC/order, chunk fragmentation, verbatim tool
fragments, identity replacement, each admission boundary, oversized indices and
terminal upstream drop. They use deterministic streams without provider I/O.
Full-tree formatting still reports only the S06-owned
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`.

## Remaining boundaries

Budgets are per stream. An original transport chunk is allocated upstream of
the decoder's per-event budget. Binary zero-header JSON envelopes are unchanged;
native AWS SDK interoperability and real-provider readiness remain unverified.
Synchronous tool-result history lookup retains its reverse scan.

Strict scans 1,410 files: 31 hard, 52 mandatory and 40 soft; 83 remain above 700.
Structural clearance remains 62/145 (42.8%). All coordinator native gates are
terminal. S06 keeps its source and original plan cursor. GWP-20260908-06 still
needs an explicit source/docs freeze and shared release-build transfer receipt.
No release was built or modified; the overall optimization plan remains open.
