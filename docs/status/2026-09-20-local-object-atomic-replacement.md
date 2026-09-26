# Local object atomic replacement, 2026-09-20

Local object-storage writes now serialize into a uniquely named temporary file
in the destination directory and replace the final object with a same-directory
rename. A failed write or replacement removes the temporary file. Readers keep
the existing bounded-size and containment checks, while S3-compatible writes
retain their existing network path and deadlines.

The local provider-account object regression writes an object, replaces it,
reads the replacement, and verifies that no temporary file remains. S3 object
listing now uses the same network deadline as PUT/GET/DELETE, with a stalled
listing regression. The full object-storage group passes 13/13, including the
local junction escape guard, cycle-safe listing, S3-compatible round trip, and
network deadline tests.

Local listing now uses Tokio filesystem metadata, canonicalization, directory
iteration, and an explicit stack instead of synchronous recursive filesystem
calls on the async executor. The containment and cycle guards remain in the
same traversal boundary.

All Local operations now share an asynchronous owner lock keyed by the
configured root. Reads, writes, deletes, listing, and readiness probes cannot
interleave their containment inspection and local filesystem effect inside the
same Gateway process. The lock is intentionally limited to Local storage and
does not serialize S3-compatible operations.

This closes the local partial-write window for ordinary replacement. It does
not claim protection against a concurrent filesystem replacement between
containment inspection and the final open/rename; handle-relative access and
cross-platform race proof remain open. Remote S3 production deployment proof,
Redis Cluster client deployment, Unix/macOS replay, and full product acceptance
also remain open.

Validation:

- `cargo test --offline --locked --lib object_storage::tests -- --test-threads=1`: 14 passed, 0 failed, including the per-root owner-lock regression.
- `cargo check --offline --locked --all-targets`: passed.
- Targeted `rustfmt --check` for the five changed Rust files: passed.
- Effective-lines tests: 19/19 passed; ratchet: passed.
- `git diff --check`: passed.
