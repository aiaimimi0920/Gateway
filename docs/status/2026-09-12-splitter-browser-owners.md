# Splitter and browser-executor runtime ownership

Accepted structural checkpoint: 2026-09-12 14:56:02 UTC.
Owner: parallel coordinator. The entire Gateway plan remains in progress.

The splitter entry decreases from 1,233 to 198 effective lines, and browser
executor runtime from 1,122 to 214. All 18 Rust owners remain at most 266.

## Accepted owners

| File | Effective lines |
| --- | ---: |
| src/splitter.rs | 198 |
| splitter/service.rs | 109 |
| splitter/lifecycle.rs | 256 |
| splitter/supervisor.rs | 71 |
| splitter/draining.rs | 130 |
| splitter/management.rs | 144 |
| splitter/http_proxy.rs | 176 |
| splitter/websocket_proxy.rs | 177 |
| splitter/tests.rs | 47 |
| src/browser_executor_runtime.rs | 214 |
| browser_executor_runtime/nodes.rs | 65 |
| browser_executor_runtime/slots.rs | 186 |
| browser_executor_runtime/leases.rs | 260 |
| browser_executor_runtime/health.rs | 266 |
| browser_executor_runtime/storage.rs | 103 |
| browser_executor_runtime/normalization.rs | 34 |
| browser_executor_runtime/timestamps.rs | 12 |
| browser_executor_runtime/tests.rs | 56 |
| tests/python/test_gateway_phase3_access_reliability_contract.py | 75 |

## Preservation and review

Exact comparison preserves 102 production items: 46 free functions, 22 qualified
SplitterManager methods, 22 structs/enums, six constants, five complete other
implementation blocks and the browser health trait. Twenty-five public entry
paths remain intact. All five original unit tests, two fixtures and their full
module paths are retained. Thirty-three private helpers/methods gain only the
family-local pub(super) visibility required by actual callers and original tests.

Shared worker records remain private at the splitter entry. Admission and Drop
lease accounting, readiness identity checks, reload serialization, drain deadlines,
forced process cleanup, crash supervision, management authentication and HTTP/
WebSocket forwarding retain their original bodies and ordering. Production items
after the old inline tests are preserved. The Python contract now reads the real
lifecycle owner and retains both unready-worker cleanup assertions exactly.

Browser node/slot/lease JSON contracts, Redis key construction, pipeline ordering,
TTL values, sorting/filtering, health aggregation and Chinese errors are unchanged.
The health trait implementations remain whole, avoiding ambiguous repeated method
names. No field visibility, dependency, blocking path or resource lifetime changes.
All 205 neighboring inputs are byte-identical, including callers, shared wiring,
dependencies, the original process E2E and checker policy.

Separate review findings remain: non-atomic lease release and slot ownership,
unbounded timestamp/TTL arithmetic, Redis/index/result bounds and splitter
lifecycle limits. This structural batch does not change those behaviors.

## Verification and remaining work

Paired default-feature splitter 2/2, browser 3/3, public splitter-state 2/2 and
Python access/reliability 5/5 pass. Separate all-targets, scoped formatter, checker
19/19, ratchet, exact source/neighbor proof, UTF-8 without BOM and both independent
Git whitespace checks pass. All structural native gates are terminal.

Strict scans 1,561 files: 31 hard, 31 mandatory and 40 soft. There are 62 files
above 700; accepted clearance is 83/145 (57.2%). Strict still exits 1. Global
formatting reports only the two unchanged S06 runtime-mirror files.

The pre-report snapshot records Gateway HEAD
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,715 dirty entries: 172 modified,
one unstaged deletion, two staged deletions and 1,540 untracked. Neuro HEAD remains
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and
182 untracked. Inherited changes and deletions are retained.

Immutable acceptance: target/effective-line-evidence/20260912-splitter-browser-owners/scope.json.
[Lane](../plan/parallel-lanes/splitter-browser-owners.md). Earlier scopes stay immutable.
The separate current-source process/Redis E2E passed 1/1 at 2026-09-12 15:08:33 UTC.
A fresh cargo build --offline --locked --bin gateway succeeded before the test.
The tested executable was target/debug/gateway.exe, 89,877,504 bytes, SHA-256
e8aec0236e7d5c0976f6d012baf834122927817e50944e7be5db56f3d73310a4.

The unchanged E2E verifies replacement cutover, in-flight draining, failed readiness
cleanup, active-worker crash supervision, recovery, closed process ports and Redis
container removal. State/routes are isolated, credential automation is disabled
and the Redis container uses a random loopback port. The live 4200 container ID,
status and start time are unchanged. A subsequent host check found no gateway.exe
process and no 4226 listener. All accepted source hashes remained unchanged.

Runtime evidence: target/effective-line-evidence/20260912-splitter-runtime/scope.json.
This debug-executable gate does not establish packaged runtime, UI or release
acceptance. Both immutable scopes remain separate.

S06 retains its implementation/cursor. GWP-20260912-01 and the explicit final
freeze/build transfer remain pending. No release was built, persistent service
changed or runtime-profile policy migrated.
