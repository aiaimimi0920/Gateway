# Stream state, arithmetic and terminal cleanup

Owner: parallel coordinator. State: `accepted`.
Started: 2026-09-12.

Scope is the existing tracked-stream entry, archive and usage leaves, plus
`tests/stream_observation_contract.rs`. Original production counts are 106/94/243;
the new public contract is 219 effective lines. The accepted structural snapshot
is immutable; this hardening uses `target/effective-line-evidence/20260912-stream-state/`.

Reproduce unchecked usage arithmetic, false archive truncation on empty chunks,
and retained upstream state after terminal completion. The same 13-test contract
must run before and after production changes. It covers both usage wire formats,
explicit totals, normal cache metadata, real/empty archive overflow, UTF-8 snapshot
behavior, clean/error/drop cleanup, terminal polling and nested tap snapshots.

Preserve all other public interfaces, 14 original stream tests, HTTP SSE caller
behavior, forwarded bytes before termination, metrics and exactly-once completion.
Use the established reduced feature set for paired tests and a separate fresh
default-feature all-targets gate. Snapshot all 98 existing source/input files and
the complete regression source before editing production. All four scoped owners
must remain below 500 effective lines.

The fixed public contract reproduced four passes and nine failures before any
production change. An initial harness compile used the package name as its crate
import; the two imports were corrected to the declared `neuro_gateway` library.
The failed compile log and initial contract are retained. The v2 contract is the
unchanged source for the actual baseline/final pair. Original reduced-feature
stream 14/14 and HTTP SSE 3/3 baselines also pass.

Production corrections are implemented. Usage totals use saturating addition and
the already-shadowed duplicate output-token branch is removed. Empty archive
chunks leave truncation state unchanged. Tracked streams release their optional
pinned upstream before exactly-once completion, then stop polling after EOF/error.
This intentionally makes released upstream state observable inside callbacks.

Production owners are 109/97/217 effective lines; the contract remains 219. Exact
proof preserves 19 function signatures, 14 other bodies, eight other structs,
module/import declarations and 95 neighbors. Only five function bodies and the
private tracker field change. Final regressions pass 13/13; original stream 14/14
and HTTP SSE 3/3 pass with the same reduced feature set. Fresh default-feature
all-targets, scoped formatter, checker 19/19, ratchet, encoding and both Git checks
pass. All native gates are terminal. No compiler warning names a scoped owner.

Immutable scope: `target/effective-line-evidence/20260912-stream-state/scope.json`,
captured at 2026-09-12 01:15:12 UTC. Strict scans 1,491 files: 31 hard, 41 mandatory
and 40 soft; 72 remain above 700. Clearance stays 73/145 (50.3%). Global formatting
still reports only the unchanged S06 runtime-mirror files. The preceding extraction
scope remains immutable. [Acceptance report](../../status/2026-09-12-stream-state.md).

This scope does not yet address SSE observation-buffer admission or CPU work per
poll. S06 ownership/cursor, the pending transfer request, runtime profiles, line
policy/baseline and the release root remain unchanged. No release build is part
of these gates.
