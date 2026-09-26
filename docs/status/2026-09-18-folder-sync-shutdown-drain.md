# Folder-sync shutdown drain and native deadlines, 2026-09-18

This checkpoint hardens watcher shutdown without changing the public status shape.
The folder-sync task now treats admitted sync operations as drainable work: a
shutdown request is observed at run boundaries, but the active operation is allowed
to finish its final status CAS before the watcher task exits. The first status
publication for an enabled runtime is also drainable; disabled-startup and watcher
diagnostic status writes remain cancellable, so a blocked Redis connection cannot
hold shutdown indefinitely when no enabled work has been admitted.

## Native owner boundary

Native watcher readiness and destruction have a ten-second operation deadline. A
deadline failure returns the stable
`provider_credential_folder_watch_owner_deadline_exceeded` error. Native setup and
destruction themselves remain non-preemptible; after a timeout, the detached owner
thread still owns the resources and observes the stop signal before dropping them.
The deadline is therefore a caller responsiveness bound, not a claim that the OS
backend has been forcibly interrupted.

## Verification

- Native watcher tests: 11 passed, 0 failed, including readiness and shutdown timeout
  cleanup ownership.
- Complete watcher group: 41 passed, 0 failed.
- Folder-sync run ownership group: 10 passed, 0 failed.
- `cargo test --offline --locked --test provider_credential_folder_sync_runtime --no-run`: passed.
- `cargo check --offline --locked --all-targets`: passed with existing Gemini warnings.
- Scoped rustfmt and `git diff --check`: passed.

The guarded PostgreSQL/Redis runtime suite remains environment-dependent and was
not claimed here. Remote S3 success/read proof, status ordering under cross-process
writers, blocking-pool fairness, filesystem replacement and hard-link/reparse
races, Unix/macOS backend behavior, and full provider/UI/Docker/release acceptance
remain open. No release artifact, port 4200, or `Neuro/release/Gateway` state was
touched.
