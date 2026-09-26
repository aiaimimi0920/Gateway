# Folder synchronization filesystem containment

S09 static containment is verified on 2026-09-16. Import discovery/read, export
write, stale cleanup and management deletion use one shared path owner. Validate
raw source_path before normalization; reject invalid components, descendant links,
Windows reparse points and special material. Configured root links remain trusted.
Incomplete discovery fails before reconciliation; actual I/O boundaries recheck.
Traversal retains depth-first read_dir order with an explicit iterator stack.

Staged real-filesystem baseline: 75 passed / 9 failed / 6 ignored. Frozen candidate:
84/84, six ignored; all 78 previous identities/results exact and all 12 new Windows
tests pass. Existing deletion integration 9/9; all-targets, scoped fmt, checker
19/19, ratchet and staged/unstaged Git checks pass. No candidate correction or
successful native replay. Windows junctions ran; Unix leaf-link execution remains.

Owners root/filesystem/filesystem-export/export/import/layout/paths/deletion/tests/
filesystem-tests/fixture: 125/118/56/65/133/186/159/209/76/221/68 effective lines.
All 53 subtree Rust files remain <=500, maximum 460. Strict 2154/11/20/39; all 31
above-700 hashes/counts unchanged; clearance 114/145 (78.6%). Union 1859, unchanged
neighbors 1848, assets 22. The removed deletion/paths.rs is recorded in absent.

Static security and wiring reviews complete; final test/result proof uses exact
evidence paths. No real credentials/database fixtures or service/release writes.
Concurrent replacement, hard links, general bounds, blocking I/O, deadlines,
shutdown/status, database/provider and full runtime/UI/Docker/release acceptance
remain open. S06 source/cursor/final native build stays reserved. Overall goal active.

Evidence: target/effective-line-evidence/20260916-folder-sync-filesystem-containment/.
Scope SHA-256: 1fa27bf68cc39b6fcb786e879ef48373f9c48706e2b950598e042b3f7cbf848c.
[Report](../../status/2026-09-16-folder-sync-filesystem-containment.md).
