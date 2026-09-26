# Console configuration owner

## Structural change

The configuration contract was extracted from `src/console/mod.rs` into
`src/console/config.rs`. The console module now contains module declarations,
public re-exports, and its persistence-lock contract tests; configuration types,
environment parsing, path resolution, release-payload containment validation,
origin normalization and Redis namespace validation have one owner.

Effective lines changed from 592 in `src/console/mod.rs` to 151; the new
`src/console/config.rs` is 443 effective lines. The new file remains below the
500-line normal ceiling and retains all public names through `pub use config::*`.
No protocol, persistence ordering, path rule or error code was changed.

The extraction was generated from the exact pre-change ranges: configuration
constants/types/validation and their private helpers moved together, while the
existing persistence-lock tests stayed in `mod.rs`. No sibling repository, Cargo
manifest, baseline or exception file changed.

## Verification

Fresh Gateway checks:

- `npm run test:effective-lines --prefix scripts`: 19 tests passed.
- `npm run check:effective-lines --prefix scripts`: ratchet passed. The scan now
  reports 2257 files, 10 above 1500, 18 between 701 and 1500 and 24 between
  501 and 700; this clears one soft debt entry.
- `git diff --check` in Gateway: passed.
- `git diff --check` in Neuro: passed.

`cargo fmt --all -- --check` remains red only on three pre-existing unrelated
files (`src/provider_credential_folder_sync/paths.rs`,
`src/upstream/gemini_canvas_runtime_mirror.rs` and its test). None is in this
scope. A fresh `cargo check --locked --lib` was attempted twice but exceeded
the 60-second and 180-second execution windows after emitting only existing
warnings from `src/upstream/gemini_canvas_image_edit_local_helpers.rs`; no Rust
diagnostic referencing the console extraction was produced. Therefore this
checkpoint does not claim a completed Rust compile gate.

## Risk and continuation

The module move preserves existing path trust and release-root validation logic;
those semantics were not hardened in this structural batch. The public glob
re-export intentionally preserves current import paths. The next validation
window should rerun a targeted console Rust test or a cached lib check when the
shared build lane is available.

Full strict closure, integrated runtime acceptance and release publication remain
open. S06 Rust/Gemini and the final native build window stay reserved pending a
transfer receipt. Existing dirty/staged state was preserved; no commit, push,
live service or release/Gateway content was changed.
