# Folder watcher native lifecycle ownership, 2026-09-16

The S09 watcher native-owner checkpoint is verified. Root directory creation,
native/poll watcher construction and watcher-resource destruction now run on one
dedicated OS thread. The folder task responds to the existing application shutdown
signal and waits for its native resources to be destroyed. Library tests pass
116/116 with six unchanged ignored tests; guarded real Redis runtime tests pass
18/18. The overall optimization and release goal remains active.

## Ownership and behavior

The native owner creates and destroys the watcher pair on the same thread. Its
readiness channel carries only optional running status; native handles never move
back to the async executor. A stop sender controls the idle lifetime, and a separate
completion receiver acknowledges destruction. A failed setup returning None exits
without watchers. Watch-disabled or all-backends-failed setup does not retain an
idle owner thread. No Tokio blocking-pool slot is occupied while watchers wait.

This boundary includes destruction because notify 6.1.1 has platform-dependent
blocking behavior: Windows/Linux/kqueue watch operations wait for backend acks,
PollWatcher watch scans synchronously, and macOS FSEvents stop can spin and join.
Only offloading construction would leave blocking destruction on the executor.
The cost is one extra sleeping OS thread per running watcher pair. No global
worker quota, throughput improvement or native setup deadline is claimed.

The async task stores the native owner before awaiting readiness. Its outer
shutdown selection can therefore cancel a pending startup/status/enable/loop
future and then await native destruction, including unfinished setup. Already
requested shutdown takes priority. Caller abort or Tokio runtime teardown drops
the stop sender; the native worker remains responsible for late resources after
non-preemptible setup returns. Factory panic closes readiness and completion, which
map to the fixed provider_credential_folder_watch_owner_failed error. New failure
logs include error kind without captured configuration or panic payloads.

Only the resolved root, folder logging flags, debounce and mailbox sender cross
the native boundary. AppState and unrelated configuration do not. Root resolution
remains after enablement; mkdir and its failure log remain before startup tracing
and native-then-poll construction. Root creation failure still returns before
startup status. Backend failure still preserves partial success and periodic
fallback. Normal startup status remains after both backend attempts.

The existing disabled-start sequence, initial synchronization and complete watcher
loop retain their statements. Periodic/debounce timing, disable epochs, event
filtering, status bodies and success-only deletion-intent clearing are preserved.
All native/poll constructor and callback bodies remain byte-exact; watcher.rs gains
only its native module declaration. Management deletion remains separate.

## Shutdown correctness and limits

Review found a pre-existing registration race in GatewayShutdownHandle::wait:
request could set its atomic flag and call notify_waiters between the flag check
and creation of Notified. The method now creates Notified before checking the
persistent flag. Pinned Tokio 1.51.0 explicitly guarantees notify_waiters delivery
from future creation, even before polling. Every other byte in state.rs is exact.
This scope extension was recorded after baseline; no baseline test was changed.

The registration correction has source-order and pinned-library contract evidence,
plus compilation and functional shutdown tests. There is no deterministic
failing-before test of that tiny cross-thread registration window, and the
current-thread tests do not prove every multithreaded interleaving.

Native calls cannot be preempted, so a stuck constructor or destructor can still
delay shutdown indefinitely. The three-second timeout in the fixture is a test
rescue, not a product deadline. The completion signal confirms resource destruction,
not an OS owner-thread join. notify Windows/Linux/Poll backends also detach their
own threads; the real unit test additionally waits for all callback senders to
disappear. No cross-platform backend-join claim is made.

Stopping the watcher cancels its awaiting future, while previously admitted sync
runs retain their independently verified caller-drop policy and can finish later.
This checkpoint does not drain all sync runs, guarantee final status persistence,
or complete application-wide graceful shutdown. Post-shutdown status persistence,
native deadlines and shared-pool/global admission remain open.

## Fresh verification

The preceding blocking-run scope, publication, receipts, source/test/assets and
both repository HEAD/status sets were revalidated before edits. The baseline used
a test-only inline owner to model native construction/destruction on the executor;
production watcher/runtime remained unchanged. Library baseline: 110 passed /
6 failed / 6 ignored, 3038 filtered, in 7.02 seconds. Candidate: 116/116, six ignored,
3038 filtered, in 0.90 seconds. All 113 predecessor identities/results remain exact.

The six repaired library contracts cover setup responsiveness, destruction
responsiveness, construction/destruction on one non-executor thread, caller
cancellation during setup, Tokio runtime teardown and fixed panic propagation.
Three other new cases cover an occupied one-slot Tokio blocking pool, empty
initialization, and real native/poll callback cleanup. All nine new tests and every
baseline-executed test file remain hash-exact across candidate validation.

