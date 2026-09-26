# S06 upload hardening audit during the shared source freeze

Status: read-only findings verified; implementation and runtime regressions pending.
Date: 2026-09-08. HEAD: `4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`.

The preceding goal turn completed upload-HTTP structural verification and posted
the terminal build-window transfer. This turn is investigation progress, not a
verified process wait: no live coordinator job handle was polled. The shared
handoff still has no explicit window return. S06 has run no Cargo/test/formatter
or provider command and has changed no release input during this audit.

Two one-shot read-only scouts located byte/body ownership boundaries. Main read
the exact HTTP owner and sampled the dependency implementations and protocol
normalizer. The findings below change the next implementation, not merely its
status. No candidate below is claimed compiled, benchmarked or deployed.

## 1. Direct bounded-text substitution would change response semantics

- `src/upstream/gemini_canvas_upload_http.rs:123-126,219-222` consumes start and
  finalize replies with `rquest::Response::text`. The text is operational input
  for challenges, session errors, resource extraction and error/debug previews.
- `src/protocol/upstream_body.rs:18-24,49-111` already supplies provider-aware
  byte collection with a 64 MiB declared/streamed limit, checked arithmetic and
  fallible buffer growth. Network failures retain `classify_network_error`.
- Its provider text wrapper at `:40-47,136-141` is lossy UTF-8 only. The separate
  non-provider wrapper at `:26-37` is strict UTF-8. Neither is interchangeable
  with the current upload reader for all inputs.
- Locked rquest is 5.1.0; its default features include `charset`. Local dependency
  `src/client/response.rs:171-175,219-234` parses Content-Type with `Mime`, selects
  the encoding label, falls back to UTF-8, and uses encoding_rs BOM-aware decode.
  UTF-8 BOM removal and windows-1252 are concrete differences from the existing
  provider wrapper, even when ordinary ASCII upload replies look identical.

Decision: do not wire the existing provider-text wrapper directly into upload.
Keep the existing FreeBuff/other text semantics untouched. Reuse bounded bytes,
but characterize charset/BOM behavior before selecting a text adapter.

A candidate requiring no new dependency exists: retain only the Content-Type HeaderValue,
collect bounded bytes, construct an in-memory `axum::http::Response<Vec<u8>>`,
and convert it into rquest Response for its existing `.text()` decoder. Both
locked libraries use http 1.x. rquest's conversion (`response.rs:451-471`) accepts
`Into<Body>` and disables content decoding, so the adapter must not copy transport
headers or perform a second network read. This reuses the actual decoder rather
than reimplementing MIME/BOM selection or adding manifest dependencies. It still
requires compile/runtime proof and has small response-wrapper allocation costs.

The alternative is an explicit charset decoder dependency/API; that is a larger
manifest and semantic maintenance change, not necessary merely to add byte bounds.

### Bounds, deadlines and errors to preserve

- The collector bounds accumulated body bytes, not total process memory. Chunk
  buffers, Vec capacity and decoded UTF-8 output can coexist; charset/invalid-byte
  conversion can expand the resulting text. Do not call this a 64 MiB heap cap.
- rquest `response.rs:41-49,305-311,372-374` makes both `.bytes()` and
  `.bytes_stream()` consume the same timeout-wrapped body. Its `body.rs:309-328`
  polls the retained deadline, and `:400-418` installs that wrapper. A collector
  does not need to reset or replace the request deadline. New cancellation and
  slow-body regressions remain required; source inspection is not runtime proof.
- Oversize bodies should use the existing `upstream_body_too_large` provider
  error, not be mislabeled an expired session or silently truncated into success.
- Count streamed bytes after any active content decoder. Do not infer gzip is
  enabled from rquest defaults: charset is default, gzip/brotli are separate
  features. Confirm the integrated feature/client configuration before claiming
  a decompression regression exercises that runtime path.

Required red/green proof after the window returns: exact-limit and limit+1,
declared and unknown-length responses, start and finalize rejection, original
provider network error/timeout, malformed UTF-8, UTF-8 and UTF-16 BOMs, declared
non-UTF-8 charset, invalid charset fallback, and unchanged diagnostic previews.
Use synthetic responses for decoder comparisons and bounded loopback fixtures
for network/cancellation behavior; no production provider traffic is needed.

## 2. Shared immutable upload bytes avoid a larger LazyCell rewrite

- `gemini_canvas_upload_http.rs:65-71` clones fallback bytes before awaiting the
  encoder. On successful reencode that allocation is overwritten unused.
- `:74-78` borrows effective bytes in LazyCell and warms diagnostics before the
  start request if tracing is present. The finalize `.body(...clone())` at `:208`
  currently copies the complete Vec, while later snapshots still borrow it.
- The scout correctly rejected blindly moving the Vec into the request, but
  overstated the need for an owned fallback solely because an await exists:
  borrowed upload data can span await. Ownership is needed at the request-body
  boundary, not as a general rule for every borrowed async input.
