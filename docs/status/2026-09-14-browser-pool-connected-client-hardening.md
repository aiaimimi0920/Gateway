# Browser-pool connected-client hardening checkpoint

Accepted at 2026-09-14T07:13:33.693Z. Entry 6829 -> 6829 effective lines;
owner 217, fixture 106, test 240, package contract 178.

Two focused regressions fail on the accepted extracted source and pass after the
fix. Responses require the currently authenticated owning connection; unsigned,
foreign and revoked senders cannot alter headers/body or settle another request.
Synchronous send or JSON serialization failure clears the timer and pending entry
while preserving the original error. Owner grows 211 -> 217 lines; root, bootstrap
and server registration remain unchanged. Read-only review found no regression.

Focused regressions 0/2 before -> 2/2 after; full Node 262/262, no skips; package
1/1. Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged runtime claim.

Evidence: target/effective-line-evidence/20260914-browser-pool-connected-client-hardening/scope.json.
SHA-256: 1bdde55d501bf3daccd6bcd8fa6ff1815a88232305d7632bbaf791e520153929.
Union 749; unchanged neighbors 748; web assets preserved. Strict: 1905 scanned;
16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance 105/145 (72.4%).

Next: preview frame discovery and page lease ownership, plus the separately
reproduced preview probe timer cleanup. Other protocol lifecycle leads stay open.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.
