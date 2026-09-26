# Folder watcher native lifecycle owner

The S09 watcher-native checkpoint is verified on 2026-09-16. One dedicated OS
thread owns root mkdir, native/poll construction, idle stop waiting and resource
destruction. Readiness transfers status only. No Tokio blocking-pool slot remains
occupied while watchers wait; the cost is one extra sleeping owner thread per
running pair. Disabled/all-failed setup exits without retaining an idle thread.

The shutdown-aware wrapper stores ownership before readiness and waits for native
destruction after cancelling startup/status/loop waits. Caller abort signals stop
without preempting native setup. Completion confirms resources destroyed, not OS
or backend joins. Admitted sync runs remain independently owned and can finish
later; full application drain, native deadlines and final status remain open.

The existing Notify registration race is fixed by creating Notified before checking
the persistent shutdown flag. Pinned Tokio documentation proves this notify_waiters
ordering; no deterministic failing-before registration-race test is claimed.
state.rs was explicitly added after baseline; all other state bytes remain exact.

Inline-owner library baseline: 110 passed / 6 failed / 6 ignored. Candidate 116/116,
six ignored, all 113 prior identities exact. Nine native tests cover responsiveness,
thread ownership, cancellation, panic, pool independence and real callback cleanup.
Unchanged-production real Redis baseline: 15 passed / 3 failed; candidate 18/18,
all 15 prior identities exact. All executed baseline tests remain frozen.

Deletion 9/9, all-targets/fmt/checker 19/19/ratchet/both Git checks pass. Fixtures
cleaned, zero native temp roots, 45 prior containers preserved. No post-freeze
source correction, native success replay or proof-script correction was needed.

Watcher/runtime/native/native-tests/lifetime/runtime-entry/shutdown/fixture/state:
164/299/77/151/108/102/43/204/171 effective lines. Original watcher loop/status and
native constructor bodies are preserved. All 64 subtree Rust files <=500, maximum
460. Strict 2167/11/20/39; 31 unchanged above 700; clearance 114/145 (78.6%).
Union 1872, unchanged neighbors 1863, assets 22.

Next: management deletion offload with authorization, delete-before-DB order and
cancellation ownership preserved. DB success/hydration, memory/path budgets,
deadlines/full shutdown/status priority, backend interleavings, races/hard links,
cross-process exclusion and full provider/runtime/UI/Docker/release acceptance
remain open. S06 stays reserved; the overall goal is active.

Evidence: target/effective-line-evidence/20260916-folder-sync-watcher-native-owner/.
Scope SHA-256: 683b7431da29d0bb9ffebc723950c9d72514e05042f73190ee9dbfd17e33ebfd.
[Report](../../status/2026-09-16-folder-sync-watcher-native-owner.md).
