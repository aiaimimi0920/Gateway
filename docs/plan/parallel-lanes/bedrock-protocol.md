# Bedrock Converse protocol ownership

Owner: parallel coordinator. State: `structurally_verified`.
Started and verified: 2026-09-12.

Scope is `src/protocol/bedrock_converse.rs` (772 effective lines), its private
`bedrock_converse/` directory and the two Bedrock contract integration targets.
The HTTP handlers, shared SSE parser, canonical types, S06 source and release
ownership remain outside this extraction.

Separate request packing, request normalization and event-stream state/encoding.
Keep response assembly, finish-reason mapping and endpoint paths in the entry,
with explicit public re-exports preserving every existing caller path. Move
complete items, including the stream-state implementation, without changing
allocation, event order, failure behavior or lifetime.

Before production moves, preserve the three original unit tests and establish
public request/response characterization plus binary stream contracts. Cover
model precedence, system text, tool identities, passthrough fields, raw content,
invalid input, one-byte UTF-8 fragmentation, CRC framing, tool delta ordering,
upstream errors and downstream drop. Repeat the same tests after extraction,
then run all-targets compilation, formatter checks, checker tests, ratchet,
strict inventory, encoding checks and separate Gateway/Neuro Git checks.

Immutable original source and comparison evidence are under
`target/effective-line-evidence/20260912-bedrock-protocol/`. All completed owners
must stay below 500 effective lines. These contracts preserve current behavior;
AWS SDK interoperability and buffer/output resource bounds require separate
acceptance. Integrated release still needs the explicit GWP-20260908-06
source/docs freeze and shared release-build transfer receipt.

The entry is now 78 effective lines, with packing 167, normalization 206 and
event-stream state/encoding 266. All seven owned source/test files are at most
266. Complete comparison preserves 15 moved items, three retained functions,
three original tests and their fixture; the nine new contract tests retain the
same source hashes across both gates. Paired unit 3/3 and public contracts 9/9,
all-targets compilation, scoped formatter, checker 19/19, ratchet, encoding and
independent Git checks pass. Strict has 83 files above 700, down from 84.

The separate [stream hardening batch](bedrock-stream.md) now verifies verbatim
tool fragments, decoder/output/tool-state budgets and terminal upstream cleanup.
Its intentional stream changes have independent evidence; the extraction
snapshots and counts above remain historical. Native AWS SDK interoperability
remains open. See the [structural acceptance report](../../status/2026-09-12-bedrock-protocol.md)
and [hardening acceptance report](../../status/2026-09-12-bedrock-stream.md).
