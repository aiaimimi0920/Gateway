# Web publisher lock and cleanup ownership

Date: 2026-09-21. State: local_gates_green; independent_review_pending.

Scope: apps/desktop/tools/publish-web-dist.mjs (613 effective), new sibling
web-publish-lock.mjs and web-publish-cleanup.mjs, plus the existing Python
cleanup source contract. Preserve the Node ESM CLI/public entrypoint and every
function body. Lock owns acquisition, owner snapshots/recovery and release;
cleanup owns the existing bounded remove retry and sleep. Transaction/rollback,
file digests, ready signalling and CLI remain in the publisher.

Keep snapshot-before-lock ordering, live mutation under lock, token-checked
release, ownerless nonrecursive recovery, dead-owner checks, grace period,
retry codes/backoff and error aggregation unchanged. Retarget the existing
source contract to the implementation owner without claiming new retry fault
injection coverage. The .mjs siblings preserve the existing direct Node runtime.

Run paired Python standalone web readiness/transaction contracts, Node syntax,
desktop typecheck and real local build:web (rsbuild publisher integration),
exact source projection, checker/ratchet/strict, encoding and separate Git
checks. Docker copies apps/desktop as a directory and Cargo watches tools as a
directory; no individual-script packaging copy was found. No live deployment,
release package, profile or reserved S06 operation is authorized by this lane.

Evidence: target/effective-line-evidence/20260921-web-publisher-owners/.
All resulting source owners must be <=500; full-goal acceptance remains open.

Result: 407/180/30 effective lines; paired 13/13 Python contracts, syntax,
typecheck, web build and exact projection pass. Independent reviewer service
authentication failed; review is not claimed. See
`docs/status/2026-09-21-web-publisher-owners.md`.
