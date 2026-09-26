# S06 image ingress characterization

## Scope and outcome

Added `tests/image_edit_ingress_limits.rs`: 220 effective / 232 physical lines,
one responsibility, real-router JSON/multipart body-limit contracts. Production
handlers, limits, dependencies, credentials and releases are unchanged. S06 still
owns its returned source/Cargo window; coordinator browser-script lanes remain
separate. This step completes ingress characterization, not all ingress hardening.

Fresh combined test command:

`cargo test --offline --locked --test ingress_extractor_limits --test image_edit_ingress_limits`

Result: 6 registered tests passed, 0 failed/ignored. The earlier miniature
extractor/layer suite contributes 12 table cases; the real router suite adds
28 cases (8 JSON, 16 multipart, 4 configured route-limit cases). The final run
took 0.13s and 0.47s respectively after the warm build. All-target check passed
in 4.59s. Formatter, checker tests19/19, ratchet and diff checks passed.

## Proven behavior

- Both `/v1/images/edits` and `/v1/new-api/images/edits` enforce the additional
  Axum 2 MiB extractor limit even with route/global caps configured to 4 MiB.
- Oversized JSON returns HTTP413 / `request_too_large`. Below-limit JSON without
  prompt reaches validation and returns `missing_required_field` before pipeline.
- Oversized multipart currently returns HTTP400, not the JSON413 contract.
  The error context depends on the actual polling boundary: eager available
  chunks fail in next_field with `Invalid multipart image edit body`; headers
  ready immediately followed by a pending body chunk reach the prompt/image
  reader and fail with `Invalid prompt field` / `Invalid uploaded image data`.
- Below-limit prompt-only multipart reaches `missing_input_image`; image-only
  multipart reaches `missing_required_field` before decoding/provider execution.
- Configured route caps below the extractor default still reject oversized
  bodies on both routes, with/without Content-Length.

The fixture uses an explicit Config, empty route store, disabled automation and
lazy Redis pointing to 127.0.0.1:1. It does not read provider configuration from
environment, start a service, send a provider request, or mutate image handlers.
All successful extraction cases intentionally fail validation before run_pipeline.

## Fixture corrections and root-cause evidence

Initial characterization assumptions failed three times (2 passed/1 failed each).
These were test-fixture assumptions, NOT red demonstrations of a production fix.
Do not report them as a fixed product regression.

The first expected all over-limit failures to happen in field.text/bytes. Actual
eager prefetch failed earlier in next_field. A second attempt distinguished chunk
count but both chunks were immediately ready. Yielding before every chunk still
failed early. Exact local dependency review explained the final subtlety:

- multer3.1.0 `src/multipart.rs:254` prefetches before parsing a field.
- In FindingFirstBoundary, if initially no bytes are available, `:262` polls the
  stream a second time in the same call; it does not redo boundary parsing then.
- A yield before the first chunk can therefore leave headers unparsed until the
  next outer poll, whose initial prefetch can already observe the oversized chunk.
- The final fixture emits its first chunk immediately and yields only before the
  second. This deterministically exposes the field-reader boundary, without sleep
  timing, extra tasks or network scheduling.
- multer `src/buffer.rs:32-55` drains ready chunks until Pending; tower-http0.5.2
  `src/limit/service.rs:48-62` only wraps the body and checks declared length, not
  an eager Gateway-side full-body rebuild.

The scout's inference that yielding before EVERY chunk necessarily reaches field
parsing was contradicted by execution and rejected after the exact-source review.

## Review and remaining work

Input bodies are fixed synthetic sizes, per-case owned and sequentially consumed;
stream slices share Bytes storage. No external input controls the fixture sizes.
No new production behavior, blocking worker or background resource was introduced.

The latest concurrent ratchet scan saw1025 files,44 hard+78 mandatory=122 over700,
38 soft. This changing structure debt reflects coordinator lanes, not these tests.
No baseline/policy/exception change or debt reduction is claimed by S06.

Do not blindly disable the production extractor default: that widens accepted
image inputs before pixel/decode/concurrency limits are finished. Multipart413
normalization is a separate behavior decision and must not be hidden in structural
refactoring. Next S06 priority is the already confirmed enabled-trace synchronous
image decode/hash/write boundary with cancellation-safe bounded admission.
Full S06/S07-S21 and live-provider/visual/product completion remain unproven.
