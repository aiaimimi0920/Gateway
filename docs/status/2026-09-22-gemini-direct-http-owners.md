# Gemini Canvas direct HTTP owners

This checkpoint completes the direct HTTP helper extraction from
`src/upstream/gemini_canvas_direct_http_helpers.rs`.

## Ownership split

The original 2,044 effective-line implementation is now a small facade with
four production owners:

- `gemini_canvas_direct_http_contract_helpers.rs`: bootstrap candidates,
  request contracts, path and preflight resolution, headers, and redaction
  helpers (433 effective lines).
- `gemini_canvas_direct_http_error_helpers.rs`: direct HTTP, page-harvest, and
  media error constructors (118 effective lines).
- `gemini_canvas_direct_http_page_harvest_helpers.rs`: bounded redirect-aware
  page harvesting and session refresh (254 effective lines).
- `gemini_canvas_direct_http_json_helpers.rs`: signed JSON request transport,
  response validation, and invalid-JSON handling (224 effective lines).

The facade re-exports the same `crate::upstream::gemini_canvas_direct_http_helpers`
symbols used by `client.rs`, image-edit helpers, and official API helpers.

The former inline test module is registered as focused owners:

- contract bootstrap tests (182 effective lines);
- contract resolution/header/redaction tests (375 effective lines);
- error contract tests (326 effective lines);
- page-harvest async shape tests (61 effective lines);
- JSON transport and invalid-JSON tests (59 effective lines);
- shared payload fixture (12 effective lines).

All new owners remain below 500 effective lines and no exception record was
added.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo test --offline --locked upstream::gemini_canvas_direct_http_helpers --lib -- --test-threads=1`: 55 passed, 0 failed.
- `cargo check --offline --locked --lib`: passed.
- `cargo check --offline --locked --all-targets`: passed.
- `npm run test:effective-lines --prefix scripts`: 19 passed.
- `npm run check:effective-lines --prefix scripts`: ratchet passed; every new
  direct HTTP owner is below 500 effective lines.
- Scoped `git diff --check`: passed.

This is a source/test ownership checkpoint only. No Gateway release artifact or
runtime deployment was produced by this batch.
