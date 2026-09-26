# Media response diagnostic hardening

Accepted on 2026-09-12. Worker failure-message overrides and Suno malformed-JSON
previews now use bounded diagnostic admission and the existing provider sanitizer.
This closes the message-publication gaps identified by the preceding response
extraction; it does not complete the overall Gateway plan.

## Failure and correction

The three workers first classified the original body, then replaced the sanitized
message with raw `error.message`. Suno copied a 200-character raw body preview into
its JSON error. Truncating a quoted, space-containing credential before sanitizing
can leave part of its value outside the redaction match.

The unchanged new contract reproduced six failures and four preservation passes
before production edits. Each worker now checks the override's byte length before
regex processing: up to 16 KiB is sanitized; larger values use a fixed omission
marker. The sanitizer removes controls and limits messages to 512 characters.
Suno applies the same admission policy, sanitizes the full admitted diagnostic
before truncating its preview, then sanitizes the composed error.

Classification still reads the original body. Nested status/code precedence,
selected provider identity, error kinds, retryability and body-derived retry delay
are unchanged. Empty explicit worker messages still override body messages; absent
messages retain the classified body message. Valid Suno JSON, including large
successful payloads and secret-like payload fields, is returned unchanged.

Only four production functions change. Their owners are 84/96/109/108 effective
lines; the new unit contract is 232 and its module wiring file is 157. All six
scoped files are below 500. Exact comparison preserves 32 function signatures,
28 other function bodies and 71 neighboring files. All 122 original response
tests remain byte-identical. No dependency, baseline or exception changes.

## Verification

- Identical regression source: 4/10 before, 10/10 after, with six defects reproduced.
- Original LumaLabs/Suno/Udio response tests: 30/30, 52/52 and 40/40.
- Fresh offline, locked default-feature all-targets compilation: passed.
- Scoped official formatter, checker tests 19/19, ratchet and both Git checks: passed.
- Signature/body/neighbor preservation, UTF-8 without BOM and whitespace: passed.

The paired contract and original library tests use `--no-default-features` with
`line-lumalabs-web-reverse-api`, `line-suno-web-reverse-api` and
`line-udio-web-reverse-api`. Baseline and final contract commands are identical,
with `--offline --locked --lib upstream::media_response_diagnostic_tests` and one
test thread. The default-feature compilation is a separate fresh gate.

Global formatting reports only the unchanged S06 runtime-mirror source/test files.
Strict scans 1,479 files: 31 hard, 44 mandatory and 40 soft; 75 remain above 700.
Structural clearance remains 70/145 (48.3%). All native gates are terminal.

Immutable evidence:
`target/effective-line-evidence/20260912-media-response-diagnostics/scope.json`.
Capture time: 2026-09-11 22:41:32 UTC. The regression SHA-256 is
`53e0b291b5b86588e0e4770d7dce11f0657db3cfb4bb1c951ab15a26d6952ecc`.
The preceding protocol, constructor and response-extraction snapshots remain
unchanged. Do not regenerate their old body comparisons after this intentional fix.

The pre-documentation snapshot records Gateway HEAD
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`, with 1,599 status entries:
161 modified, one unstaged deletion, two staged deletions and 1,435 untracked.
Neuro HEAD remains `bf818f0324024634bc890585efb78cc8e603d11a`, with 192 entries:
10 modified and 182 untracked. Inherited changes and staged deletions are retained.

## Remaining boundaries

The guard bounds additional diagnostic processing after worker/HTTP input has
already been read. Original process-output allocation, raw-body classification
and successful JSON parsing remain unchanged and unbounded by this guard.
No provider, browser, packaged runtime or release was exercised. S06 retains its
implementation and original cursor; GWP-20260908-06 still requires explicit
source/docs freeze and shared release-build transfer. No new release was built.
