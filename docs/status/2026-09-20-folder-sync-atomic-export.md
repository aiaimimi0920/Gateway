# Folder-sync atomic export, 2026-09-20

Folder-sync export writes now use a uniquely named temporary file in the
destination directory and replace the final credential file with a same-
directory rename. The target and temporary path are revalidated through the
existing `SyncRoot` containment boundary before the effect. Failed writes and
failed replacements remove the temporary file when cleanup is still local.

The export regression writes a credential file, replaces its payload, verifies
the replacement bytes, and confirms that no temporary material remains. The
focused filesystem group passes 27/27. The complete folder-sync library passes
140/140 with 14 ignored.

This closes the ordinary partial-write window for folder-sync export and keeps
the existing symlink, reparse-point, hard-link, depth, entry, and byte-limit
guards. A filesystem actor that replaces an ancestor between inspection and
the final open/rename still requires handle-relative, cross-platform proof and
is not claimed here. Redis legacy-writer coordination, production Redis
Cluster-aware pool deployment, remote S3 deployment proof, Unix/macOS replay,
and full product acceptance remain open.

Validation:

- `cargo test --offline --locked --lib provider_credential_folder_sync::tests::filesystem -- --test-threads=1`: 27 passed, 0 failed.
- `cargo test --offline --locked --lib provider_credential_folder_sync -- --test-threads=1`: 140 passed, 0 failed, 14 ignored.
- Targeted `rustfmt --check`: passed.
