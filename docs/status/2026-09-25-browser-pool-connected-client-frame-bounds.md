# Browser-pool connected-client WebSocket frame bounds

Status: **hardening_green for connected-client frame admission**, 2026-09-25 UTC.
Unknown-length native/CDP response reads, aggregate process memory, S06/S18, and
release/runtime acceptance remain open.

## Admission boundary

The shared body owner allows 4 MiB of UTF-8 text and derives a 25 MiB frame cap:
six times the text budget covers worst-case JSON escaping for one-byte control
characters, with 1 MiB reserved for the message envelope. The connected-client
handler counts a string or Buffer before `JSON.parse` and closes an application-
level oversized message with code 1009. The browser-pool `WebSocketServer` uses
the same cap as `maxPayload`, so the `ws` receiver also rejects oversized frames
before delivering a `message` event. This bounds one accepted frame; it does not
establish a global in-flight or process-heap budget.

The regression suite verifies that a 4 MiB NUL body survives worst-case JSON
escaping, that the handler rejects an over-limit Buffer without invoking
`JSON.parse`, and that a real `ws@8.21.0` loopback server with a 1-byte test cap
closes a 2-byte client frame with code 1009. The loopback case wires the server
close event to connected-client cleanup and checks that the pending request,
connection entry, and deadline are released. The server unit test separately
checks that production startup passes the shared 25 MiB constant to
`WebSocketServer`.

## Verification

Node 22.22.2 focused tests pass **46/46** across the connected-client, server,
and body suites. `node --check` passes for the three owners and three related
test/fixture files. Effective-line checker tests pass **31/31**; the fresh
ratchet scans **2,502** files, classifies 14 immutable runtime artifacts, and
reports no governed source above 700 lines. The nested-worker package contract
passes **1/1**, and the Neuro development-standard contract passes. Gateway and
Neuro `git diff --check` both exit 0. The current effective-line scan reports
136 lines for the shared body owner, 277 for the connected-client owner, 161 for
the server owner, 389 for the connected-client suite, 90 for server fixtures,
and 116 for server tests; all remain below 500.

The application-level check narrows the prior parse-time allocation risk, and
`maxPayload` limits each WebSocket frame. Concurrent pending responses and other
CDP body readers remain separate memory boundaries. See also the historical
[response retention record](2026-09-25-browser-pool-connected-client-retention.md).
