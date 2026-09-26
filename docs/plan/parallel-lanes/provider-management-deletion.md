# Provider management deletion owner

Verified on 2026-09-16. Both authorized management DELETE routes admit one
nonqueued operation per AppState before copying inputs or hydrating database rows.
Busy returns HTTP 409 with provider_management_delete_busy. The independent permit
is owned by GatewayLifecycleState; synchronization/enable permits are unchanged.

The complete original ordered file, DB/object and Redis operation runs on a blocking
worker. Caller cancellation leaves admitted cleanup running. Runtime teardown wakes
async waits, while native calls remain non-preemptible. Original auth/Pg checks,
response literals, serial account loop, credential-only 404 handling and every
unrelated handler statement are preserved by complete formatted-source proof.

Library inline baseline 2 passed / 5 failed; candidate 7/7. Accepted real PostgreSQL/
Redis route baseline3 4 passed / 2 failed; candidate 6/6. Row-lock cancellation
contracts prove continued DB/cache cleanup after both request futures are aborted.
Success JSON, invalid paths, disabled/manual files and cross-route admission pass.
Initial fixture module-path and execution-mode corrections are retained; all
accepted baseline test bytes remain frozen. No candidate source correction/replay.

Deletion 9/9, two auth targets 4/4 each, all-targets/scoped fmt/checker 19/19/ratchet/
both Git checks pass. Eight owned containers/anonymous volumes and temporary roots
are cleaned; 45 prior containers are preserved. All ten owners are below 500 lines.
Strict 2173/11/20/39; all 31 above700 hashes/counts unchanged. Clearance 114/145
(78.6%). Union 1878, unchanged neighbors 1868, assets 22.

Next: DB hydration/parsed-memory/path budgets, successful folder synchronization,
native deadlines and complete application drain/status ordering. Deletion remains
non-durable and can make partial progress on I/O failure or process/runtime exit.
Pool fairness, cross-process/file races and full provider/UI/Docker/release
acceptance remain open. S06 remains reserved and the overall goal is active.

Evidence: target/effective-line-evidence/20260916-provider-management-deletion/.
Scope SHA-256: 1aec7f90e2381c85681ca1398ee4224c03ff0256e5c9cdb348d652e407f9d38d.
[Report](../../status/2026-09-16-provider-management-deletion.md).
