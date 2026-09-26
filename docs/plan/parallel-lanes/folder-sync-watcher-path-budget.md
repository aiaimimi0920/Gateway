# Folder-sync watcher deletion-intent bounds

Verified on 2026-09-18. Native and polling watcher callbacks now retain deleted
relative paths through `BoundedPathSet`, with an 8 MiB conservative byte budget
including each string's allocated capacity and a 128-byte per-entry bookkeeping
margin. Duplicate paths are charged once. The bound is
enforced when an event is built, when mailbox filesystem events are coalesced, and
when pending work receives a coalesced event, so queue-node bounds cannot hide
unbounded path data.

Overflow is fail-closed for deletion: the bounded set drops all partial paths and
marks itself overflowed. The runtime cancels the pending debounce, clears the
overflow marker, records a stable watcher error in the existing `last_watch_error`
status field, and skips that event's explicit deletion intent. Later events can
accumulate a fresh bounded set. Epoch changes and disable reconciliation clear both
paths and overflow state. No ancestor path is invented and no partial set reaches
the database deletion phase.

The mailbox still coalesces at most one filesystem node and one watcher-error node.
Merging an overflowed node propagates overflow and releases all retained paths.
Existing latest-kind ordering, epoch rejection, duplicate coalescing, parallel
sender preservation and receiver cleanup remain unchanged.

Focused overflow, recovery and merge tests pass. The complete folder-sync unit
group passes 126/126 with 12 external-service tests ignored. All-targets and
scoped formatting pass; checker 19/19 and the effective-line ratchet pass. The
new owner remains below the 500-line boundary. Existing Gemini/S06 warnings and
global formatting differences remain outside this lane.

The 8 MiB budget limits watcher deletion intent retained between filesystem
callbacks and a scheduled run. It does not certify native callback allocation
before `notify` constructs an event, nor database snapshot or material budgets.
Native deadlines and complete drain, status ordering, blocking-pool fairness,
filesystem replacement races, Unix/macOS behavior, selected remote object reads and
full provider/UI/Docker/release acceptance remain open. The single-account transient
hydration bound is now covered by the 2026-09-18 checkpoint. S06 stays reserved and
the overall goal remains active.

Evidence: `target/effective-line-evidence/20260918-folder-sync-watcher-path-budget/`.
[Report](../../status/2026-09-18-folder-sync-watcher-path-budget.md).
