# Folder-sync database bounds

Verified on 2026-09-17. Folder import now reads at most 4,096 provider-account
classification rows and 100,000 credential bookkeeping rows. Each SQL query requests
one sentinel row and returns the stable
`provider_credential_folder_sync_database_limit` error when the configured bound is
exceeded. Both failures occur before filesystem mutation or deletion reconciliation.

The account projection contains only classification metadata and an inline base URL.
Canonical account mutations derive and persist protocol family/profile from the full
input payload independently of its inline or object storage mode. Full account payloads
are fetched only after a file selects that account and the source hash requires
normalization; one run caches each hydrated account by ID. Same-hash material and
unselected object-backed accounts do not read full payloads. A selected account with
missing payload fails closed.

Real guarded PostgreSQL/Redis database tests pass 12/12. They exercise both exact
bounds and both overflow paths, prove overflow preserves a deletion-eligible row and
its four Redis keys, and retain create/update/skip/export, duplicate-path last-wins,
missing-payload and public lookup contracts. Guarded import/deletion overlap passes
6/6; filesystem deletion passes 9/9; the folder-sync library passes 116/116 with 12
ignored. All-targets, scoped formatting, checker 19/19, ratchet and both Git diff
checks pass. Global formatting still reports only the two reserved S06 runtime-mirror
files.

Nine scoped owners measure 44/41/268/413/147/44/396/107/12 effective lines. Strict
inventory is 2,180 files with 11 hard, 20 mandatory and 39 soft entries; its expected
exit remains 1. Two fixture cleanup wrappers hit PowerShell/template compatibility
errors after their tests passed; all four uniquely labelled containers and four
anonymous volumes were subsequently identity-checked and removed.

Selected remote object-storage success and legacy rows created outside canonical
account mutations are not certified here. Aggregate parsed-memory/path retention,
native deadlines/full drain/status ordering, blocking fairness, file replacement and
link/cross-process races, Unix/macOS behavior and complete release acceptance remain.
S06 stays reserved and the overall goal remains active.

Evidence: `target/effective-line-evidence/20260917-folder-sync-database-bounds/`.
Scope SHA-256: `5cab763a22ffbf1d460d101b5a6f0ba78fc3404a0582d1dd55992f789af5641b`.
[Report](../../status/2026-09-17-folder-sync-database-bounds.md).
