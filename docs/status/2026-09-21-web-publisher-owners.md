# Web publisher lock and cleanup extraction

Date: 2026-09-21. Status: local_gates_green; independent review pending.
The full optimization and release goal remains active.

## Structure and contracts

| File under `apps/desktop/tools/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `publish-web-dist.mjs` | 407 (was 613) | Snapshot, file publication, digest/marker verification, rollback, cleanup aggregation and CLI |
| `web-publish-lock.mjs` | 180 | Lock owner snapshots, ownerless/dead-owner recovery, acquisition and token-checked release |
| `web-publish-cleanup.mjs` | 30 | Existing bounded filesystem removal retry and timer delay |

All original function bodies are preserved exactly. Only owner-specific imports,
constants and exports move. The Node ESM extension preserves direct execution
and rsbuild import without introducing a TypeScript runtime loader. The public
publishWebDist entrypoint, declaration file, marker schema and CLI stay unchanged.

The dependency direction is publisher -> lock -> cleanup, with publisher also
using cleanup for file replacement, rollback and snapshot removal. Cleanup does
not import the transaction owner. Snapshot copying remains before lock acquisition;
live mutation, marker invalidation and commit remain inside the lock interval.
Release and snapshot-cleanup failures still enter the same aggregate error path.

Lock behavior retains the 60-second timeout, 50-ms retry, 10-second incomplete
owner grace, PID checks, owner-token checks and nonrecursive ownerless recovery.
Cleanup retains five attempts, linear 100-ms backoff and its exact retry-code
set. No new deletion target, retry, task, timer lifecycle or process is added.
Existing filesystem race/symlink/path-trust assumptions are not hardened here.

## Evidence and validation

Evidence: `target/effective-line-evidence/20260921-web-publisher-owners/`.
Saved baseline and extraction-proof.json retain hashes, effective counts,
UTF-8/no-BOM and exact parent/child source projections.

| Check | Result |
| --- | --- |
| Before/after `python -m unittest discover -s tests/python -p 'test_gateway_standalone_web_*.py' -v` | 13 passed / 13 passed |
| `node --check` for all three publisher owners | Passed |
| `npm run typecheck --prefix apps/desktop` | Passed |
| `npm run build:web --prefix apps/desktop` | Passed; actual rsbuild publisher integration |
| Exact source projection and UTF-8 without BOM | Passed |
| Effective-line checker tests / ratchet | 19 passed / passed |
| Separate Gateway and Neuro diff checks | Passed |

The Python contracts execute real Node subprocesses in temporary directories
and cover concurrent publishers, pruning, marker digest/invalidated readiness,
rollback, CLI and dead/incomplete-owner recovery. The existing cleanup source
contract now checks constants/body in the cleanup owner and its import/use in
the publisher. It remains a source contract, not injected transient-I/O failure
coverage. No existing test assertion was dropped.

The main thread reviewed the exact source projections and lifecycle wiring.
The independent reviewer could not run: its service returned 401 authentication
failure before review. This limitation is retained as a final-audit follow-up;
no independent approval or review success is claimed. The earlier read-only
scout confirmed callers/package shape: rsbuild imports the same entrypoint,
Docker copies the desktop directory, and Cargo watches the tools directory.
No standalone copy of only the old script was found.

Strict reports 2243 scanned, 10 hard, 18 mandatory and 28 soft entries; exit 1.
This clears one soft owner (29 -> 28), leaving 28 files above 700. No checker
policy, baseline, exception or runtime browser profile changed. There is no
desktop formatter script; original function formatting is retained.

## Whole-goal continuation

Next desktop source candidate remains ModelPoolWorkspace.tsx (624). Existing
scout evidence separates workspace state/focus/menu lifetime from model-card
rendering; exact boundaries need firsthand reading and size measurement before
editing. Independent review of this publisher extraction also remains pending.

The Gateway dirty worktree and Neuro Gateway submodule state are preserved.
No sibling source, staging, commit, push or live service was changed. Web build
output is local; no packaged release was placed in the user release directory.
S06 source/build transfer, runtime-artifact governance, full integrated gates
and final release remain open in the complete refactor audit.
