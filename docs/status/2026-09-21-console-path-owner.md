# Console path owner

The path-resolution and release-payload containment contract moved from
`src/console/config.rs` into `src/console/paths.rs`. The moved owner includes
lexical and physical path views, Windows reparse/ancestor handling, Unix
symlink resolution, descendant checks, path errors and the public
`validate_state_directory` contract. Configuration types, environment parsing,
origin normalization and Redis namespace validation remain in `config.rs`.

All error codes, messages, path comparisons and platform branches were moved
without logic edits. `config.rs` re-exports `validate_state_directory`, so
existing callers retain their import path. The effective-line counts are:

- `src/console/mod.rs`: 37.
- `src/console/config.rs`: 187.
- `src/console/paths.rs`: 246.
- `src/console/persistence_lock_contract.rs`: 118.

Each owner is below 500 effective lines. A targeted rustfmt pass was applied only
to the four Console files; an accidental formatter rewrite of
`src/console/gemini_auth_sessions.rs` was detected by the ratchet and restored
exactly before completion. No unrelated source change remains from that pass.

## Verification

- Effective-line lexer tests: 19/19 passed.
- Ratchet: passed after restoring the unrelated formatter-only drift; 2259 files,
  10 above 1500, 18 between 701 and 1500, 24 between 501 and 700.
- Gateway and Neuro `git diff --check`: passed.
- Targeted Rust formatter accepts the extracted files under edition 2024. The
  repository-wide formatter remains red only on pre-existing unrelated files.
- Cargo compile/test was not completed: shared Rust compilation exceeded the
  available execution windows in the prior and current Console batches without
  a diagnostic tied to these files. No compile success is claimed.

The path owner remains behavior-only structural work. Existing path trust,
symlink, TOCTOU and filesystem race assumptions were not hardened. Full strict
closure, integrated runtime validation, S06 transfer and release publication
remain open. Dirty/staged state and all sibling repositories were preserved.
