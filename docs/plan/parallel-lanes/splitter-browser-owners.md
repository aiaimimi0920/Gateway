# Splitter and browser-executor runtime ownership

Owner: parallel coordinator. State: verified structural checkpoint.
Started: 2026-09-12.
Accepted: 2026-09-12 14:56:02 UTC.

Scope: src/splitter.rs, src/splitter/, src/browser_executor_runtime.rs,
src/browser_executor_runtime/ and the existing Python source-contract path.
The entries decrease from 1,233/1,122 to 198/214 effective lines. All 18 Rust
owners are at most 266, with unchanged public entry paths. The Python source
contract measures 75 lines and changes only its read of the lifecycle owner.

Keep shared private worker records and state/lease semantics at the splitter
entry. Separate service startup, worker readiness/replacement, process supervision,
draining, management endpoints, HTTP forwarding and WebSocket forwarding. Split
only the qualified SplitterManager implementation; preserve the other complete
implementations and original tests, including production items after the tests.

Keep browser-executor public contracts at their current namespace. Separate node
heartbeat/listing, slot state, lease state, health aggregation, Redis persistence
and normalization. Preserve the health trait and its two implementations as whole
blocks. Keep all original test bodies and complete module paths.

Capture exact pre-edit sources, public paths, original tests and neighboring
inputs. Run paired focused unit tests, the two splitter state integration tests
and the five existing Python access/reliability contracts. Update only the Python
source read to the real lifecycle owner; preserve its cleanup assertions. Require
fresh all-targets, scoped formatter, checker 19/19, ratchet, strict inventory,
source/neighbor/encoding proof and both Git checks.

Behavior changes remain separate: browser lease compare/delete and slot ownership,
unbounded time arithmetic, Redis/index/result bounds and splitter lifecycle limits.
The opt-in process/Redis splitter E2E requires a current-source executable and
explicitly isolated runtime settings before it can serve as acceptance evidence.
S06 ownership and pending GWP-20260912-01 freeze/build coordination are unchanged.

Evidence: target/effective-line-evidence/20260912-splitter-browser-owners/.

Exact comparison preserves 102 production items, 25 public entry paths, five
original unit tests, two fixtures and 205 neighboring inputs. The 22 manager
methods use qualified identities; the five other implementation blocks and
browser health trait remain intact. Thirty-three helpers/methods gain only
family-local pub(super) visibility. All private record fields remain unchanged.

Paired splitter 2/2, browser 3/3, public splitter-state 2/2 and Python contracts
5/5 pass. Fresh all-targets, scoped formatter, checker 19/19, ratchet, source/
encoding proof and both Git checks pass. Strict scans 1,561 files: 31 hard,
31 mandatory and 40 soft; 62 remain above 700. Clearance is 83/145 (57.2%).
The separate current-source splitter process/Redis E2E passed 1/1 at 2026-09-12
15:08:33 UTC. Worker cutover, in-flight draining, failed readiness cleanup, crash
supervision, recovery and final port/container cleanup are verified. The existing
4200 container identity/start time are unchanged; no gateway.exe process or 4226
listener remained in the subsequent host check. This is a debug runtime gate.
[Acceptance report](../../status/2026-09-12-splitter-browser-owners.md).
