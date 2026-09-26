# S06 diagnostic schema extraction

## Ownership and recovery

Coordinator explicitly returned GWP04 at05:44UTC after publishing immutable
20260908-udio-worker-s06-051500. S06 observed the return and resumed its source,
documentation and Cargo scope. The previous coordination blocker is resolved;
no new source freeze or build transfer is declared by this checkpoint.

During the freeze S06 polled live build PID34252 and package-gates PID47680 through
terminal state, then waited for the explicit return rather than inferring write
permission from their absence. Coordinator evidence records819 package files,
matching source/provenance fingerprint and two isolated runtime passes. These are
coordinator release gates, not a claim of full optimization or visual acceptance.

## Exact change

Extracted12 existing image-edit diagnostic payload and redacted stream-contract
builders into src/upstream/gemini_canvas_image_edit_snapshots.rs. This establishes
the schema owner for the remaining diagnostic IO work without changing payload
construction, clock sampling, ordering, nullability or redaction behavior.

The complete concatenated12 function definitions compare byte-identical with the
pre-edit captured source after line-ending normalization. Only imports and parent
module/re-export wiring changed. All original73 parent tests remain in place.

The parent re-exports are currently required by UpstreamClient, upload HTTP and
existing contract tests, not a dead compatibility layer. The new child imports
only protocol types, header access and response sanitization; it never imports
its parent, so no reverse dependency or cycle was introduced.

| Owner | Before effective | After effective / physical |
| --- | --- | --- |
| gemini_canvas_image_edit_local_helpers.rs | 3111 | 2913 / 3139 |
| gemini_canvas_image_edit_snapshots.rs | new | 221 / 234 |

The original helper remains hard-limit debt and S06 remains incomplete. This is
one coherent extraction, not clearance of the entire parent or a decrease in the
count of files above700. No baseline, exception, dependency or provider change.

## Verification

- Baseline owner tests73/73 passed; post-extraction same73/73 passed.
- Final `cargo test --offline --locked --lib gemini_canvas -- --quiet`:588 passed,
  0 failed,3 ignored; compile6m09s/tests11.56s.
- Final `cargo check --offline --locked --all-targets`: exit0,2m27s.
- Formatter, checker tests19/19, ratchet and diff checks passed.
- Independent read-only review confirmed active callers, one-way dependencies,
  necessary re-exports, existing header-presence masking and clock/schema behavior.
- Removed the newly unused parent json import after the first compile. The final
  warnings are the pre-existing HashMap/compact_response_preview/dead helper set.

Pure builders still carry raw response values for the existing downstream debug
sanitizer. This extraction neither bypasses that sanitizer nor proves new security
coverage of arbitrary fields. No new worker, allocation algorithm or IO was added.

Next: finish the parent diagnostic/IO boundaries and mandatory normalization
admission/pixel safety. The normalization audit remains in target and the full
S06/S07-S21 objective remains open. The immutable051500 package predates this edit.
