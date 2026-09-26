# Console persistence test owner

The independent `persistence_lock_contract` test module was moved out of
`src/console/mod.rs` into `src/console/persistence_lock_contract.rs`. Test names,
fixtures, imports, assertions and cleanup bodies were preserved; only `super::`
paths became explicit `crate::console::` paths. Production console behavior was
not changed.

Effective lines are now 37 for `mod.rs`, 443 for `config.rs`, and 116 for the
new test owner. This removes the remaining test responsibility from the console
module without creating an oversized file.

Verification:

- Effective-line lexer tests: 19/19 passed.
- Effective-line ratchet: passed; 2258 scanned, 10 above 1500, 18 between 701
  and 1500, 24 between 501 and 700.
- Gateway and Neuro `git diff --check`: passed.
- `cargo fmt --all -- --check` still reports only the same three unrelated
  pre-existing files: provider credential folder paths and Gemini canvas mirror
  source/test.
- A focused `cargo test --locked console::persistence_lock_contract` was started
  but the shared Rust build exceeded the 120-second execution window without a
  diagnostic. This checkpoint therefore does not claim a completed Rust test.
  The previous configuration extraction has the same compile-window limitation.

No Cargo manifest, policy, baseline, exception, release or sibling project was
changed. Existing dirty/staged state was preserved. S06 and the final native
build window remain reserved; full strict closure, integrated runtime validation
and release publication remain open.
