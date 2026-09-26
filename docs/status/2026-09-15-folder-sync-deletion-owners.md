# Folder synchronization deletion ownership, 2026-09-15

The S09 deletion owner and original test migration are verified. Whole-root
migration, deletion hardening and full Gateway runtime/release acceptance remain open.

## Implemented ownership

provider_credential_folder_sync/deletion.rs now owns the private explicit-hit
record and twelve functions: public synchronous file deletion, explicit/missing
credential deletion, deleted watcher paths, materialization/archive/path predicates,
event construction and recent-event summary. Its six original tests live in
provider_credential_folder_sync/deletion/tests.rs. The parent retains 39 tests.

The original public delete_synced_credential_file path is preserved by re-export.
Four internal entry points gain pub(super). The shared original test credential
constructor stays in the root test module; only its visibility becomes pub(super),
allowing the migrated tests to reuse its unchanged body. All other deletion
internals remain private. Parent Pool remains required by import and is retained.

The extraction preserves DB deletion followed by runtime-key cleanup and counter
increment, error propagation, audit timing, archive/materialization/recreated-path
guards, first-seen path/ID dedupe, unique-path event counts and eight-event history.
Public signatures and synchronous/async behavior are unchanged. Shared counters,
path normalization, watcher filters and import/export orchestration stay in the
parent. Export's stale-file reconciliation remains with its export lifecycle.
The accepted 227-line status owner is byte-identical to its predecessor snapshot.

Effective lines: parent 6441 -> 5975, a reduction of 466; deletion owner 219;
migrated test owner 271. The first two S09 increments reduce the original 6655-line
parent by 680 lines. Both new owners are below 500. The parent remains >700 legacy
debt; no additional cleared-file count is claimed. No checker, policy, baseline,
exception, dependency or runtime-profile change was made.

## Fresh verification

Both fresh runs used GATEWAY_PREBUILT_WEB_UI=1 and:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1

Original baseline 45/45; first frozen candidate 45/45, both with 3038 filtered out.
The verifier permits exactly six named tests to change module path from
provider_credential_folder_sync::tests to provider_credential_folder_sync::deletion::tests.
All other identities remain exact, and all 45 logical identities are unique and
unchanged. Original assertions and the shared constructor body reconstruct exactly;
no fixture was rewritten and no new test duplicates existing implementation.

Moved coverage includes deleted-path extraction, active/materialized/archive
predicates, recreated-path protection, summary order and empty-event behavior,
path/ID dedupe and materialized-state classification. Full parent/owner/test source
is reconstructed from the hashed original file with exact moves, import rewiring
and allowed visibility changes, then compared with official rustfmt output.

All-targets cargo check, scoped official rustfmt --check, checker tests 19/19,
ratchet, and both Gateway/Neuro staged and unstaged Git checks pass. All native
phases are terminal. Successful warning sets are identical; the inherited Gemini
unused HashMap warning remains. Strict exits 1 for existing debt: 2100 scanned /
12 hard / 20 mandatory / 39 soft. Above-700 membership remains 32; only this parent's
line count changes. Clearance stays 113/145 (77.9%).

## Preserved risks and next hardening

The independent review found no introduced extraction regression. It also identified
inherited behaviors that this structural checkpoint does not fix:

- Path normalization retains parent components; path assembly does not enforce
  canonical root containment. The public file deletion boundary therefore needs
  failing-before filesystem regression coverage and a dedicated containment fix.
- Synchronous file deletion retains its exists-then-remove check and race window.
- Explicit and missing deletion loops share the same pre-import credential snapshot.
  A shared hit may be processed twice when delete_missing is enabled; inspect exact
  DB return semantics and reproduce before changing counting/failure behavior.
- Event counts represent unique paths, not the number of deleted credentials;
  paths recreated before watcher processing are not treated as removed.

The existing tests do not establish live DB/Redis failure semantics, containment,
symlink/race safety or provider acceptance. Path containment is the next behavioral
priority; watcher ownership and remaining root migration follow as independent work.

## Evidence and repository state

Evidence: target/effective-line-evidence/20260915-folder-sync-deletion-owners/.
Before capture revalidated the status scope, 1803 inputs, 22 assets, five docs and
exact Git states. Final union: 1805 inputs; 1802 unchanged neighbors; 22 unchanged
web/Tauri assets. Source, logs, receipts, process guards, inventory, review and
verification scripts are hash-bound. All modified/new source, scripts and docs
are UTF-8 without BOM. This lane needed no failed patch, formatter or candidate
correction; preceding checkpoints keep their own historical failure evidence.

scope.json observedAt: 2026-09-15T15:41:13.677Z.
Scope SHA-256: aaf6b0651f6eb2ff876857f6c820ea7c6f46109a52cbee596799fd4e35cecfc9.
Source SHA-256 values:

- provider_credential_folder_sync.rs: fc5ed2e8fb8ff9877466525ccd178e9111f7421af2eb7dd233f3af48588e3ead.
- deletion.rs: 32a8d226063cefb7e02d483141a03b39b766a94982121a3989a673618fc700d8.
- deletion/tests.rs: 6e6235db913c3e2a77ad1a32d36e6f78582ebca77f7dea9bf835ed57ce951621.

publication.json verifies the final five docs, source/evidence integrity, script
limits and exact Git delta. Gateway: 193 modified, 1 unstaged deletion, 2 staged
deletions, 2312 untracked; Neuro: 10 modified, 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit or release occurred.

Full strict/feature/language/provider/runtime/UI/Docker/release gates remain open.
S06 original implementation/cursor and final-build coordination remain reserved
under GWP-20260912-01. Persistent service target stays 4200; releases belong only
under Neuro/release/Gateway.

