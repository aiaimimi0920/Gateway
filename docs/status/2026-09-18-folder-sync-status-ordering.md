# Folder-sync status timestamp ordering, 2026-09-18

This checkpoint hardens shared folder-sync status against out-of-order admitted
runs without changing the public status schema.

## Ordering rules

- RFC3339 timestamp fields accept a candidate only when it is newer than the
  stored valid value. A valid candidate can repair a malformed stored value;
  a malformed candidate never replaces a valid value.
- Run phase timestamps (last_run_at, last_import_at, and last_export_at)
  advance independently. A late run cannot move a phase timestamp backward.
- A late run cannot overwrite aggregate counters or last_error owned by a
  newer run. Its explicit-delete audit events still merge into the retained
  history.
- Explicit-delete history is kept newest-first by event occurrence time, while
  last_explicit_delete_* remains owned by the newest event.

## Verification

- Folder-sync library: 134 passed, 12 ignored.
- cargo check --offline --locked --all-targets: passed with existing Gemini
  warnings.
- npm run test:effective-lines --prefix scripts: 19 passed.
- npm run check:effective-lines --prefix scripts: ratchet passed; inventory
  remains 2,183 files with >1500=11, 701-1500=20, and 501-700=39.
- git diff --check: passed.

The guarded Redis status suite was not available in this shell because its
dedicated fixture environment is absent. Therefore this checkpoint proves the
ordering logic through source review and unit coverage, but does not claim a
fresh cross-process Redis run. Same-field enable/watch-running precedence,
remote S3, PostgreSQL/Redis synchronization, filesystem races, Unix/macOS
behavior, and full provider/UI/Docker/release acceptance remain open.

