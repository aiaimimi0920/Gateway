# Folder synchronization enabled atomic commit

Status: verified on 2026-09-18.

The management setter now commits the Redis status and enabled override with one
raw-snapshot Lua CAS. Observer status writes use a matching two-key snapshot and
reload after concurrent management changes. Shared overrides are authoritative;
missing overrides leave startup runtime ownership local. Malformed status no longer
leaves a partially persisted override or local runtime mutation.

Enabled-startup status publication drains during shutdown; disabled-startup and
diagnostic writes remain cancellable. Guarded Redis status tests pass 7/7 and the
complete folder-sync runtime fixture passes 22/22, including cooperating Gateway
runtime watcher convergence through Redis Pub/Sub and enabled-override reconciliation
after a forced Pub/Sub disconnect without a corresponding event. Closing
library, compilation, checker and diff gates are recorded in the status report.

This lane claims watcher convergence only for cooperating Gateway runtimes. It does
not claim Redis Cluster support, arbitrary legacy-writer coordination,
disconnect-window product guarantees, or full provider/runtime/UI/Docker/release
acceptance.
