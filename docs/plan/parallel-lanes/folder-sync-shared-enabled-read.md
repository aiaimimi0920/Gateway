# Folder synchronization shared-enabled read projection

Status: verified on 2026-09-18.

The status read path now prefers the Redis enabled override when the key exists,
then applies the static folder-sync configuration without changing local runtime
state. A missing override keeps the startup runtime fallback. This prevents a
status response in one process from replaying that process's stale `enabled`
value after another process changes the management override.

The focused status tests pass 9/9, the complete folder-sync library passes 138/138
with 12 guarded tests ignored, and the guarded runtime integration target compiles.
The new Redis regression remains ignored until its dedicated fixture environment is
available. All-targets, effective-lines 19/19 plus ratchet, rustfmt and diff checks
pass. This lane does not claim watcher convergence or distributed write ordering.

Next: provide the guarded multi-process fixture and define the precedence/commit
protocol for rapid management toggles versus watcher and completed-run observers.

