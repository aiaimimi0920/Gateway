# Realtime HTTP bridge ownership

Owner: parallel coordinator. State: `structurally_verified`.
Started: 2026-09-11. Verified: 2026-09-12.

Scope is `src/http/routes/realtime.rs` (737 effective lines), its private
`realtime/` directory and `tests/realtime_websocket_contract.rs`. This is the
remaining HTTP file above 700 at the preceding 1,399-file inventory. S06,
protocol normalizers, shared SSE bridge, pipeline implementations and releases
remain outside this scope.

Keep upgrade, socket event dispatch, bearer parsing and frame writes in the
entry. A session owner holds canonical request construction, tool parsing and
conversation-item normalization. A response owner borrows the same session and
socket for pipeline execution, SSE deltas, tool completion and final events.
Preserve whole bodies, ordering, allocations, public paths and failure behavior.
Restricted visibility must stay within the Realtime module family.

Before production moves, retain the existing tool-shape test and add focused
session characterization plus isolated loopback WebSocket control/error tests.
Run the same contracts after extraction, all-targets compilation, scoped/global
formatter checks, checker tests, ratchet, strict inventory and independent Git
checks. Preserve the original source and full comparison evidence under
`target/effective-line-evidence/20260911-http-realtime/`.

All new and completed owners must remain below 500 effective lines. Review
existing session/frame bounds and streaming lifetime separately from extraction;
do not claim real-provider streaming or global hardening from control-frame tests.
Integrated release still needs the GWP-20260908-06 freeze/build transfer receipt.

The entry is now 212 effective lines, with session 218 and response 308. All six
scoped source/test files are at most 308. Complete source comparison preserves
four moved items (including the session implementation), five retained functions
and the existing test. Fields/methods become visible only inside the Realtime
module family. Paired unit 5/5 and loopback WebSocket 2/2 tests pass, plus router
layer 3/3, all-targets compilation, scoped formatter, checker 19/19 and ratchet.

Strict now has 84 files above 700 and none under `src/http/`. Existing session
history/frame budgets and stalled-upstream cancellation remain hardening work;
this extraction does not add caps or claim native-provider streaming acceptance.
See [Realtime checkpoint](../../status/2026-09-12-http-realtime.md).
