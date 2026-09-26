# Folder synchronization disable epochs

S09 coalesced-disable admission is verified on 2026-09-16. One typed watch snapshot
holds enabled plus an owned Arc epoch, rotated atomically on true -> false. Mailbox
invalidation precedes coalescing, dequeued envelopes are checked again, and every
loop branch reconciles pending timer/path ownership. Obsolete selected timer work
is skipped; new-epoch paths survive delayed control processing. Two signal kinds
and exact deletion intent remain; stale sets are freed outside the callback lock.

Initial baseline did not compile because Tokio test-util is unavailable. Only the
work timer fixture was corrected, without dependency changes. Executable staged
baseline2: 66 passed / 6 failed. Same candidate library tests: 72/72, six unchanged
CAS tests ignored. Candidate changes only epoch rotation; all original 63 identities
remain, with eight original mailbox bodies preserved through a test-only adapter.
Four parallel senders preserve 256 exact new paths across the toggle boundary.
Unchanged guarded Redis runtime 10/10; no PostgreSQL synchronization claim.

All-targets/fmt/source/checker 19/19/ratchet/Git pass. A native-process guard paused
before checker tests; idle recovery resumed unfinished gates without replaying any
success. The private Redis fixture/directories are cleaned; all 45 existing
containers retain identity/state. State/watcher/runtime/mailbox/tests 165/163/256/
161/136; new work/tests/adapter/epoch tests 28/93/16/127. Root unchanged 5543.
Strict 2124/12/20/39; all 32 oversized hashes/counts unchanged; clearance 113/145
(77.9%). Union 1829, unchanged neighbors 1820, unchanged assets 22.

Rust subscribe() now returns the typed snapshot; its sole existing production
consumer is migrated. HTTP/Redis contracts and bool accessors remain. Admission
does not cancel in-flight work or classify late OS callbacks by occurrence time.
I/O deadlines, backend shutdown, shared status priority, durable audit, database/
root migration and full release acceptance remain open. S06 source/cursor/final
build remains reserved under GWP-20260912-01.
Evidence: target/effective-line-evidence/20260916-folder-sync-disable-epochs/.
Scope SHA-256: a41c82824699bf8f0354684573e0a5e8dc99ae0c411039506fcb24ad07f879bd.
[Report](../../status/2026-09-16-folder-sync-disable-epochs.md).
