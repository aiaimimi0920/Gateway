# Browser-pool cookie restoration ownership

Owner: resumed coordinator. Accepted at 2026-09-14 03:24:08.657 UTC.
Entry 9,382 -> 9,143; new owner 247, fixture 71, new tests 86/43 and package
contract 162 effective lines. Paired Node suites pass 99/99 and package tests 1/1.
Source/syntax/checker 19/19/ratchet and both Git checks pass; all gates terminal.

Move eight header parsing/sync/auth/embedded-storage functions intact except
indentation into createCookieOwner({ log }). Initialize before the app factory;
retain parser binding for fetch auth, root exports and caller lifecycle ownership.
The URL-cookie compatibility fix is already part of the baseline. Keep existing
fallback/error/allowlist semantics and do not add direct public helper exports.

Eleven pre-extraction contracts include three real-browser tests. Preserve source
bytes on success/failure, sanitize logs, use offline/local/synthetic fixtures and
clean up temporary roots, contexts, browsers and mocks. Both suites are serialized.

Evidence: target/effective-line-evidence/20260914-browser-pool-cookie-owners/scope.json.
SHA-256: 9eb04a18ef267cdf3aa459854c4262951716246cddd9818ffee82182da09c7ae.
Predecessor: e73994efe770872f267db2f90c8bd62edfd9fbf1fd86b8bb29e6c2976da000bb.
Union 716; unchanged neighbors 713. Strict: 1,873 scanned; 16 hard, 24 mandatory,
40 soft; 40 above 700. Clearance 105/145 (72.4%). S18/release remain open.
[Checkpoint report](../../status/2026-09-14-browser-pool-cookie-owners.md).

Next: program/share navigation; preserve caller-owned page adoption/capture/close.
S06 scope/cursor and final build transfer remain reserved under GWP-20260912-01.