- Main found a smaller candidate than eagerly materializing/removing LazyCell:
  select encoder output first, copying fallback only on None, then retain
  `bytes::Bytes` and clone its shared handle into rquest. `bytes = "1"` already
  exists in Cargo.toml and resolves to 1.11.1. Its `src/bytes.rs:960-977` transfers
  a Vec allocation; `:687-691` clones through the shared representation. rquest
  `src/client/body.rs:203-215` consumes Bytes or Vec as reusable bodies.

Decision: verify the shared-Bytes candidate before undertaking a larger lifetime
rewrite. It can preserve existing lazy/raw-capture timing and all byte-length
consumers while removing the finalize payload copy. It may allocate shared-owner
metadata; no zero-allocation or measured latency claim is made.

Required proof: successful encoder output is selected without fallback copying;
fallback source remains unchanged; cloned request/debug handles share payload
storage; actual finalize wire bytes/hash/length remain identical; trace-off never
forces debug; trace-on/raw capture still occurs before a start network failure.
Existing 10 loopback scenarios cover wire/error branches but not successful
browser reencode selection or allocation identity by themselves.

## 3. Encoder limits are not complete upload-ingress limits

`src/protocol/gemini_canvas.rs:603-619` allocates decoded base64 and normalizes
the image before the browser encoder is called. Normalization fully decodes at
`:647`, before downscaling. Encoder 32 MiB source/8 MiB output limits do not bound
those earlier allocations or source image pixel dimensions.

The normalizer's 127600-byte constant is a target, not a hard acceptance limit:
`:741-743` explicitly returns `best_over_limit` when no under-limit candidate
exists. The HTTP owner itself has no fallback-byte cap. Do not convert these
facts into an assertion of unbounded external ingress without tracing HTTP/body
guards, but do not label the whole upload pipeline bounded from encoder tests.
Ingress/pixel limits need their own boundary and compatibility proof; they must
not be hidden inside the behavior-preserving ownership extraction.

## Evidence snapshot and next gate

| Current worktree path | SHA256 |
| --- | --- |
| `src/upstream/gemini_canvas_upload_http.rs` | `b9d3d32749bf937986fae5c5f937ca59bdc2cd4516825a459e2c1ac7a3d33c58` |
| `src/upstream/gemini_canvas_upload_http_tests.rs` | `42dd090a17f8af608702a9ed2f4e68af73eb503382f27c8668f30017ef967ad6` |
| `src/protocol/upstream_body.rs` | `7388d70eedb57247279583cce1a9294d2dda92cc5791295f010efdfdf4d05ad2` |
| `src/protocol/gemini_canvas.rs` | `44d1dea4b30c0cd2d4dbdbe38c91aaf9a83c3507904f6ba10041cd94cb397eac` |
| `Cargo.toml` | `eba94e8164f9f07b38b46ccd62719dc97eae2866f0b78bf177047118b5f63e02` |
| `Cargo.lock` | `54efc7e07637256f9b8c3d340a92d46885ab7156c1a1ce11c1ef9f895b5b035a` |

After explicit window return, refresh these definitions and the coordination
scope. Implement and prove shared-byte ownership first, then bounded response
decoding in a separate write batch. Preserve full S06/S07-S21 scope, diagnostic
cost/ingress follow-ups and product gates. The last 580-pass structural gate is
historical evidence for the unchanged owner, not validation of these candidates.

## Follow-up after the coordinator's explicit window return

The window returned at2026-09-08 01:52:34 UTC. S06 then implemented shared Bytes
selection with focused ownership tests; see the main plan for current gate state.
The earlier table records the audit baseline, not the newly edited upload owner.
The subsequent charset-preserving bounded-reply implementation is recorded below.

Further source inspection during the freeze refined ingress/decoder claims:
real global/route HTTP byte limits exist; Axum's separate2MiB default may prevent
the configured50MiB limits from being fully usable. image0.25.10 also already
has a512MiB default allocation safeguard and reserves decoded output before
allocation, but that is non-strict and not a full application-memory bound.
Normalization is synchronous in the async image preparation path before browser
encoder admission. Do not bypass the existing limits or replace full decode with
header-only validation without focused rejection/compatibility/lifecycle proofs.
Exact local paths and findings are retained in the ignored target evidence file
`target/release-evidence/20260908-parallel-refactor-002354/S06-INGRESS-DECODE-AUDIT.md`.

## Bounded-reply implementation checkpoint

The real oversized-start regression first failed with error codeNone rather than
upstream_body_too_large. The new reader in src/protocol/upstream_body.rs now reuses
the64MiB collector and rquest's decoder with only Content-Type retained. Existing
UTF8 readers/collector code remain exact; no manifest/dependency change occurred.
The start/finalize upload seams preserve under-limit charset/BOM/error behavior.

Final body12/12, upload12/12 (14 trace-off/on wire cases), Gemini583 passed with3
ignored probes, and all-target exit0 pass. New tests exercise charset/BOM inputs,
malformed headers, unknown-length rejection/drop, error classification,
cancellation and retained real body deadlines. This bounds accumulated bytes,
not all transient memory or decoded String expansion. Full decoder/diagnostic
CPU/IO and ingress/concurrency work remains open; this is not full S06 completion.
The next lane E source/docs freeze is governed by the final shared handoff receipt.
