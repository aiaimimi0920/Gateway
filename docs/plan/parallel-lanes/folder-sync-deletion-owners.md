# Folder synchronization deletion ownership

Status: verified deletion boundary, coordinator-owned S09 increment, 2026-09-15.

Parent 6441 -> 5975; deletion/tests 219/271. Paired folder_sync 45/45, exact source
proof and original assertions preserved. Only six named test module paths change.
All-targets, scoped formatter, checker 19/19, ratchet and both Git checks pass;
all native phases terminal. Strict 2100/12/20/39; 32 above 700; clearance 113/145
(77.9%). The full parent remains legacy debt. Path containment is the next repair.
[Acceptance report](../../status/2026-09-15-folder-sync-deletion-owners.md).

## Design and verification boundary

Before capture revalidates the accepted folder-sync status checkpoint
ec1637568b5e74ea2e3439c41f50c81967b698d14085e648c292db3c530d7c6e. Root baseline is 6441 effective lines.
Own only src/provider_credential_folder_sync.rs deletion code and its six tests,
new provider_credential_folder_sync/deletion.rs and deletion/tests.rs. Preserve the
227-line status owner and all other inputs. S06 source/cursor/final-build reservation
remains GWP-20260912-01; no original plan cursor or runtime profile changes.

Move the public synchronous file-deletion API, explicit/missing credential deletion,
watch-event deletion paths, materialization/archive/path predicates and bounded
recent-event summary. Move their private hit record and six original tests. Keep
the shared counters, root/path normalization and watcher filters in the parent.
The shared test credential constructor stays in the original test module with
pub(super) visibility for the moved tests; its body is unchanged. Four internal
production entry points gain pub(super); all other deletion internals stay private.

Preserve original public API via re-export, operation order, errors, runtime-key
cleanup, counters, archive/materialization/recreated-path guards, first-seen dedupe
and eight-event retention. Export's stale-file reconciliation loop belongs to the
export lifecycle and remains there. Do not mix new deletion policy with extraction.
The path assembly currently accepts parent components and has no explicit canonical
containment gate; preserve it here and evaluate that separate behavior boundary
with failing-before filesystem tests in a later hardening lane.

Run the same broad folder_sync library filter before/after. Verify all 45 logical
test identities, allowing only the six explicit tests to move from root::tests to
root::deletion::tests, with exact original bodies and helper preserved. New owners
must be <=500. The root must decrease and remains legacy debt, not fully cleared.
Freeze sources/tests/assets during serialized native gates; run all-targets,
scoped rustfmt, checker 19/19, ratchet, strict and both repositories' Git checks.
Capture exact official-rustfmt source projections and independent semantic review.
Live DB/Redis, whole-root migration, full runtime/provider/UI/Docker/release remain open.
