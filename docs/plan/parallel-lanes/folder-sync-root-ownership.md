# Folder synchronization root ownership completion

S09 root migration is verified on 2026-09-16. Root 3066 -> 124 effective lines;
24 production functions move, three coordinator/formatting functions remain.
Import/export/filesystem/accounts/layout owners are 132/98/100/277/206. The two
moved types preserve their fields and narrow parent-subtree access. Public and
deletion/watch/status paths remain. Query/write/delete/error/retry/ranking/path
behavior is unchanged.

All 36 original tests and two fixtures move into seven contract suites plus
tests.rs: 75/211/357/183/307/460/275/347. Every name/attribute/assertion/value remains.
Exact 36-entry module-path mapping preserves all 78 candidate results: fresh
library 72/72, six ignored; 42 identities/results remain unchanged. Before gate
reuses the hash-revalidated accepted normalization snapshot. No native success replay.

All-targets, scoped fmt, whole-source proof, checker 19/19, ratchet and both Git
checks pass. Strict 2151/11/20/39; 31 above 700, all remaining hashes/counts unchanged.
Clearance is 114/145 (78.6%). Source union 1856, unchanged neighbors 1842, assets 22.
All 50 Rust files in the folder-sync subtree are <=500; maximum 460. Root ownership
migration is accepted; combined root/new-owner size adds 86 wiring/formatter lines.

Three accepted independent reviews complete. A wrong-baseline I/O review was
replaced with an exact absolute-path comparison; factual count/visibility and
Node REPL observation issues are documented. No candidate source/test correction.
Exact Git audit permits only 13 new source paths and two new documents.

Filesystem blocking/bounds/symlinks, path containment, deletion TOCTOU, status/
shutdown/deadlines, database/provider coverage and full runtime/UI/Docker/release
acceptance remain. No performance or fresh runtime claim. S06 source/cursor/final
build stays reserved under GWP-20260912-01. The overall goal remains active.

Evidence: target/effective-line-evidence/20260916-folder-sync-root-ownership/.
Scope SHA-256: 3512e8b7f83d8d5145ceff66489454e1e8404b316449028bc5651c2c3598fc01.
[Report](../../status/2026-09-16-folder-sync-root-ownership.md).
