# Gemini Business test-owner checkpoint

This batch continues the Gateway large-file migration after the Gemini auth
session owners. The Gemini Business production implementation was already below
the 700-line ceiling; its oversized footprint came from the embedded test
module. The tests are now split into focused owners while the production module
and public protocol surface remain unchanged.

## Ownership and line counts

Counts use the repository's authoritative effective-line lexer.

| Owner | Before | After |
| --- | ---: | ---: |
| `src/protocol/gemini_business.rs` | 1,244 | 485 |
| `src/protocol/gemini_business_errors.rs` | New | 61 |
| `src/protocol/gemini_business_test_support.rs` | New | 89 |
| `src/protocol/gemini_business_error_tests.rs` | New | 156 |
| `src/protocol/gemini_business_normalization_tests.rs` | New | 265 |
| `src/protocol/gemini_business_response_tests.rs` | New | 207 |

The production parent retains all protocol constants, errors, types, request
normalization, runtime extraction, request plans, response construction, and
private JSON/upload helpers. Test support owns shared payload/request/runtime
fixtures. Error tests cover Gateway error contracts and local plan rejection;
normalization tests cover request/runtime/plans; response tests cover generated
files, uploads, prompt/format validation, response envelopes, and model alias
behavior.

No public function, type, protocol wire contract, or external module path
changed. Tests remain registered beneath `protocol::gemini_business` through
explicit path modules.

## Preservation and regression evidence

The source projection preserves all original production functions and all 32
original Gemini Business tests. Only module placement, imports, and test-module
names changed. The extraction review covered request validation, runtime field
aliases, upload parsing, response format bounds, deduplicated file extraction,
rate-limit/error mapping, and request-plan URL/body construction.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo test --offline --locked protocol::gemini_business --lib --
  --test-threads=1`: 32 passed, 0 failed.
- `cargo check --offline --locked --all-targets`: passed.
- Effective-line scan: parent 542; new owners 89, 156, 265, and 207. All are
  below 500 effective lines and no exception is required.
- Existing unrelated warning remains in
  `src/upstream/gemini_canvas_image_edit_local_helpers.rs` (`HashMap`).

The repository-wide strict inventory remains red for the other historical
Rust owners and browser-profile/runtime payloads. No release package was built
or deployed for this atomic test extraction.

## Remaining scope

The active Rust large-file debt is now concentrated in the two reserved S06 hubs
(`src/upstream/client.rs` and `src/protocol/gemini_canvas.rs`), the two Gemini
Canvas helper files, and the remaining browser-profile/runtime payload policy
decision. Full provider/UI/runtime/release acceptance remains pending.

Gateway remains a dirty, uncommitted working tree with inherited changes
preserved. This batch owns the modified Gemini Business parent, five new source/test
owners, and this report; other subprojects received no edits or validation
claims.
