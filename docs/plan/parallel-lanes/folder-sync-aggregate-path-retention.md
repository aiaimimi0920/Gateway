# Folder-sync aggregate path retention bound

Verified on 2026-09-18. Folder import now gives one scan-level budget to the
normalized source-path keys shared by the database deletion snapshot, the observed
filesystem set and the lookup index. The 64 MiB cap charges each distinct string's
allocated capacity plus a fixed 384-byte key/index margin with saturating arithmetic.

Database metadata still applies its independent 32 MiB source-path byte preflight and
the filesystem still applies its 512-character path limit. The aggregate owner is
created after those checks and is used for both stored metadata paths and newly
observed files. A duplicate normalized path reuses its `Arc<str>` without another
charge. An admission failure returns the stable
`provider_credential_folder_sync_path_retention_limit` error before either deletion
phase can run.

`CredentialPathIndex` releases the metadata row's duplicate `source_path` allocation
after interning. Deletion eligibility, duplicate last-row selection, database order,
explicit-delete audit paths and observed-path identity continue to use the shared key.
No partial index is returned when the aggregate budget rejects a new path.

Focused tests cover duplicate accounting, fail-closed key admission, metadata-path
release and the existing last-duplicate-wins contract. The complete folder-sync unit
group passes 129/129 with 12 external-service tests ignored. The local provider-account
object-key write/read regression passes inside the 20-test object-storage group.
All-targets and the effective-line ratchet remain required gates; guarded
PostgreSQL/Redis execution and remote S3 success/read proof remain separate open
evidence.

Native callback allocation, operation deadlines, complete shutdown drain, status
ordering, blocking-pool fairness, filesystem replacement and hard-link/reparse races,
Unix/macOS behavior and full provider/UI/Docker/release acceptance remain open. S06
stays reserved and the overall optimization goal remains active.

Evidence: `target/effective-line-evidence/20260918-folder-sync-aggregate-path-retention/`.
[Report](../../status/2026-09-18-folder-sync-aggregate-path-retention.md).
