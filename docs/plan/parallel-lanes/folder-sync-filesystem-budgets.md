# Folder synchronization filesystem resource budgets

S09 resource budgets are verified on 2026-09-16: encoded material 32 MiB; source
path 512 Unicode characters; descendant depth 32; total entries 100,000; JSON files
10,000. Byte/path ceilings align with existing storage capacity; scan ceilings are
explicit new operational policy. Every production caller uses these defaults.

Discovery counts directories/non-JSON entries and fails before reconciliation.
Reads check both metadata and actual bytes, consuming at most one sentinel byte
beyond the limit. Bounded pretty serialization checks each fragment before buffer
growth. Oversized output cannot create directories, and an oversized existing file
cannot be replaced. Capacity errors are immediate; ordinary retry/containment/
counter/metadata behavior remains. No new config, dependency or unsafe code.

Staged baseline 84 passed / 13 failed / 6 ignored. Frozen candidate 97/97, six
ignored; all 90 previous identities/results exact. All 13 new tests pass unchanged.
Deletion integration 9/9, all-targets/fmt/checker 19/19/ratchet/Git pass. No candidate
correction, native success replay or residual fixture roots.

Scoped owners root/filesystem/filesystem-export/export/material/paths/limits/tests/
budget-tests: 126/138/82/61/114/167/42/222/192. All 56 subtree Rust files <=500,
maximum 460. Strict 2157/11/20/39; all 31 above-700 hashes/counts unchanged;
clearance 114/145 (78.6%). Union 1862, neighbors 1853, assets 22.

Global run admission, DB hydration, parsed JSON memory, blocking I/O, cancellation,
deadlines, duration/shutdown/status, races/hard links, database/provider and full
runtime/UI/Docker/release acceptance remain open. S06 stays reserved; goal active.

Evidence: target/effective-line-evidence/20260916-folder-sync-filesystem-budgets/.
Scope SHA-256: 01ebbd92ccf6919cde077a370d38072fb33f7f3dcffe6c173cc75809dd8cf146.
[Report](../../status/2026-09-16-folder-sync-filesystem-budgets.md).
