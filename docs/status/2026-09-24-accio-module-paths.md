# Accio shared parser module-path regression

Status: repaired and focused verification passed. The complete provider-feature
matrix is rerunning; this checkpoint does not close S06, S18, S20 or release
acceptance.

## Failure and ownership

The first integration matrix passed Accio's enabled protocol and upstream filters,
then failed while compiling `line-aistudio-official` with default features disabled.
That feature loads `src/protocol/accio_disabled.rs`, which imports the shared parser
using `#[path = "accio/line/event_parse.rs"]`.

The shared parser's implicit `mod stream_events` and `mod tests` declarations used
the wrong child directory in that inclusion mode. Compilation could not find the
stream-event module and incorrectly loaded `accio/line/tests.rs`, producing 27
errors. The default-feature Rust suite did not exercise this disabled-facade path.

The repair gives both children explicit paths under `event_parse/`. It changes
only `src/protocol/accio/line/event_parse.rs`: 261 to 263 effective lines. No parser
body, test body, exported API, wire shape or compiled-out response changes.
Removing the two attributes and their explanatory comment reproduces the exact
pre-edit SHA-256:
`b0bbdd7a70e48207d795e52994b2d45b1164329325920355920ec6b86e918512`.

## Verification

Evidence root:
`target/effective-line-evidence/20260924-integration-closure/accio-module-2026-09-24T13-59-25-189Z/`.

- `cargo test --locked --lib --no-default-features --features line-aistudio-official
  protocol::accio -- --test-threads=1`: 8 passed, zero failed or ignored. This runs
  the shared parser's four scenarios and the disabled facade's four contracts.
- The same feature with filter `protocol::gemini_api`: 1 passed, zero failed.
- `cargo test --locked --lib --no-default-features --features line-accio-web-reverse-api
  protocol::accio -- --test-threads=1`: 19 passed, zero failed or ignored.
- `cargo fmt --all -- --check`, checker 19/19, ratchet and `git diff --check`: passed.
- Exact source remainder, UTF-8 without BOM and unchanged source during proof:
  passed, recorded in `source-proof.json` and `summary.json`.

The original matrix failure remains under
`line-matrix-2026-09-24T13-25-04-130Z-57ed4480/`; the full rerun uses the existing
serialized product-gate runner and shared Cargo target. Its result must be checked
before describing the whole matrix as passed. Earlier complete Rust test evidence
predates this module-path repair.

## Runtime boundary

The live development container remained healthy at loopback port 4200 with
`GATEWAY_DEV_WATCH=0`. A Rust source-path edit does not replace its running binary.
No browser profile, route, credential, release or live process was modified. The
change adds no input, allocation, async operation or resource owner.
