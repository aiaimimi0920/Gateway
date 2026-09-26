# Folder synchronization status conditional commits

S09 stale whole-status overwrites are accepted as repaired on 2026-09-16. All
three writers reapply owned metadata to the latest typed JSON and use single-key
Lua presence/raw-byte CAS. Eight total attempts bound contention; exhaustion
returns 409 without a blind write. Rust retains exact integer serialization.
Business I/O executes once; phase flags and deletion events update current status.

Staged GET/SET baseline: 3 passed / 3 failed. Frozen candidate real Redis: 6/6.
Only store.rs changes between those runs; callers, both test owners and fixture
remain byte-identical. Present/missing conflicts, audit retention, bounded failure,
large integers, malformed state and initialization pass. Library 63/63 plus six
separately executed ignored tests; original 60 identities remain. Startup/runtime
6/6 also passes. All three fixture containers/directories are removed; 45 existing
containers retain identity/state. No PostgreSQL synchronization claim.

All-targets, scoped fmt, root/status reconstruction, checker 19/19, ratchet and
both Git checks pass. Root 5553 -> 5543; status 227 -> 190; new owners
53/78/141/57/90. Strict 2117/12/20/39; 32 above 700; clearance 113/145 (77.9%).
Union 1822; unchanged neighbors 1815; unchanged assets 22. No dependency/policy/
baseline, profile, persistent service, staging, commit or release changes.

Next: concurrent enabled override/memory ordering, rapid toggles, queued filesystem
events and backend shutdown; then remaining root and full acceptance. Same-field
priority, operation cancellation and durable database/audit transactions remain open.
S06 source/cursor/final build stays reserved under GWP-20260912-01.
Evidence: target/effective-line-evidence/20260916-folder-sync-status-cas/.
Scope SHA-256: ef02f6d2be86277ba6b38e1857174b0594f1460f751da1300d65f71c0c9ece93.
[Report](../../status/2026-09-16-folder-sync-status-cas.md).
