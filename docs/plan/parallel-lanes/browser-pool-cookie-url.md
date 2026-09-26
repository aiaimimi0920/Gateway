# Browser-pool cookie URL compatibility

Owner: resumed coordinator. Accepted at 2026-09-14 02:59:35.208 UTC.
Move path initialization into the embedded-cookie normalizer's domain branch.
Keep URL precedence, domain defaults and all remaining entry code unchanged.
Entry remains 9,382 effective lines; new real-browser regression file has 37.

Accepted baseline 86/88 with two URL/path errors; final 88/88. Both commands
serialize test files. Initial 79/88 baseline is retained and rejected because
seven app-navigation setup failures also occurred after Edge launch timeout.
Use before.json and baseline-attempt2 receipts for the accepted pairing.
Package 1/1, source/syntax/checker 19/19/ratchet and separate Git checks pass.

Evidence: target/effective-line-evidence/20260914-browser-pool-cookie-url/scope.json.
SHA-256: e73994efe770872f267db2f90c8bd62edfd9fbf1fd86b8bb29e6c2976da000bb.
Predecessor: 8121919a38fa798ca002ee5efe457daf19cfc94fa1a33ab7528becadb6587150.
Union 713; 712 unchanged neighbors. Strict: 1,870 scanned; 16 hard, 24 mandatory,
40 soft; 40 above 700. Clearance 105/145 (72.4%); S18/release remain open.
[Checkpoint report](../../status/2026-09-14-browser-pool-cookie-url.md).

Next: pure cookie/storage-state ownership extraction with a fresh baseline.
S06 scope/cursor and final build transfer remain reserved under GWP-20260912-01.
