# Bedrock streaming correctness and resource admission

Owner: parallel coordinator. State: `hardened_verified`.
Started and verified: 2026-09-12.

Scope is `src/protocol/bedrock_converse/eventstream.rs` and
`tests/bedrock_eventstream_contract.rs`. The structural extraction is accepted
separately; its immutable evidence must not be rewritten to hide these behavior
changes. Other Bedrock request/response owners, HTTP routes, shared decoder,
S06 files and release ownership remain outside this patch.

Reproduce tool-fragment whitespace/empty-fragment corruption, unbounded decoder
and output admission, retained tool identity growth, signed-index overflow and
upstream retention after transport error. Preserve valid framing/event order and
the existing request/response contracts. Reuse the existing bounded SSE decoder
for lazy per-event parsing rather than adding a second parser.

The stream must enforce the shared 64 MiB SSE frame budget, a 64 MiB queued
output budget, 4,096 retained tool calls, 4 MiB retained tool ID/name bytes and
8,196 queued events per input event. Admission errors must be terminal, avoid
publishing a partial failed event, and release upstream/state before yielding the
error. Resource limits are per stream; allocation supplied by the upstream
transport and native AWS SDK interoperability are separate boundaries.

Original files and paired regression evidence are under
`target/effective-line-evidence/20260912-bedrock-stream/`. New/modified owners
must remain below 500 effective lines. Run the same regressions, existing public
contracts, shared-decoder tests, all-targets compilation, official formatter,
checker tests, ratchet, inventory and independent Git/encoding checks.

The same stream suite advances from five passes/eight failures to 13/13 passing.
Request/response 5/5, original Bedrock unit 3/3, shared decoder 9/9, all-targets
compilation, scoped formatter, checker 19/19, ratchet, encoding and independent
Git checks pass. Production/test owners are 329/269 effective lines. Five
neighboring files, the tool identity type and all original stream test/helper
items are preserved; paired test source matches the failing baseline exactly.

Strict remains at 83 files above 700. S06 runtime-mirror formatting and the
GWP-20260908-06 source/docs freeze and release-build transfer remain open. No
release was built. See the [acceptance report](../../status/2026-09-12-bedrock-stream.md).
