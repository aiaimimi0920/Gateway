# Credential automation HTTP response bounds

Owner: parallel coordinator. State: hardening_green. Started: 2026-09-12 UTC.
Accepted: 2026-09-12 21:35:10 UTC.

Scope: src/credential_pool_automation/driver.rs and a frozen real-loopback HTTP
contract under its private test directory. The pre-fix driver enforced its
2 MiB response limit only after complete accumulation. Reproduce delayed rejection
for an oversized declared length and an oversized chunked response whose ending
is withheld before changing production behavior.

Retain the 2 MiB budget, request JSON, authentication/header construction, HTTP
status admission, timeout, JSON parsing, error messages and both unrelated driver
functions. Reject oversized advertised lengths before reading and enforce the
same byte budget while streaming. Preserve exact-limit success, ordinary chunked
responses, invalid JSON, finite excess, non-success status and truncated-body
diagnostics. Script I/O lifetime and bounds remain a separate scope.

Require frozen before/after runtime assertions, paired original automation units,
fresh all-targets, scoped formatter, checker 19/19, ratchet/strict inventory, exact
source/neighbor/encoding proof and separate Git observations. Loopback fixtures
must cap request reads, own one bounded socket task, close it before assertions
and avoid credentials or external services.

S06 retains its implementation/cursor. GWP-20260912-01, runtime-profile governance
and final freeze/build transfer remain pending. No release, persistent deployment,
dependency, checker-policy, baseline or exception change belongs to this lane.

Evidence: target/effective-line-evidence/20260912-automation-http-bounds/.

The frozen contract reproduced two response-budget failures while the server
withheld completion. It advances from six passes/two failures to 8/8 after
declared-length admission and bounded chunk accumulation. Three signatures, both
unrelated driver bodies, the 2 MiB constant and 295 neighboring inputs remain
unchanged. Removing the reviewed read change, import and test wiring reconstructs
the entire original owner. Production/contract/fixture owners are 150/110/179
effective lines; both test files remain byte-identical to the red baseline.

Original automation units pass 10/10 in both phases. Fresh all-targets, scoped
formatter, checker 19/19, ratchet, source/encoding proof and both Git checks pass.
All Cargo gates are terminal. Strict scans 1,609 files: 29 hard, 29 mandatory and
40 soft; 58 remain above 700, with clearance 87/145 (60.0%). Global formatting
still reports only the unchanged S06 runtime-mirror files.

The accepted scope binds snapshots, paired runtime cases/results, compiler/checker
logs, inventories and separate repository observations. See the
[acceptance report](../../status/2026-09-12-automation-http-bounds.md).
Desktop native process/profile ownership is the next structural review.
