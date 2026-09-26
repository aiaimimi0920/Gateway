# Realtime HTTP bridge extraction

Started: 2026-09-11. Verified: 2026-09-12, Asia/Shanghai.
Owner: parallel coordinator. State: `structurally_verified`.
Scope: [Realtime HTTP lane](../plan/parallel-lanes/http-realtime.md).

## Source result

`src/http/routes/realtime.rs` decreased from 737 to 212 effective lines. It keeps
the WebSocket upgrade, socket event loop, bearer parsing and frame writes. The
session owner holds canonical request construction and item/tool normalization;
the response owner borrows that session and socket for pipeline execution, SSE
delta handling and response completion.

| Scoped source/test file | Effective lines |
| --- | ---: |
| `src/http/routes/realtime.rs` | 212 |
| `realtime/session.rs` | 218 |
| `realtime/response.rs` | 308 |
| `realtime/tests.rs` | 27 |
| `realtime/session_contract_tests.rs` | 106 |
| `tests/realtime_websocket_contract.rs` | 156 |

All six files are at most 308. No exception or baseline regeneration was needed.
Counts, hashes, original source hash, comparison proof and independent Git states
are captured in
[`scope.json`](../../target/effective-line-evidence/20260911-http-realtime/scope.json).
The evidence directory retains the exact `realtime.before.rs` snapshot and
`extract-realtime.mjs --verify` for the complete item comparison.

## Preservation and review

Four moved production items, including the full session implementation, preserve
their bodies. Five entry functions and the existing tool-shape test are unchanged.
Required field/method visibility is limited to `pub(super)` within the Realtime
module family. Public handler paths, signatures, query/header propagation and
the shared SSE bridge are unchanged.

The comparison retains event ordering, tool argument accumulation, model and
instruction precedence, history cloning, original awaited operations, error
messages, final usage/finish mapping and session updates. No new task, lock,
channel, client, buffer, timeout or allocation was introduced by extraction.
The child response owner continues to borrow the existing session and socket.

The review also records inherited limits rather than treating the split as
hardening completion:

- The upgrade does not select a configured frame budget, and the event loop
  grows session history without an explicit session budget.
- Response text and pending tool collection retain the existing accumulation
  behavior. Their complete resource bounds were not established by this batch.
- The response loop still awaits upstream chunks inline. These tests do not
  establish prompt cancellation when the client disconnects during a stalled
  upstream read.

These items remain owned by a subsequent Realtime hardening batch. Control-frame
tests do not establish live-provider streaming or native audio support.

## Fresh verification

| Gate | Observed result |
| --- | --- |
| Original implementation plus session characterization | 5 unit tests passed |
| Original loopback WebSocket controls/errors | 2 tests passed |
| Extracted session/entry implementation | Same 5 unit tests passed |
| Extracted loopback WebSocket controls/errors | Same 2 tests passed |
| Router layer contracts | 3 passed |
| `cargo check --offline --locked --all-targets` | Passed |
| Scoped official Rust formatter | Passed |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | Passed |
| UTF-8 without BOM and scoped whitespace checks | Passed |
| Gateway and Neuro `git diff --check` | Passed independently |

Characterization covers instruction fallback, response overrides without session
mutation, omitted versus empty tool overrides, text ordering and structured tool
outputs. Loopback tests cover upgrade, session identity/model updates, item echo,
JSON ping, WebSocket Ping/Pong and recovery after malformed, binary, audio and
unsupported events. Fixtures use port 0, close the client and abort/join their
listener task, with drop cleanup on assertion failures. They do not invoke Redis,
PostgreSQL or an upstream provider.

Logs remain under `target/effective-line-evidence/20260911-http-realtime/`, including
`baseline-unit.log`, `baseline-websocket.log`, `final-unit.log`, `final-websocket.log`,
`final-all-targets.log`, formatter/checker/ratchet logs and `strict-inventory.json`.
Global formatting still reports only the reserved S06 runtime-mirror source and
test files. No owned-file compilation or formatting failure remains.

## Remaining plan and release boundary

Strict scans 1,404 files: 31 hard, 53 mandatory and 40 soft. There are 84 files
above 700 and none under `src/http/`. Structural clearance is 61/145 (42.1%).
Strict remains red for debt outside that HTTP threshold result.

No release was built or modified. S06 retains its source and original plan cursor;
GWP-20260908-06 still needs the explicit source/docs freeze and shared-build
transfer receipt. Remaining structural work, Realtime hardening and integrated
runtime/UI/Docker acceptance remain open.
