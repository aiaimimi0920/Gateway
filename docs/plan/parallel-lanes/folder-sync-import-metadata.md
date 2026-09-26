# Folder-sync import metadata

Verified on 2026-09-16. A private nine-field SQL projection removes unused old
credential payload hydration from import bookkeeping. It preserves all rows,
created_at ASC order, duplicate-path last-wins and deletion eligibility. Account
and source-path lookup maps borrow snapshot entries. Public lookup/export and
all mutation/storage behavior stay unchanged; no fake payload view is introduced.

Real PostgreSQL/Redis baseline: 3 passed / 4 failed; frozen candidate 7/7. Cases
cover create/update/skip/export, exact hashes/status, missing-payload independence
and repair, duplicate selection, deletion eligibility/cache cleanup, and retained
public lookup/export failure. Original library 116/116 in both phases, six ignored
and all 122 identities/results exact. Deletion 9/9, all-targets/fmt/checker 19/19/
ratchet/both Git checks pass. Warning sets and all database test bytes are frozen.

Nine owners: 420/103/31/133/209/289/214/12/107 effective lines. Strict 2177/11/20/39;
all 31 above700 files unchanged, clearance 114/145 (78.6%). Union 1882, neighbors
1873, assets 22. Four owned containers/volumes cleaned; 45 prior containers and
all persistent services/releases preserved. Guard and proof-order corrections
are recorded; no post-freeze source change or successful native-gate replay.

Metadata/query bodies, full import and deletion source, and original deletion test
bodies have exact preservation proof. Previous payload_content_type review wording
is corrected in new evidence: real INSERT/UPDATE require that schema column.

Next: explicit-delete/delete-missing overlap, DB row/account/parsed-memory/path
bounds, native deadlines/full drain/status, races and full acceptance. HTTP and
remote object-storage success remain unproved. S06 stays reserved; goal active.

Evidence: target/effective-line-evidence/20260916-folder-sync-import-metadata/.
Scope SHA-256: 1d4c45f68730f1c90ca5ebb7ecc5f0a949b13937d0fdac76bd3630a2ba0257d6.
[Report](../../status/2026-09-16-folder-sync-import-metadata.md).
