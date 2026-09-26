# Grok packing owner

The Grok adapter's model mapping, request serialization, message flattening and
request-plan construction moved from `src/protocol/grok/line.rs` to
`src/protocol/grok/packing.rs`. The parent retains stream accumulation, NDJSON
translation and translator state. Existing public `pack_grok` and
`build_request_plan` wrappers preserve their signatures and module paths.

Function bodies were moved without changing model mappings, JSON fields, role
labels, URL construction, HTTP method or response kind. The new owner is 84
effective lines; the parent is 549 -> 492, below the 500-line threshold. No
protocol, dependency, baseline or exception policy changed.

Verification:

- Targeted edition-2024 rustfmt for both Grok files: passed.
- Effective-line lexer tests: 19/19 passed.
- Effective-line ratchet: passed; 2261 files, 10 above 1500, 18 between 701
  and 1500, 22 between 501 and 700. This clears the Grok soft-debt entry.
- Gateway `git diff --check`: passed.

Focused `cargo test --locked protocol::grok -- --test-threads=1` was started but
exceeded the 90-second shared build window without diagnostics. No Rust test or
compile success is claimed. Full provider runtime, strict closure, S06 transfer
and release publication remain open. Existing dirty/staged state was preserved.
