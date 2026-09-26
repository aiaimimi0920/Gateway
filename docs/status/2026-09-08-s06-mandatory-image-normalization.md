# S06 mandatory image normalization admission

## Scope and behavior

Mandatory Gemini Canvas image-edit base64 decoding, image decoding, resize and
JPEG search now execute in an owned blocking worker rather than on the async
upstream caller. Two process-wide, nonqueued permits bound submitted work.
Admission occurs before copying upload base64 strings; saturation returns HTTP
503 / `image_edit_normalization_busy`, never a silently missing image. Empty
legacy input keeps its previous empty-result behavior without worker submission.

The worker owns only parsed uploads and its permit, not the complete request,
provider payload or prompt. Caller cancellation retains the permit until the
worker exits. Worker join failure produces a generic
`image_edit_normalization_worker_failed` server error; admitted input errors are
returned unchanged. Request timing still starts after normalization finishes.

The synchronous public protocol API remains available. JPEG normalization and
encoding bodies are exact against the pre-extraction captured source after
line-ending normalization; base64-before-MIME validation order is retained.

| File | Before effective | After effective / physical |
| --- | --- | --- |
| protocol/gemini_canvas.rs | 7944 | 7784 / 8343 |
| protocol/gemini_canvas_image_edit_uploads.rs | new | 175 / 191 |
| upstream/gemini_canvas_image_normalization.rs | new | 54 / 62 |
| upstream/gemini_canvas_image_normalization_tests.rs | new | 154 / 162 |
| upstream/gemini_canvas_followup_types.rs | 370 | 372 / 413 |
| upstream/gemini_canvas_followup_plan_tests.rs | 161 | 163 / 179 |
| upstream/client.rs | 15638 | 15638 / 16351 |

The small protocol extraction supplies the owned S06 worker boundary; it does
not complete S17 or the oversized protocol/client owners. No legacy file grew
above its existing hard-limit size.

## Verification

- Baseline synchronous extraction test: 1 passed.
- New normalization tests: 6 passed, 0 failed. They cover empty bypass, explicit
  overload before malformed parsing, parse-error permit release, unchanged input
  errors, real PNG-to-JPEG equivalence/source retention/order, off-thread work,
  sanitized panic error and cancellation-safe input/permit ownership.
- Independent read-only review found no admission, cancellation, parser or caller
  wiring defect. Its over-target JPEG concern is retained below as existing debt.
- Formatter, effective-line checker tests 19/19, ratchet and diff checks passed.
  All seven source files are valid UTF-8 without BOM.
- Full Gemini passed 594 tests, 0 failed, 3 ignored (6m21s compile / 11.98s
  tests); all-target compile passed in 2m33s. Compilation identified newly unused
  protocol imports: removed `gemini_business` and restricted `base64::Engine` to
  tests. Final post-adjustment all-target check passed in 2m27s; Gemini rerun
  passed 594 tests, 0 failed, 3 ignored (5m53s compile / 11.43s tests). Only the
  inherited HashMap/compact_response_preview/dead music helper warnings remain.

## Remaining limits and ownership

This is a concurrency/submission bound, not a heap, decoded-pixel or duration
bound. Image decoding still precedes resize; the existing image decoder policy
is unchanged. The 127600-byte JPEG target may still return the best over-target
candidate. Retained source bytes and downstream browser encoding are unchanged.
No ingress limit, HTTP image/music/video handler, live provider, Docker process
or release package was changed. The immutable 051500 package predates this work.

S06 and the full S07-S21 optimization objective remain open. Remaining work
includes diagnostic IO ownership, runtime-mirror adapter characterization and
mandatory decoder pixel/resource policy. GWP05 is pending explicit handoff;
All S06 Cargo handles are now terminal. GWP05 transfer will be recorded as the
last source/docs write in the shared handoff; that separate receipt controls the
freeze, not this report. No source/docs edits or Cargo work may resume after
that receipt until the coordinator explicitly returns the window.

Git census at this checkpoint (tracked / untracked, independent repositories):
Neuro 10/29, Gateway 141/231, Hook 70/211, Loom 89/39, Platform 41/12,
Talk 73/13, Tea 0/1. These include inherited/concurrent changes, not this batch's
change count; only Gateway was written by S06. Latest ratchet scans 1119 files:
40 hard, 75 mandatory, 38 soft. The 115 files above 700 remain unresolved; no
concurrent Node debt reduction is attributed to this batch.
