# Folder synchronization deletion containment

S09 static-path containment is accepted on 2026-09-16. Public API signature and
re-export remain. Raw relative-path checks precede normalization; trusted root
canonicalization, descendant symlink/reparse rejection and canonical Path::starts_with
prevent the reproduced static escapes. In-root links are also rejected. Ordinary
separators, repeated separators and empty/missing/disabled noops remain supported.
NotFound during unlink returns false; other I/O and unsafe-path errors propagate.

Baseline filesystem tests: 2 passed / 7 failed; unchanged candidate 9/9. Original
folder_sync remains paired 45/45. All-targets, scoped rustfmt, exact source proof,
checker 19/19, ratchet and both Git checks pass. No executed-fixture correction.
Windows junctions ran; Unix leaf-link coverage awaits a Unix run. Fixture residues: 0.

Deletion owner 219 -> 209; paths/tests/fixture 118/167/84. Parent stays byte-identical
at 5975. Strict 2103/12/20/39; 32 above 700; clearance 113/145 (77.9%). Union 1808,
unchanged neighbors 1804, unchanged assets 22. No dependencies/policies/baselines,
profiles, services, commits or releases changed.

This is not race-resistant deletion: concurrent directory replacement remains open.
Unsafe legacy source paths can block management deletion; account deletion retains
its existing incremental transaction behavior. Scanning/import/export, DB duplicate
hit semantics, watcher ownership, root migration and full optimization remain open.
S06 and final build stay reserved under GWP-20260912-01.

Evidence: target/effective-line-evidence/20260915-folder-sync-deletion-containment/.
Scope SHA-256: 89bf5e4fa0e267c63c5b39349d9d7be9ecef88de61bc16ab19aa9a63e3990de7.
[Report](../../status/2026-09-16-folder-sync-deletion-containment.md).