The separate real Redis baseline exercised unchanged production: 15 passed /
3 failed in 9.96 seconds. The failures demonstrate missing shutdown response while
disabled, after real watcher initialization, and while the only Redis connection
is held during disabled-status I/O. Candidate: 18/18, zero ignored, in 0.95 seconds.
All 15 predecessor runtime identities/results remain exact. Existing cancellation
tests retain their abort assertions; the added graceful fixture method rescues
timed-out old tasks before failing and removing their directories.

Serialized fresh gates passed:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_runtime -- --ignored --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <9 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Deletion integration passes 9/9 in 0.27 seconds; checker tests pass 19/19. Both
repositories' staged/unstaged git diff --check and terminal native-idle guards pass.
Warning sets match the predecessor: three library warnings, one lib-test warning
and the prebuilt-web-assets notice. No post-freeze source correction, successful
native-gate replay or evidence-script correction was needed.

Each real Redis phase used an owned guarded container on a random loopback port.
Both containers and run-owned directories were removed; all 45 prior container
identities/states were preserved. The real native callback fixture also left zero
gateway-folder-native temporary directories. Persistent services were untouched.

## Size, review and evidence

| Scoped file | Before | After |
| --- | ---: | ---: |
| src/provider_credential_folder_sync/watcher.rs | 163 | 164 |
| src/provider_credential_folder_sync/watcher/runtime.rs | 256 | 299 |
| src/provider_credential_folder_sync/watcher/native.rs | new | 77 |
| src/provider_credential_folder_sync/watcher/native/tests.rs | new | 151 |
| src/provider_credential_folder_sync/watcher/native/tests/lifetime.rs | new | 108 |
| tests/provider_credential_folder_sync_runtime.rs | 100 | 102 |
| tests/provider_credential_folder_sync_runtime/shutdown.rs | new | 43 |
| tests/provider_credential_folder_sync_runtime/fixture.rs | 178 | 204 |
| src/state.rs | 170 | 171 |

All 64 folder-sync Rust files remain at most 500 effective lines, maximum 460.
Strict retains expected exit 1: 2167 scanned / 11 hard / 20 mandatory / 39 soft.
All 31 above-700 entries retain exact hashes/counts; clearance remains
114/145 (78.6%). Source/test union is 1872, unchanged neighbors 1863, unchanged
web/Tauri assets 22. No exception or checker-policy change was needed.

Five read-only reviews and coordinator verification cover native ownership,
fixture cleanup, Notify registration and task wiring. reviews.md records accepted
findings and corrections: outer async cancellation does not await a held pool
connection; admitted run inputs do not retain AppState; mailbox is_current checks
enabled as well as epoch. Remaining coverage limits are explicitly recorded.

Evidence: target/effective-line-evidence/20260916-folder-sync-watcher-native-owner/.
scope.json observedAt: 2026-09-16T03:00:56.464Z. Scope SHA-256:
683b7431da29d0bb9ffebc723950c9d72514e05042f73190ee9dbfd17e33ebfd.
Original/staged/candidate source, immutable snapshots, logs, receipts, guards,
scripts and reviews are hash-bound. publication.json verifies all five checkpoint
documents and exact repository path-status deltas.

After publication, Gateway retains 194 modified, one unstaged deletion, two staged
deletions and 2411 untracked entries. This checkpoint adds one production file,
three test files and two documents. Neuro retains 10 modified and 182 untracked.
HEADs remain 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commits, dependencies, baseline,
exceptions, checker-policy, persistent-service or release changes occurred.

## Remaining work

Management credential/account deletion still performs synchronous filesystem work
from async HTTP handlers; its authorization, delete-before-DB ordering and late
effects need an independently owned offload. Native deadlines, all-backend failure
cases, readiness/destructor-panic interleavings, global admission, status priority
and complete application/sync-run shutdown drain remain open.

The Redis fixture has PgPool=None. Successful PostgreSQL import/export and final
metadata/status under load remain unverified. DB hydration/parsed-memory and
explicit-path budgets, concurrent replacement/hard links, cross-process exclusion,
Unix/macOS execution, provider/UI/Docker and full release acceptance remain open.
The other 31 oversized files are untouched. S06 source/cursor/final native build
remains reserved under GWP-20260912-01; target 4200 and Neuro/release/Gateway are
unchanged. The overall optimization goal stays active.
