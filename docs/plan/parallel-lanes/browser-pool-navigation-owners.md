# Browser-pool program/share navigation ownership

Owner: resumed coordinator. Accepted at 2026-09-14 03:40:55.040 UTC.
Entry 9,143 -> 8,791; owner 359, fixture 72, new tests 61/42 and package contract
163 effective lines. Paired Node suites pass 113/113 and package tests 1/1.
Source/syntax/checker 19/19/ratchet and both Git checks pass; all gates terminal.

Move resolveProgramPageUrl, ensureProgramPage, ensureSharePage, waitForShareSurface
and tryFollowShareEntryPoint into one owner. Preserve bodies except indentation.
Capture log/collectButtonSnapshot; directly import app/input helpers. Keep root
bindings, exports and caller-owned page adoption/close/network capture unchanged.

Fourteen new pre-extraction contracts include twelve real-browser cases. Exercise
navigation/draft/account/auth, popup and same-page returns plus failure diagnostics
using local offline fixtures. Preserve original page ownership and close the popup;
context teardown also runs on failures. These do not prove higher-level adoption,
provider redirects or production latency. Both complete Node suites are serialized.

Evidence: target/effective-line-evidence/20260914-browser-pool-navigation-owners/scope.json.
SHA-256: 711f13242b2266a526b9a0fd1d5440569bb09f6947a7083a802b254bf685c946.
Predecessor: 9eb04a18ef267cdf3aa459854c4262951716246cddd9818ffee82182da09c7ae.
Union 719; unchanged neighbors 716. Strict: 1,876 scanned; 16 hard, 24 mandatory,
40 soft; 40 above 700. Clearance 105/145 (72.4%); S18/release remain open.
[Checkpoint report](../../status/2026-09-14-browser-pool-navigation-owners.md).

Next: measure transport-hint and action/UI parsing boundaries; inspect all tests,
including untracked files, before selecting scope. S06 scope/cursor and final
build transfer remain reserved under GWP-20260912-01.
