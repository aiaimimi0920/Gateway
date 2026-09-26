# Browser-pool connected-client protocol checkpoint

Accepted at 2026-09-14T07:04:54.125Z. Entry 7029 -> 6829 effective lines;
owner 211, fixture 106, test 224, package contract 178.

Twelve functions and three state declarations move with indentation only. The
factory retains root SCRIPT_DIR and original map initialization; root HTTP/HTTPS
WebSocket registration and listener bytes are preserved. Fourteen new contracts
cover authentication, dispatch, response/error/timeout/disconnect and bootstrap.
A synthetic VM executes the real browser client through auth, fetch and disconnect.
The first passing baseline is excluded because bootstrap JS was not input-bound;
accepted retry1 binds its bytes and checks its package record before and after.
Cross-client response attribution and send-failure timer retention are reproduced
as existing defects, to be fixed separately. No behavior correction is included.

Paired serialized Node 262/262, identical identities/warnings, no skips; package
1/1. Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged runtime claim.

Evidence: target/effective-line-evidence/20260914-browser-pool-connected-client-owners-retry1/scope.json.
SHA-256: 88740cae409ab1d2c4fa04b6275cc73919e48f4fb9e77a43be0f30b366eac01e.
Union 749; unchanged neighbors 746; web assets preserved. Strict: 1905 scanned;
16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance 105/145 (72.4%).

Next: repair response ownership/authentication checks and synchronous-send cleanup
with failing regressions, then preview frame discovery and page lease ownership.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
