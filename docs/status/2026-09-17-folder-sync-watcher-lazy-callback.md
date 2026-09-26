# Folder-sync watcher lazy callback checkpoint, 2026-09-17

This checkpoint narrows callback-side allocation when auto sync is disabled or
the mailbox receiver has already closed. The broader folder-sync optimization
and release goal remains active.

## Implemented boundary

SignalSender::send_lazy checks receiver state and the current runtime epoch
before invoking the callback builder. Native and poll watcher callbacks now use
that path for filesystem events and watcher errors, so disabled or closed
watchers do not construct deletion-path sets or error strings. The existing
epoch discard, two-node coalescing, and bounded deletion-intent behavior remain
unchanged.

## Verification

- cargo test --offline --locked --lib provider_credential_folder_sync::watcher -- --test-threads=1: 39 passed, 0 failed.
- cargo test --offline --locked --lib provider_credential_folder_sync:: -- --test-threads=1: 130 passed, 0 failed, 12 ignored.
- cargo check --offline --locked --all-targets: passed with existing Gemini warnings only.
- Scoped rustfmt --edition 2021 --check: passed for watcher.rs, mailbox.rs, and mailbox/tests.rs.
- npm run test:effective-lines --prefix scripts: 19 passed.
- npm run check:effective-lines --prefix scripts: ratchet passed; inventory 2,183 files.
- git diff --check and git diff --cached --check: passed.

## Limits

The fast path avoids callback construction only when state is already inactive;
a concurrent disable or receiver drop can still race after the precheck and
discard a constructed signal. Native backend allocation, active-event path
vectors, native operation deadlines, complete shutdown drain, status races,
blocking-pool fairness, filesystem replacement and hard-link/reparse races,
Unix/macOS behavior, guarded PostgreSQL/Redis execution, remote S3 success/read
proof, and full provider/UI/Docker/release acceptance remain open. No release
artifact, port 4200, or Neuro/release/Gateway state was touched.
