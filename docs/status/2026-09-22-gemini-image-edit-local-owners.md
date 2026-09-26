# Gemini Canvas image-edit local helper owners

This checkpoint completes the next Gateway large-file migration batch for
`src/upstream/gemini_canvas_image_edit_local_helpers.rs`.

## Ownership split

The parent helper now keeps upload configuration, error contracts, upload
headers and resource-path parsing, runtime-mirror wiring, and a small facade.
The signaler lifecycle and HTTP transport live in separate owners:

- `gemini_canvas_image_edit_signaler.rs`: bootstrap, prewarm, poll-state and
  signaler state transitions (430 effective lines).
- `gemini_canvas_image_edit_signaler_transport.rs`: signaler request, polling,
  credential refresh, and bounded body collection (244 effective lines).

The former inline test module is registered as focused owners:

- `gemini_canvas_image_edit_local_helpers_error_tests.rs`: error contracts
  (257 effective lines).
- `gemini_canvas_image_edit_local_helpers_transport_tests.rs`: async return
  shapes and upload transport contract (265 effective lines).
- `gemini_canvas_image_edit_local_helpers_state_tests.rs`: signaler state and
  handoff contracts (467 effective lines).
- `gemini_canvas_image_edit_local_helpers_asset_tests.rs`: media asset and
  bootstrap merge contracts (181 effective lines).
- `gemini_canvas_image_edit_local_helpers_contract_tests.rs`: upload URL,
  headers, runtime key, and resource-path contracts (197 effective lines).
- `gemini_canvas_image_edit_local_helpers_snapshot_tests.rs`: debug snapshot
  and response-shape contracts (460 effective lines).
- `gemini_canvas_image_edit_local_helpers_test_support.rs`: shared payload
  fixture.

The parent facade re-exports the signaler owners so existing `client.rs`,
upload, follow-up, and trace-writer call paths retain their module paths. The
existing snapshot and runtime-mirror owners remain unchanged.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo test --offline --locked upstream::gemini_canvas_image_edit_local_helpers --lib -- --test-threads=1`: 74 passed, 0 failed.
- `cargo check --offline --locked --lib`: passed.
- `cargo check --offline --locked --all-targets`: passed.
- `npm run test:effective-lines --prefix scripts`: 19 passed.
- `npm run check:effective-lines --prefix scripts`: ratchet passed; every new
  owner is below 500 effective lines.
- Scoped `git diff --check`: passed.

This is a source/test ownership checkpoint only. No Gateway release artifact or
runtime deployment was produced by this batch.
