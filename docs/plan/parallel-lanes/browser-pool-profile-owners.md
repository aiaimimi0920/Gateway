# Browser-pool profile clone ownership

Owner: resumed coordinator. State: accepted at 2026-09-14 01:06:37.583 UTC.
Entry 10,147 -> 10,084; owner 72, fixture 61, new contracts 82 and package
contract 159 effective lines. Paired Node suites pass 55/55 and package tests 1/1.
Source/syntax/checker/ratchet and both Git checks pass. All gates are terminal and
storage fixture census is zero. The 702-input union preserves 699 neighbors/web
assets. Strict: 1,859 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700,
clearance unchanged at 105/145 (72.4%). S18 and release remain open.

Evidence: target/effective-line-evidence/20260914-browser-pool-profile-owners/scope.json.
SHA-256: 6d3d8dd5ca7935425d93eaa0648ce4875a65c8a0ae8b73213d2cc74502cf2add.
[Checkpoint report](../../status/2026-09-14-browser-pool-profile-owners.md).

Accepted preparation/design:
Predecessor scope: target/effective-line-evidence/20260914-browser-pool-runtime-state-owners/scope.json.
SHA-256: 4c9e5c58e8c4267671f733de8431565f0adbf0132b9d5926cdc9c46c2be10f23.

Move the six launch-profile path/copy/clone/removal/cleanup functions to adjacent
gemini-canvas-browser-pool-profile.mjs. A createProfileCloneOwner(log) factory
captures the root's existing logger and returns cloneProfileDirectory,
removeManagedLaunchProfileClone and cleanupClonedLaunchProfile. One root-level
factory invocation preserves all existing call signatures and logger behavior;
there is no mutable module state or startup I/O. The factory owns no browser,
context or page and introduces no new cleanup policy.

Preserve bodies with indentation only, recursion and best-effort file copy,
source-path hashing/timestamp naming, cleanup flag-before-await ordering and
delegation to the existing managed-root removal guard. Keep real launch/retry,
failure handling and context disposal call sites in the root unchanged. Only
copyFileSync/readdirSync and removeLaunchProfileClonePath imports leave the root;
other fs/crypto imports still have callers and must remain.

Before source edits, expose the three root-private entry points in the existing
fixture. Add real filesystem contracts for nested copying, skipped junctions,
locked-file copy errors, source preservation, cleanup ordering/idempotency,
outside/root deletion refusal and missing-source propagation. Mock only the
single copy-error boundary; restore builtin bindings and logging after the test.
Use contained temporary roots and the accepted storage environment fixture.

Run paired complete browser-pool and package contracts plus source/encoding/syntax,
checker, ratchet, strict and both Git checks. All new owners must be at most 500
effective lines. S18 remains incremental; S06/final build transfer stays reserved
under GWP-20260912-01. No browser/provider/live service or release is launched.
