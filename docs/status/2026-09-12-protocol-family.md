# Protocol registry and routing family extraction

Verified: 2026-09-12, Asia/Shanghai. Owner: parallel coordinator.
State: `structurally_verified`.
Scope: [Protocol family lane](../plan/parallel-lanes/protocol-family.md).

## Source result

The registry entry decreased from 1,314 to 91 effective lines; routing protocol
resolution decreased from 1,219 to 80. All 15 owned source/test files are at most
322 effective lines. No baseline or exception changed.

| Scoped file | Effective lines |
| --- | ---: |
| `src/protocol/registry.rs` | 91 |
| `registry/families.rs` | 274 |
| `registry/profiles.rs` | 221 |
| `registry/profile_inference.rs` | 247 |
| `registry/tests.rs` | 322 |
| `registry/official_profile_tests.rs` | 222 |
| `src/routing/protocol_resolution.rs` | 80 |
| `protocol_resolution/surfaces.rs` | 143 |
| `protocol_resolution/model_policy.rs` | 204 |
| `protocol_resolution/request_compatibility.rs` | 224 |
| `protocol_resolution/tests.rs` | 108 |
| `protocol_resolution/tests/candidate_selection.rs` | 196 |
| `protocol_resolution/tests/text_bridges.rs` | 103 |
| `protocol_resolution/tests/gemini_bridges.rs` | 234 |
| `tests/protocol_family_contract.rs` | 97 |

The registry separates family aliases/selectors, profile defaults and ordered
profile inference. Its constants and family inference remain at the public entry.
Routing keeps candidate mutation and route-policy matching at the entry, with
separate surface capabilities, model allow/block policy and request compatibility.
Private modules are connected by explicit imports and existing public re-exports.

## Preservation and scoped review

The complete extraction proof preserves 34 moved functions, four retained entry
functions, all 48 constants, 54 original tests and two fixture helpers. Registry
and routing retain all 19 and six public function paths respectively. The only
visibility adjustment is `pub(super)` on request compatibility, keeping it inside
its original routing-resolution family. Original test bodies are unchanged after
their import/module relocation.

The new public model-policy suite passed 5/5 before any production edit and remains
byte-identical afterward. It covers case/whitespace-normalized model aliases and
upstream names, terminal wildcard matching without substring matches, global and
model block precedence, unsupported explicit families, stable first-seen ordering,
nested/string family inputs and empty model-value fallback. Paired test SHA-256:
`78c42ed6774acb4c6de5061c6a181e091a281ac71c44ac52008bdb3b440363df`.

All production owners remain synchronous transformations. No task, lock, I/O,
queue, client, dependency or new allocation was introduced. Priority saturation,
family ordering, endpoint compatibility and model-policy precedence are preserved.
Existing base-URL substring heuristics remain profile inference, with no new URL
validation or security claim. Recursive family-array traversal and linear
deduplication retain their existing complexity and input ownership.

Original snapshots, exact-item/public-path proofs, paired counts, source hashes,
encoding checks and separate Git states are in the immutable
[scope record](../../target/effective-line-evidence/20260912-protocol-family/scope.json).
The direct caller `src/routing/credential_routing.rs` also matches the pre-extraction
inventory hash; it was validated without source edits.

## Fresh verification

| Gate | Observed result |
| --- | --- |
| Registry library tests, before and after | Same 22 passed |
| Routing-resolution library tests, before and after | Same 32 passed |
| Credential-routing caller tests, before and after | Same 26 passed |
| Public model-policy contracts, before and after | Same 5 passed |
| `cargo check --offline --locked --all-targets` | Passed |
| Scoped official Rust formatter | Passed |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | Passed |
| Scoped UTF-8 without BOM and whitespace | Passed |
| Gateway and Neuro `git diff --check` | Passed independently |

Cargo used `--offline --locked`, the library filters `protocol::registry::`,
`routing::protocol_resolution::tests`, `routing::credential_routing::tests`, and
`--test protocol_family_contract`, all with `-- --test-threads=1`. Logs are under
`target/effective-line-evidence/20260912-protocol-family/`. Registry tests keep the
default feature set required by their existing compiled-capability expectations.

Full-tree formatting still reports only the S06-owned runtime-mirror source and
test files. Other S06 compile warnings remain outside this source scope. Both
repositories retain their inherited dirty state and staged deletions.

## Remaining plan and release boundary

Strict scans 1,423 files: 31 hard, 50 mandatory and 40 soft. There are 81 files
above 700, down from 83. Structural clearance is 64/145 (44.1%); strict and the
overall optimization plan remain open.

All coordinator native gates are terminal. S06 retains its implementation and
original cursor. GWP-20260908-06 still needs the explicit source/docs freeze and
shared release-build transfer receipt. No release or live-provider gate ran.
