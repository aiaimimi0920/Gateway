# Pipeline send ownership

Owner: resumed parallel coordinator. State: accepted at 2026-09-13 23:55:49.886 UTC.
The first candidate passed tests/compile but added an unused pack-owner import.
After native handles terminated, only that import was removed. First-attempt
evidence remains immutable; candidate2/projection2 and attempt2 gates are accepted.

Structural evidence: target/effective-line-evidence/20260914-pipeline-send-owners/.
Projection has 21 owners, entry 326 and maximum 450 effective lines. It preserves
40 helpers, 37 tests/three fixtures, preparation/terminal error precedence and
the stream callback. Candidate-local Next/Stop results preserve continuation
versus termination. Paired groups pass 42/7/26/36/55 with identical normalized
identities/warnings. Independent exact projection review found no concrete
regression. All-targets, scoped fmt, checker 19/19, ratchet and both Git checks
pass. The 674-input union preserves 653 neighbors and published web assets.
Strict: 1,848 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance:
105/145 (72.4%). Global fmt still reports only the two reserved S06 mirror files.
All owned native handles are terminal; owned fixtures are removed.

Accepted scope: target/effective-line-evidence/20260914-pipeline-send-owners/scope.json.
SHA-256: 4ee7494e5c7cbfce0c8cbe3a284add51292f1ac35915a922c4b50013378c2e7f.
[Structural report](../../status/2026-09-14-pipeline-send-owners.md).

Earlier runtime preparation:

Seven populated-candidate HTTP/SSE integration tests pass. New files have
9/60/234/158/125 effective lines. Fresh all-targets, scoped formatter, checker
19/19, ratchet and both diff checks pass. All 649 frozen inputs remain unchanged;
the accepted input union is 654. Runtime fixture census is zero before and after.
Strict: 1,828 scanned, 17 hard, 24 mandatory, 40 soft; 41 above 700. Structural
clearance remains 104/145 (71.7%).

Evidence: target/effective-line-evidence/20260914-pipeline-send-runtime/scope.json.
SHA-256: 684db7bb0d99badc272cba0966b8aac603682e6168431d9d500c02eb430a5620.
[Runtime baseline report](../../status/2026-09-14-pipeline-send-runtime.md).

Predecessor: accepted pipeline-finalize scope, SHA-256
974ee55889a10a00db970bf7bb51f3dc7c86788d0adb3049b8fe38031792bb66.
Stage-send originally had 3,516 effective / 3,792 physical lines. The coordinator
read the complete source. Existing tool_stream children remain intact.
The 37 inline tests mainly cover helper behavior; only the empty-candidate test
calls run, without executing a populated provider candidate.

Preparation scope adds tests/pipeline_send_runtime_contract.rs and private test
owners under tests/pipeline_send_runtime/. Reuse tests/support/mod.rs unchanged.
Freeze production inputs and published web assets. Use ephemeral loopback HTTP/
SSE fixtures, no PostgreSQL, no keepalive, no route policy and no session. Lazy
Redis points to an unused local port. Test servers have explicit abort/join
cleanup; console state uses a uniquely owned temporary directory. No environment
mutation or external service configuration is required.

Prove actual response/model/usage, retry counts and provider fallback, terminal
client errors, deferred streaming feedback, stream cancellation and permit-before-
admission behavior. Do not claim database-backed finalization or provider browser
recovery coverage from these local contracts.

Accepted structural design extracts send ownership by candidate preparation,
stream startup/preflight, translation, completion callback, buffered dispatch,
recovery, metadata, feedback and response packing. Preserve loop continue versus
terminal errors, side-effect order, clone/move counts and bounded collection.
Every new/extracted owner must be at most 500 effective lines. Establish and
review the exact projection before production edits. The entry and all extracted
owners meet the limit and paired gates pass; whole-plan acceptance remains open.

All Cargo/compiler/formatter operations are serialized with idle guards and
GATEWAY_PREBUILT_WEB_UI=1. S06 and final build coordination remain reserved under
GWP-20260912-01. No protocol/upstream/router, dependency, policy, baseline,
exception, live service or release modifications belong to this lane.
