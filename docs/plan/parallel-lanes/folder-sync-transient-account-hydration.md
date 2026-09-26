# Folder-sync transient account hydration bound

Verified on 2026-09-18. Folder-sync account classification still uses the bounded
metadata projection and only hydrates an account after path selection and source-hash
comparison. The selected-account lookup now performs a PostgreSQL byte preflight with
`octet_length(payload_inline::text)` before selecting the full `GatewayProviderAccountRow`.

The folder-sync-only preflight uses the existing 32 MiB standard object-storage limit,
locks the account row across the size check and full-row read, and returns the stable
`provider_credential_folder_sync_account_payload_too_large` error for an oversized
inline payload. Public `get_provider_account` behavior is unchanged. Object-backed
payloads continue through the existing bounded object-storage reader, and the existing
retained-byte LRU still prevents unusually large hydrated views from being cached.

The new guarded regression proves an oversized selected inline account fails before
credential mutation or deletion and preserves its four runtime keys. Existing same-hash
and unselected-object paths remain before hydration. The complete folder-sync unit group
passes 126/126 with 12 external-service tests ignored; all-targets, scoped formatting,
checker 19/19 and the ratchet pass.

The guarded PostgreSQL/Redis suite was not executable in the current shell because
`GATEWAY_DELETE_TEST_RUN_ID`, `GATEWAY_DELETE_TEST_DATABASE_URL` and
`GATEWAY_DELETE_TEST_REDIS_URL` are absent. The new integration test compiles with
`--no-run`; no database result is claimed for this checkpoint.

The changed owners remain below the 500-line ratchet boundary. Native callback
allocation, remote object-storage success/read proof, parsed account/value peak bounds,
deadlines/full drain/status ordering, fairness/races and complete release acceptance
remain open. Aggregate normalized path retention is covered separately by the
2026-09-18 checkpoint. S06 stays reserved and the overall goal remains active.

Evidence: `target/effective-line-evidence/20260918-folder-sync-transient-account-hydration/`.
[Report](../../status/2026-09-18-folder-sync-transient-account-hydration.md).
