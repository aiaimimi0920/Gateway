# Cohere packing owner

The Cohere protocol adapter's request/response packing responsibility moved to
`src/protocol/cohere/packing.rs`. The owner contains message, tool, tool-call,
content-part and tool-choice serialization, plus the chat-v2 request and success
response builders. Stream decoding, normalization, event translation and state
remain in `cohere.rs`.

The extraction follows the existing function bodies and keeps the parent module's
public functions and signatures unchanged. The helper `map_cohere_finish_reason`
is consumed through the child module's parent visibility; no finish mapping or
JSON field was changed.

Effective lines:

- `src/protocol/cohere.rs`: 621 -> 488.
- `src/protocol/cohere/packing.rs`: 131.

Both owners are below 500 effective lines. No protocol, dependency, manifest,
baseline or exception configuration changed.

## Verification

- `rustfmt --edition 2024` on both changed Rust files: passed.
- Effective-line checker tests: 19/19 passed.
- Effective-line ratchet: passed; 2260 scanned, 10 above 1500, 18 between 701
  and 1500, 23 between 501 and 700. This clears the Cohere soft-debt entry.
- Gateway `git diff --check`: passed.

A focused `cargo test --locked protocol::cohere -- --test-threads=1` was started,
but the shared Rust build exceeded the 120-second execution window without a
diagnostic. No Rust test success is claimed for this checkpoint. Repository-wide
formatter output still contains unrelated historical files outside this scope.

The extraction does not harden provider validation, stream bounds or error
handling. Full strict closure, integrated runtime/provider validation, S06
transfer and release publication remain open. Existing dirty/staged state and
sibling projects were preserved.
