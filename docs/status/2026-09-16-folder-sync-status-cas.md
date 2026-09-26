# Folder synchronization status conditional commits, 2026-09-16

S09 stale whole-status overwrites are repaired across management, watcher and run
result writers. Real Redis conflict regressions advance from 3 passed / 3 failed
to 6/6 with frozen caller/test bytes. The library suite passes 63/63, and the
unchanged isolated startup/runtime suite passes 6/6. Full optimization and release
acceptance remain open.

## Cause and change

All writers previously read the complete status JSON, changed their own fields,
then unconditionally SET the whole object. A watch update could erase a completed
run's counters or deletion history; a long run could restore an obsolete watch
timestamp or error. Management configuration updates had the same overwrite path.

The new status/store owner compares the exact JSON snapshot before committing.
Each attempt obtains the latest value, decodes it in Rust, applies the caller's
metadata mutation and serializes it. A single-key Lua script compares presence
and raw bytes, then performs SET only when that snapshot still matches. A missing
snapshot cannot overwrite a value created by another writer. Lua never decodes
JSON or converts status integers to floating-point numbers.

Only compare conflicts are retried, with eight total attempts. Every attempt
reapplies the mutation to a fresh DTO; it does not reuse a modified old snapshot
or replay a static JSON diff. Exhaustion returns HTTP 409 without a blind write.
Redis I/O and JSON errors propagate immediately. There is no WATCH/MULTI state
left on a pooled connection and no additional task or cross-key transaction.

The run owner carries successful import/export phase flags and existing counters
to the final status mutation. It retains timestamps for phases that did not
complete, keeps independent watcher fields, merges deletion events into the latest
history and clears last_error on success. Existing event ordering and the eight-
event history limit remain. Database import/export work executes once; only its
metadata mutation is retried.

Initial status validation still precedes filesystem/database effects. A run also
reloads status at commit time, adding a status read compared with the former
long-lived snapshot. This is a correctness tradeoff; no latency improvement is
claimed. The retry count is bounded, not the duration of every Redis await.

The Redis key, DTO schema, public exports, configured fields, enabled override
I/O and watch None/clear semantics are preserved. Shared config/watch_running
fields keep their existing policies; CAS does not give conflicting same-field
writes a new priority or make enabled override and process memory transactional.

## Executed verification

The staged baseline first moved callers to the owned-field update boundary while
retaining unconditional GET/SET. It is explicitly not an untouched-source baseline.
The candidate changes only status/store.rs; the other six source/test/fixture files
are byte-identical between baseline and candidate.

The dedicated regression command was executed in separate guarded Redis fixtures:

    cargo test --offline --locked --lib folder_sync::status::store::tests -- --ignored --test-threads=1

Baseline: 3 passed / 3 failed. Candidate: 6/6, zero ignored. A second real Redis
connection writes a competing value after the tested GET and before SET/CAS.
The tests verify concurrent run/deletion audit preservation, concurrent creation
after a missing snapshot, bounded contention without overwrite, exact u64/usize
maximum values, malformed JSON preservation and missing-key initialization.
No production hook, timing guess or mock Redis substitutes for this boundary.

Fresh closing gates pass:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo check --offline --locked --all-targets
    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1

The library has 63 passes, six ignored and 3038 filtered out. Those six ignored
Redis regressions were executed explicitly above. All prior 60 library identities
remain; three new run-owner tests protect uncompleted phase/watch-field retention,
history merge and failed-run metadata. The unchanged startup/runtime target passes
6/6 with zero ignored, retaining actual file-event/status and task cleanup checks.
It does not prove successful PostgreSQL credential synchronization.

Scoped official rustfmt --check, checker tests 19/19, the effective-line ratchet
and both staged/unstaged Git checks in Gateway and Neuro pass. Strict exits 1 for
remaining legacy debt. Gates ran serially against frozen sources/assets. There
was no candidate correction, fixture correction or successful-test replay.
Existing Gemini warnings remain. Independent design, candidate and regression
reviews and their dispositions are retained in the evidence directory.

All three private Redis containers are removed; no run-owned temporary directories
remain. The same 45 pre-existing container identities/states remain. No persistent
service changed.

## Source and evidence

Effective lines: root 5553 -> 5543; status 227 -> 190; new run owner 53; new store
78; store tests 141; Redis fixture 57; run tests 90. New and completed owners are
below 500. The legacy root remains incomplete, with only its status wiring changed.
Whole-root reconstruction binds both phase flags to their original success points
and preserves the entire business/test suffix. The full status owner reconstructs
from the original, including eight unchanged functions and the exact DTO schema.

Strict: 2117 scanned / 12 hard / 20 mandatory / 39 soft. There are still 32 files
above 700; the other 31 keep identical hashes/counts. Clearance is 113/145 (77.9%).
The input union is 1822 files, with 1815 unchanged neighbors and 22 unchanged
web/Tauri assets. Dependencies, policies, baselines and exceptions are unchanged.

Evidence: target/effective-line-evidence/20260916-folder-sync-status-cas/.
Snapshots, original files, paired tests, logs, cleanup, source reconstruction,
reviews and scripts are hash-bound. scope.json observedAt:
2026-09-15T19:58:52.112Z. Scope SHA-256:
ef02f6d2be86277ba6b38e1857174b0594f1460f751da1300d65f71c0c9ece93.

publication.json verifies the final five documents and exact Git deltas. Gateway
retains 193 modified, 1 unstaged deletion, 2 staged deletions and 2341 untracked
entries; Neuro retains 10 modified and 182 untracked. Gateway HEAD remains
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d; Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a. No profile, staging, commit or release
change occurred. Modified source and documentation are UTF-8 without BOM.

## Remaining requirements

Concurrent management setters can still disagree between the enabled override
key and process memory. Rapid boolean-watch coalescing, filesystem events retained
across disable, startup I/O deadlines and full native/poll backend shutdown remain
lifecycle work. Shared-field ordering and cancellation of management operations
are not covered by this single-key status compare-and-commit contract.

After database effects, failed/exhausted status persistence still returns an error
without rolling those effects back. Ambiguous I/O or cancellation can leave a caller
uncertain whether Redis committed. Durable audit/outbox semantics and end-to-end
database import/export are not accepted. Deletion-path storage, directory replacement
races, remaining root migration and full strict/feature/provider/runtime/UI/Docker/
release gates remain open. S06 original source/cursor/final build remains reserved
under GWP-20260912-01. Persistent target stays 4200; release root stays
Neuro/release/Gateway. The overall optimization goal remains active.
