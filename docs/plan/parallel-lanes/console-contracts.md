# Console contract test decomposition

## Resumed checkpoint: 2026-09-11

Owner: the coordinator of the resumed `01a07dea-2db1-7c11-93df-3755175a8222`
conversation. Current state: `structural_green`; final evidence is recorded below.

Exclusive scope: the existing Console document, persistence, management and transaction test
suites, their domain-specific test support, and this lane record. Production
Rust/Gemini ownership is unchanged. Shared Cargo checks are serialized and the
current process census found no existing Cargo/rustc job before validation.

The inherited management, sensitive-input, credential-probe and transaction
splits are present. Their last historical Cargo invocation had no recorded
terminal result. A fresh baseline now passes all 94 tests across six targets:
document 58, management 12, sensitive input 2, credential probe 6, transaction 4
and transaction runtime 12. The baseline retained three production warnings and
three unused imports in the new sensitive-input test suite.

The captured document suite had 2,085 effective lines. Its 58 test bodies remain
in the existing Cargo integration target, with private modules for
canonical revisions, redaction, patch operations, identity, JSON pointers,
resource bounds and URL validation. Common YAML fixtures stay with the parent;
the environment guard moves only with the URL tests that use it.

Before editing, source copies and the official size report were saved under
`target/effective-line-evidence/20260911-resume-console/`. The report scans 1,298
files: 36 above 1,500, 63 in 701-1,500, and 40 in 501-700. The 99 files above 700
remain open debt. The baseline and policy are unchanged.

Proof: preserve every test function body, then rerun the same six Cargo targets,
the directly dependent compile check, Rust formatter, checker tests, ratchet,
strict inventory and Git whitespace/encoding checks. Any additional persistence
suite work requires its own pre-change baseline.

The latest existing immutable release is
`20260908-producer-mailbox-s06-123700`. No new release or completion of the full
S00-S21 plan is claimed by this test-only checkpoint.

## Document checkpoint and persistence continuation: 2026-09-11

The interrupted document extraction is now complete: the 2,085-effective-line
suite has a 41-line fixture/registration entry and nine private owners of
117-313 effective lines. All 58 test names, attributes and complete bodies match
the captured source after the same Rust formatter; YAML/diagnostic fixtures and
the scoped environment guard also match. The inherited 36 management, grant,
probe and transaction tests retain their complete formatted bodies.

Fresh six-target Cargo validation passes 94/94 with no test-source warnings;
`cargo check --offline --locked --all-targets` exits 0. Three inherited production
warnings remain. Evidence: `target/effective-line-evidence/20260911-console-completion/`.
The first post-split compile identified a missing `json!` import in the pointer
suite; the import was restored before the successful final run.

The same coordinator now owns the remaining 1,060-effective-line
`tests/console_persistence_contract.rs` and its private test modules. A separate
pre-change snapshot and passing 40-test baseline were recorded under
`target/effective-line-evidence/20260911-console-persistence/`. Production
persistence code and S06 Rust/Gemini ownership remain outside this test-only
write scope. Final lane formatter, size and whitespace evidence includes both
test extractions.

## Final structural acceptance: 2026-09-11

Persistence is now a 52-effective-line fixture/registration entry, with YAML
snapshots (214), journal (256), recovery (275), atomic-backend (198) and native
path/lock (108) owners. All 40 complete tests, their Windows cfg attributes and
shared/moved fixture blocks are unchanged after applying the same formatter to
the baseline. The final Cargo run reports 40 passed and the fresh all-targets
check exits 0. `--nocapture` also exposes an inherited environment limitation:
the file-symlink contract returns early on Windows error 1314. Its rejection
assertion is not counted as executed platform proof.

Together with the six previously validated targets, Cargo reports 134 passed,
zero failures. No test-source warning remains. All 23 source files in this lane
are at most 442 effective lines, valid UTF-8 without BOM, and require no new soft
exception. Checker 19/19, scoped Rust formatting, ratchet and Gateway/Neuro diff
checks pass. The common development-standard contract passes. The final strict
inventory has 1,312 files, 35 hard + 62 mandatory = 97 above 700, and 40 soft;
two remaining debt entries were cleared by this continuation.

Global `cargo fmt --all -- --check` remains red only for the S06-owned
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. No S06 source was changed.
The default review agent failed to start because its model was unsupported;
no independent review is claimed. Coordinator source review, exact extraction
proofs and fresh compiler/test gates provide this structural acceptance.

Full evidence, commands, per-file counts and release limitations are in
[the acceptance report](../../status/2026-09-11-console-contracts-completion.md).
No new immutable release is included; the next integrated build still requires
the explicit GWP-20260908-06 source/docs freeze and Cargo transfer receipt.
