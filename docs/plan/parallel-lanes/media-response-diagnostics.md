# Media response diagnostic hardening

Owner: parallel coordinator. State: `verified`.
Started: 2026-09-12.

Scope is the three response-local `worker_results.rs` owners, Suno's
`http_responses.rs`, a new feature-gated
`src/upstream/media_response_diagnostic_tests.rs` and its test-only module wire.
The complete four production owners and the upstream module have been read.

The workers classify the original upstream body, then overwrite the sanitized
message with raw `error.message`. Suno truncates malformed JSON before redacting
its preview. Regressions must exercise real JSON admission and HTTP publication,
classification/status/code/provider precedence, retry delay, controls, multibyte
limits, exact 16 KiB admission and oversized omission. Successful JSON payloads,
empty worker overrides and all 122 existing response tests remain unchanged.

Keep classification on the original body. Reuse the existing provider sanitizer
only after the separate diagnostic passes the 16 KiB admission check; retain the
512-character publication ceiling. Sanitize Suno's diagnostic before preview
truncation and sanitize the final composed error. Do not add shared utility layers
or change public signatures. Preserve the accepted extraction evidence.

Evidence: `target/effective-line-evidence/20260912-media-response-diagnostics/`.
Establish failing regressions before production edits, then run the identical
suite, original response tests, default-feature all-targets, scoped formatting,
checker/ratchet/strict, preservation, encoding and independent Git checks.

This does not bound original process-output allocation, raw-body classification
or successful JSON parsing. S06 retains its implementation/cursor and the pending
GWP-20260908-06 source/docs freeze and shared release-build transfer. No release
build is authorized by these scoped gates.

The unchanged 10-test contract compiled with only the three media line features
enabled and reproduced six failures plus four preservation passes on the original
production source. The four fixes are now in place. Full body/signature proof,
71 unchanged neighbors, immutable regression/extraction hashes and scoped
formatting pass. The identical paired contract now passes 10/10. All 122 original
response tests pass with the same three media features; fresh default-feature
all-targets compilation passes. Checker 19/19, ratchet, encoding and both Git checks
pass. Global formatting reports only the two unchanged S06 runtime-mirror files.

The four production owners are 84/96/109/108 effective lines; the new test owner
is 232 and module wiring is 157. All six scoped files are below 500. Thirty-two
function signatures and 28 other function bodies are unchanged. Strict scans
1,479 files: 31 hard, 44 mandatory and 40 soft; 75 remain above 700. Clearance stays
at 70/145 (48.3%). All native gates are terminal and no release was built.

Accepted evidence: `scope.json`, captured at 2026-09-11 22:41:32 UTC, and
[the acceptance report](../../status/2026-09-12-media-response-diagnostics.md).
