# Bedrock Converse protocol extraction

Verified: 2026-09-12, Asia/Shanghai. Owner: parallel coordinator.
State: `structurally_verified`.
Scope: [Bedrock protocol lane](../plan/parallel-lanes/bedrock-protocol.md).

## Source result

`src/protocol/bedrock_converse.rs` decreased from 772 to 78 effective lines.
The entry retains response assembly, finish-reason mapping and endpoint paths.
Three private owners contain request packing, request normalization and the
stream state/encoder. Explicit re-exports preserve the existing public API.

| Scoped source/test file | Effective lines |
| --- | ---: |
| `src/protocol/bedrock_converse.rs` | 78 |
| `bedrock_converse/packing.rs` | 167 |
| `bedrock_converse/normalization.rs` | 206 |
| `bedrock_converse/eventstream.rs` | 266 |
| `bedrock_converse/tests.rs` | 87 |
| `tests/bedrock_converse_contract.rs` | 177 |
| `tests/bedrock_eventstream_contract.rs` | 143 |

All seven files are at most 266 effective lines; no exception or baseline change
was needed. The immutable original hash is
`22bd507f74f02aac0bb1f782bd59acee5448323aeee9bdd4062689a89666eb9e`.
Counts, hashes, paired-test hashes and separate Git states are recorded in
[`scope.json`](../../target/effective-line-evidence/20260912-bedrock-protocol/scope.json).

## Preservation and scoped review

`extract-bedrock.mjs --verify` compares all 15 moved production items, three
retained functions, three original tests and their fixture. The entire stream
state implementation is one preserved item. No production body or visibility
changed. Only original test imports were expanded after moving their module.

The entry and packing/normalization owners remain pure transformations. HTTP
authentication, header/query propagation and ingress body admission remain in
the existing HTTP/pipeline owners. Raw fields, JSON content, tool identities,
system text, model precedence and response envelopes retain their behavior.
The extraction adds no client, task, lock, I/O, queue, clone or allocation.
The stream continues to own and drop its upstream within the returned stream.

The extraction review identified the following limitations. The first two are
now covered by the separate [stream hardening acceptance](2026-09-12-bedrock-stream.md);
the immutable extraction evidence and measurements above are unchanged.

- Applying complete-argument `normalize_tool_args` to streaming fragments trims
  whitespace and replaces empty fragments. The subsequent paired regression/fix
  preserves JSON string contents across tool deltas.
- The input buffer, parsed SSE event, queued output burst and retained tool map
  had no explicit per-stream budget. Queue backpressure and retained-state
  admission are now verified by the subsequent hardening batch.
- Tool-result name lookup keeps its original reverse history scan. No complexity
  improvement is claimed for large synchronous conversation histories.
- Binary length/CRC tests preserve the current zero-header JSON envelopes. They
  do not establish native AWS SDK interoperability or real-provider readiness.

## Fresh verification

| Gate | Observed result |
| --- | --- |
| Original implementation unit tests | 3 passed |
| Original implementation public contracts | 5 request/response + 4 stream tests passed |
| Extracted implementation unit tests | Same 3 passed |
| Extracted implementation public contracts | Same 9 passed; source hashes unchanged |
| `cargo check --offline --locked --all-targets` | Passed |
| Scoped official Rust formatter | Passed |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | Passed |
| UTF-8 without BOM and scoped whitespace | Passed |
| Gateway and Neuro `git diff --check` | Passed independently |

The Cargo commands use `--offline --locked`, the unit filter
`--lib protocol::bedrock_converse::tests`, and the integration targets
`--test bedrock_converse_contract --test bedrock_eventstream_contract`, all with
`-- --test-threads=1`. Logs are under
`target/effective-line-evidence/20260912-bedrock-protocol/`.

Stream contracts validate total length, prelude/message CRC, exact ordered
payloads across one-byte/small/full chunks, multibyte UTF-8, single tool starts,
argument delta order, queued-output-before-error behavior and downstream drop.
Fixtures perform no network or provider request. Full-tree formatting still
reports only the two S06 runtime-mirror files; owned formatting is green.

## Remaining plan and release boundary

Strict scans 1,410 files: 31 hard, 52 mandatory and 40 soft. There are 83 files
above 700, down from 84; structural clearance is 62/145 (42.8%). Strict remains
red for the remaining debt. This checkpoint does not close the full plan.

No release was built or modified. All coordinator native gates are terminal.
S06 retains its source and original cursor; GWP-20260908-06 still needs the
explicit source/docs freeze and shared release-build transfer receipt.
