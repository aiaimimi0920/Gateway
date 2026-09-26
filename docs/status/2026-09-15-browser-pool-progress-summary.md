# Browser worker optimization progress, 2026-09-15

Gateway-only incremental work remains in progress. The full optimization and
release plan is not complete.

Latest independent checkpoint: folder-sync deletion overlap is verified. Missing
deletion excludes explicit-intent exact/directory paths after the unchanged explicit
phase. Library 116/116 with 12 ignored, all 128 identities exact. Real PostgreSQL/Redis
baseline 4 passed / 2 failed; candidate 6/6. Public database 7/7, deletion 9/9,
all-targets/fmt/checker 19/19/ratchet/Git pass. Five owners <=500; strict
2179/11/20/39, all 31 above700 unchanged; clearance 114/145 (78.6%). Union 1884,
neighbors 1879, assets 22. Six owned containers/volumes cleaned, 45 prior containers
preserved. Scope correction and unavailable external review route are recorded.
[Report](2026-09-16-folder-sync-deletion-overlap.md). Scope
482401e6e17c6b3bd2f0402844b86d5cc15929a2f30b267b60dc0289ebc533e7. DB/memory/path
bounds, native drain/deadlines/status, races and full acceptance remain; S06 reserved.

Previous independent checkpoint: folder-sync import metadata is verified. Nine-field
SQL avoids unused old payload hydration; maps borrow snapshot entries. Original
public lookup/export and mutation/deletion contracts remain. Real PostgreSQL/Redis
baseline 3 passed / 4 failed; frozen candidate 7/7. Library 116/116 paired, six
ignored, all 122 identities exact. Deletion 9/9, all-targets/fmt/checker 19/19/
ratchet/Git pass. Nine owners <=500; strict 2177/11/20/39, all 31 above700 unchanged;
clearance 114/145 (78.6%). Union 1882, neighbors 1873, assets 22. Four fixture
containers/volumes cleaned, 45 prior containers preserved. No post-freeze source
correction/native replay. [Report](2026-09-16-folder-sync-import-metadata.md).
DB bounds, overlapping deletion passes, native deadlines/full drain/status, races
and full acceptance remain open. S06 stays reserved; overall goal active.

Previous independent checkpoint: provider management deletion ownership passes.
Both DELETE routes share one per-AppState nonqueued permit; whole ordered file/DB/
Redis cleanup runs on a blocking worker and survives caller cancellation. Busy is
explicit 409, with auth and original response/body contracts preserved. Library
baseline 2 passed / 5 failed; candidate 7/7. Real PostgreSQL/Redis baseline3 4 passed /
2 failed; candidate 6/6. Deletion 9/9, auth 4/4 each, all-targets/fmt/checker 19/19/
ratchet/Git pass. Ten owners <=500; strict 2173/11/20/39, 31 unchanged above700,
clearance 114/145 (78.6%). Union 1878, neighbors 1868, assets 22. Eight fixture
containers cleaned, 45 prior containers preserved. Baseline fixture corrections
retained; no post-freeze source correction/native replay.
[Report](2026-09-16-provider-management-deletion.md). DB hydration/folder-sync
success, native deadlines/full drain/status, races and full acceptance remain open.
S06 stays reserved; overall goal active.

Previous independent S09 checkpoint: watcher native ownership passes. Dedicated OS
ownership covers root mkdir, construction and destruction without retaining an idle
Tokio pool slot. Watcher shutdown awaits resource destruction; native calls and
independent admitted sync runs retain their lifetime limits. Notify registration
ordering is repaired. Library baseline 110 passed / 6 failed / 6 ignored; candidate
116/116, six ignored, all 113 prior identities exact. Redis baseline 15 passed /
3 failed; candidate 18/18, all original 15 retained. Deletion 9/9, all-targets/fmt/
checker 19/19/ratchet/Git pass. All 64 subtree files <=500, maximum 460. Strict
2167/11/20/39; 31 unchanged above 700; clearance 114/145 (78.6%). Union 1872,
neighbors 1863, assets 22; fixtures cleaned, 45 prior containers preserved. No
post-freeze source correction/native replay.
[Report](2026-09-16-folder-sync-watcher-native-owner.md). Management deletion,
deadlines/full shutdown/status, DB success, races and full acceptance remain open;
S06 stays reserved and overall goal active.

Previous independent S09 checkpoint: blocking run ownership passes. One blocking
worker retains admission through the whole operation. Caller drop/disable preserve
admitted work; runtime teardown wakes async waits without preempting native calls.
Baseline 102 passed / 3 failed / 6 ignored; candidate 107/107, six ignored, all 108
prior identities exact. Three scheduling regressions repaired, two candidate-only
shutdown tests pass. Redis 15/15, deletion 9/9, all-targets/fmt/checker 19/19/ratchet/
Git pass. All 61 subtree files <=500, maximum 460. Strict 2163/11/20/39; 31 unchanged
above 700; clearance 114/145 (78.6%). Union 1868, neighbors 1863, assets 22. Fixtures
cleaned, 45 prior containers preserved. No post-freeze source correction/native
success replay. [Report](2026-09-16-folder-sync-blocking-owner.md). Watcher startup,
management deletion blocking, DB success, pool fairness, graceful shutdown/deadlines,
races and full acceptance remain open; S06 stays reserved and overall goal active.

Previous independent S09 checkpoint: run ownership passes. Shared no-queue admission
returns stable 409 before input copies. Admitted work/permit survives caller drop
and disable through final status. Redis baseline 12 passed / 3 failed; candidate
15/15 with all ten previous identities. Library 102/102, six ignored, all 103 prior
identities exact; deletion 9/9, all-targets/fmt/checker 19/19/ratchet/Git pass.
All 59 subtree files <=500, maximum 460. Strict 2161/11/20/39; 31 unchanged above
700; clearance 114/145 (78.6%). Union 1866, neighbors 1856, assets 22. Fixtures
cleaned, 45 prior containers preserved. No source correction/native success replay;
proof-marker correction retained. [Report](2026-09-16-folder-sync-run-ownership.md).
Blocking I/O, DB success, deadlines/shutdown/status, races and full product acceptance
remain open; S06 stays reserved and overall optimization remains active.

Previous independent S09 checkpoint: filesystem resource budgets pass. Encoded
material 32 MiB, source path 512 characters, depth 32, entries 100,000 and JSON
files 10,000 are enforced. Reads/pretty serialization are bounded; discovery
limits fail before cleanup. Baseline 84 passed / 13 failed; candidate 97/97, six
ignored, all 90 prior identities/results exact. Deletion 9/9, all-targets/fmt/
checker 19/19/ratchet/Git pass. All 56 subtree files <=500, maximum 460. Strict
2157/11/20/39; 31 unchanged above 700; clearance 114/145 (78.6%). Union 1862,
neighbors 1853, assets 22. No native success replay.
[Report](2026-09-16-folder-sync-filesystem-budgets.md). Run admission, DB hydration,
blocking/cancellation/deadlines, races, shutdown/status and full product acceptance
remain open. S06 stays reserved; overall optimization remains active.

Previous independent S09 checkpoint: static filesystem containment passes. Shared
path validation protects import/read/export/stale/management deletion; invalid
raw paths and descendant links/reparse points fail closed. Configured root links
remain trusted. Staged baseline 75 passed / 9 failed; frozen candidate 84/84, six
ignored and all 78 previous identities/results exact. Existing deletion 9/9,
all-targets/fmt/checker 19/19/ratchet/Git pass. All 53 subtree Rust files <=500,
maximum 460. Strict 2154/11/20/39; 31 unchanged above 700; clearance 114/145 (78.6%).
Union 1859, neighbors 1848, assets 22. No native success replay.
[Report](2026-09-16-folder-sync-filesystem-containment.md). Concurrent replacement,
hard links, bounds/blocking, I/O/shutdown/status, database/provider and full product
acceptance remain open. S06 stays reserved; overall optimization remains active.

Previous independent S09 checkpoint: root ownership migration is complete. Root
3066 -> 124, down 2942; all 50 subtree Rust files are <=500, maximum 460. Five
production owners retain import/export/filesystem/account/layout behavior; the
public API and phase coordinator remain. All 36 tests and two fixtures are preserved
in seven suites with exact module mapping. Fresh library 72/72 and six ignored;
36 mapped and 42 unchanged identities/results verified. All-targets/fmt/source/
checker 19/19/ratchet/Git pass. Strict 2151/11/20/39; 31 remaining oversized
hashes/counts unchanged; clearance 114/145 (78.6%). Union 1856, neighbors 1842,
assets 22. Three accepted reviews; no native success replay.
[Report](2026-09-16-folder-sync-root-ownership.md). Structural closure is accepted;
filesystem bounds/containment/blocking, I/O/shutdown/status, database/provider and
full optimization/release acceptance remain open.

Previous independent S09 checkpoint: complete normalization ownership passes. Root
5366 -> 3066, down 2300; 27 original functions move into 13 owners, maximum 403.
Classification, import/export metadata, dispatch, all provider normalizers and
readers retain their bodies/contracts; original root tests retain every byte.
Hash-revalidated before gate reused; fresh library 72/72, exact 78 identities/results
including six ignored. All-targets/fmt/source/checker 19/19/ratchet/Git pass.
Strict 2138/12/20/39; other 31 oversized hashes/counts unchanged; clearance
113/145 (77.9%). Union 1843, unchanged neighbors 1829, assets 22. Four reviews
complete. Native admission resumed at checker-tests without success replay or
process kill. [Report](2026-09-16-folder-sync-normalization.md).
No runtime replay or performance claim. Dedicated coverage gaps, I/O/shutdown/
status, database/root and full optimization/release acceptance remain open.

Previous independent S09 checkpoint: canonical naming ownership passes. Root 5543 ->
5366; new owner 186. Six exact functions move with aliases/fallback/order and the
full original tests preserved. Accepted hash-revalidated before gate reused; fresh
library 72/72, all 78 identities exact including six ignored. All-targets/fmt/source/
checker 19/19/ratchet/Git pass. Strict 2125/12/20/39; other 31 oversized hashes/counts
unchanged; clearance 113/145 (77.9%). Union 1830, unchanged neighbors 1828, assets 22.
[Report](2026-09-16-folder-sync-canonicalization.md). No native success replay;
scope-capture correction affects evidence only. Runtime/deadlines/shutdown/status,
database/root and full optimization/release acceptance remain open.

Previous independent S09 checkpoint: coalesced-disable admission passes. Typed watch
epochs invalidate queued/enveloped signals and task-owned timer/paths; new-epoch
exact paths survive late control processing. First test-util compilation failure
retained; corrected baseline2 66 passed / 6 failed; frozen library candidate 72/72;
unchanged Redis runtime 10/10. Original 63 identities remain. All-targets/fmt/source/
checker 19/19/ratchet/Git pass; native guard recovery replayed no successes. Owners
165/163/256/161/136/28/93/16/127; root unchanged 5543. Strict 2124/12/20/39;
32 unchanged oversized hashes/counts; clearance 113/145 (77.9%).
[Report](2026-09-16-folder-sync-disable-epochs.md). In-flight/late OS work, I/O
deadlines, status priority, backend shutdown, database/root and full acceptance remain.

Previous independent S09 checkpoint: management enable ownership passes. Shared
runtime admission precedes owned inputs and spawn, covering override, memory and
status. Admitted work survives caller cancellation; queued callers create no work.
Staged inline baseline 9 passed / 1 failed; frozen real Redis candidate 10/10;
unchanged library 63/63. Original six runtime tests remain. All-targets/fmt/source/
checker 19/19/ratchet/Git pass. State/status/target 141/140/98; owners 44/60/134;
root unchanged 5543. Strict 2120/12/20/39; all 32 oversized hashes/counts unchanged;
clearance 113/145 (77.9%). [Report](2026-09-16-folder-sync-enable-ownership.md).
Rapid toggles, shared-field/cross-process ordering, I/O deadlines, backend shutdown,
database/root and full optimization/release acceptance remain open.

Previous independent S09 checkpoint: stale whole-status overwrites are repaired.
All three writers reapply metadata to current JSON and use raw-byte single-key CAS;
eight attempts bound conflicts, with no database replay. Staged baseline 3 passed /
3 failed; same Redis candidate 6/6; library 63/63; unchanged startup/runtime 6/6.
All-targets/fmt/root/status proof/checker 19/19/ratchet/Git pass. Root 5553 -> 5543;
status 190; new owners 53/78/141/57/90. Strict 2117/12/20/39; 32 above 700;
clearance 113/145 (77.9%). [Report](2026-09-16-folder-sync-status-cas.md).
Enabled override/memory ordering, rapid toggles, backend shutdown, durable database
effects/audit, remaining root and full optimization/release acceptance remain open.

Previous independent S09 checkpoint: runtime-owned timer cancellation passes. Disable
drops even elapsed Sleep readiness; queued timer messages and spawned timer tasks
are removed. First-event deadlines remain. Old expired-abort probe 3 passed /
1 failed; library 60/60; unchanged isolated Redis runtime 6/6. Seven owner tests
replace three obsolete protocol tests. Registered-waker cleanup and all-targets/
fmt/source/checker 19/19/ratchet/Git pass. Owners 162/255/27/98/119/134; root
unchanged 5553. Strict 2112/12/20/39; 32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-16-folder-sync-owned-timer.md). Rapid toggles, queued filesystem
events, status races, backend shutdown, root and full acceptance remain open.

Previous independent S09 checkpoint: disabled-start activation passes. The task
subscribes before status I/O and waits for enablement, then starts without restart.
Isolated Redis baseline2 2 passed / 4 failed; frozen candidate 6/6. Real file events,
status, connection-barrier notification and task cleanup are covered; library
56/56 identities stay exact. Initial fixture compile correction and foreign-process
guard recovery are retained. All-targets/fmt/source/checker 19/19/ratchet/Git pass.
Runtime/tests/fixture 265/96/178; root unchanged 5553. Strict 2112/12/20/39;
32 above 700; clearance 113/145 (77.9%). No DB/provider or release acceptance.
[Report](2026-09-16-folder-sync-startup.md). Queued timers/toggles, backend shutdown,
status races, distinct-path memory, root and full optimization remain open.

Previous independent S09 checkpoint: watcher signal coalescing and timer cleanup pass.
Baseline 51 passed / 5 failed; unchanged candidate tests 56/56. Original 45 identities
stay exact. Three pending signal nodes retain every distinct deletion path and the
latest diagnostic; parent cancellation now aborts the owned pending timer. Concurrent
400-path retention, 10,000-change coalescing, closure and cancellation regressions pass.
All-targets/fmt/source/checker 19/19/ratchet/Git pass. Owners 163/257/133/22/135/51;
root unchanged 5553. Strict 2110/12/20/39; 32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-16-folder-sync-watcher-signals.md). Total distinct-path memory is not
hard-capped; enable/toggle/native shutdown and full optimization remain open.

Previous independent S09 checkpoint: watcher and background-task ownership is verified.
Root 5975 -> 5553; new watcher/runtime/tests 161/257/30. Eight function/method bodies,
three type definitions and all original assertions retain exact formatted source.
Paired folder_sync 45/45; three named filter tests move, other 42 identities unchanged.
All-targets, scoped fmt/source/checker 19/19/ratchet/Git pass. An initial verifier-only
import-newline mismatch was corrected without source or test changes. Strict
2106/12/20/39; 32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-16-folder-sync-watcher-owners.md). Event queue growth, timer cancellation
and disabled-start behavior need targeted runtime proof; root/full optimization remain.

Previous independent S09 checkpoint: static file-deletion path containment is repaired.
Real filesystem baseline 2 passed / 7 failed; frozen candidate 9/9. Raw validation,
canonical root confinement and descendant link/reparse rejection preserve ordinary
separators and empty/missing/disabled noops. Original folder_sync stays paired 45/45.
Deletion/paths/tests/fixture 209/118/167/84; parent unchanged 5975. All-targets, scoped
fmt/source/checker 19/19/ratchet/both Git checks pass. Strict 2103/12/20/39; 32 above
700; clearance 113/145 (77.9%). Windows junctions ran; Unix leaf-link test is pending.
[Report](2026-09-16-folder-sync-deletion-containment.md). Concurrent directory replacement,
watcher/root migration and runtime/release acceptance remain open; S06 stays reserved.

Previous independent S09 checkpoint: credential deletion has a 219-line owner and
271-line original-test owner. Parent 6441 -> 5975; cumulative reduction from 6655
is 680 lines. Paired folder_sync 45/45; all 45 logical identities and original
assertions preserved, with only six explicit module-path moves. All-targets,
scoped rustfmt/source proof/checker 19/19/ratchet/both Git checks pass. Strict
2100/12/20/39; 32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-15-folder-sync-deletion-owners.md). Path containment needs dedicated
behavioral repair; the parent, runtime and release plan remain unfinished.

Previous independent S09 checkpoint: folder-sync status/default/Redis persistence
now has a dedicated 227-line owner. Parent 6655 -> 6441; two DTOs and twelve function
bodies are preserved, with original public paths re-exported. Paired folder_sync
45/45; first-candidate missing-import failure is retained, corrected candidate2 is
green. All-targets, scoped rustfmt, source proof, checker 19/19, ratchet and both Git
checks pass. Strict 2098/12/20/39; 32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-15-folder-sync-status-owners.md). The remaining parent, real Redis/
provider/runtime/UI/Docker/release checks remain open; S06 stays reserved.

Previous independent ChatGPT checkpoint: proof solvers now use two-slot fail-fast
admission before copying inputs and run on spawn_blocking. Queued work aborts on
caller drop; running work retains inputs/capacity. Inline baseline 174 passed /
4 failed; candidate 178/178. Original 170 identities and four new preservation/
admission tests stay green. Requirements/solver/tests: 159/85/101/130 effective lines.
All-targets, scoped rustfmt, source proof, checker 19/19, ratchet and both Git checks
pass. Strict 2097/12/20/39; 32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-15-chatgpt-solver-scheduling.md). Admission thresholds still need
real-provider validation; remaining debt and runtime/UI/Docker/release work stay open.

Previous independent ChatGPT checkpoint: HTML challenge MIME matching now uses the
exact essence. Response owner 652 -> 404; classifier/tests 72/316. Broad chatgpt
original 163/163; extracted regression 166 passed / 4 failed; candidate 170/170.
All original identities and three preservation tests remain green. All-targets,
scoped rustfmt, source proof, checker 19/19, ratchet and both Git checks pass.
Strict 2094/12/20/39; 32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-15-chatgpt-html-mime.md). Solver scheduling, remaining debt and
actual packaged/runtime/provider validation remain open.

Previous independent ChatGPT checkpoint: SSE admission now compares the exact
media-type essence rather than searching for a substring. Four HTTP groups fail
before and pass after; upstream ChatGPT baseline 67 passed / 4 failed, candidate
71/71. Original 66 tests plus the new preservation case stay green. Production
25 -> 30; test owner 187 -> 274. All-targets, scoped rustfmt, exact source proof,
checker 19/19, ratchet and both Git checks pass. Strict unchanged 2092/12/20/40;
32 above 700; clearance 113/145 (77.9%).
[Report](2026-09-15-chatgpt-sse-mime.md). HTML challenge MIME matching, synchronous
proof scheduling, real-provider and packaged release work remain.

Previous independent ChatGPT checkpoint: four official API whole-body reads now
use existing 64 MiB collectors. Nine held-open overflow cases fail before and pass
after; candidate 34/34. JSON decoding, diagnostic charset/fallback and raw live SSE
retain their contracts. [Report](2026-09-15-chatgpt-official-body-bounds.md).

## Accepted changes

Across the twenty-six accepted browser-pool batches, the browser-pool entry
decreased from 5,593 to 484 effective lines, a reduction of 5,109 lines.
The latest server lifecycle and media-policy extraction reduced 808 to 484,
removing 324 effective lines while retaining routes, shared client/context state,
startup ordering, progress/gate/retry decisions and the root fatal guard.
The entry is now below both the 700-line target and the 500-line owner limit.
The preceding debug lifecycle repair added three lines for its try/finally boundary;
that increase is already included in the cumulative reduction.
The subsequent storage-state exporter batch reduces its separate entry from 761
to 495 effective lines, moving auth/page signals and runtime capture to two owners.
Five browserless extraction batches reduce their separate entry from 2283 to 490,
a reduction of 1793 lines. Music lifetime repair and material/options ownership
are verified. The browserless root is below both 700 and the 500-line owner limit.
The separate program-handle probe decreased 2474 -> 482 through 319-line
evidence, 287-line media, 115-line snapshot, 265-line interaction and 436-line
execution owners plus shared merger/action/RPC/policy/proxy/assembly/app/metadata
reuse. Pool and standalone UI/RPC policies remain distinct. Root debt is cleared; stop lifetime hardening brings
the root to 496 lines while preventing late publication after capture.stop.
Final execution cleanup brings its owner to 441 and stops capture before context.close.
The broad image-edit diagnostic runner then decreases 796 -> 390 through 283-line
capture and 155-line page owners, preserving its distinct diagnostic policies.
Lifetime repair brings capture/root to 317/397 with explicit stop and context finally.
The independent desktop stylesheet entry decreases 5545 -> 30 through 30 owners,
maximum 441. Original cascade, declarations and test assertions reconstruct exactly.

- Network capture moved to a dedicated owner, then received separate lifetime
  hardening. Queued callbacks and late text/body/Cookie results cannot modify
  stopped capture state or overwrite state adopted by a new page.
- Media URL classification, normalization and dedupe moved to a pure module.
  A separate three-line fix restored HTTPS upgrade for the intended provider
  hostnames and HTTP/HTTPS dedupe while preserving lookalike-host rejection.
- Program snapshot collection moved intact, preserving the self-contained
  browser callback and host-side handle parser boundary.
- Pair ranking/canonicalization and state assembly moved together, preserving
  selection precedence and leaving proxy discovery decisions in the entry.
- Operation-mode UI moved intact, preserving its browser callbacks and injected
  configuration/interstitial boundaries.
- Composer input population and send controls moved together, preserving native
  input events, keyboard fallback and ordered send-button candidates.
- Media asset selection moved to one owner with its scorer injected, preserving
  audio/image/video precedence, result schemas and shared-input immutability.
- Video template/create UI moved intact and imports the shared composer sender.
  Template ordering, localized cues, deselect filtering and click order remain.
- Audio/image byte extraction and navigation download moved together, preserving
  distinct MIME/error contracts, captured-byte bypass and temporary-page cleanup.
- TTS diagnostics, Listen controls and execution moved together, preserving fixture
  bypass, capture acquisition/stop, retry and timeout behavior, and response schemas.
- Text normalization, exact-answer augmentation, snapshot and execution moved from
  two text-only blocks. Baseline comparison, prompt anchoring, stable/deadline selection
  and capture-before-reset cleanup remain; interleaved media predicates stay unchanged.
- Debug capture now stops in finally after acquired-capture work, including failures.
  Reset/acquisition, disabled capture, inherited listeners and diagnostic schemas remain.
  The three-line increase repairs the current owner without mixing in an extraction.
- Debug execution and shared page/button snapshots subsequently moved to two owners.
  Their bodies, accepted cleanup, diagnostic schemas, distinct limits and call sites
  remain unchanged. Pure snapshot imports preserve early app/navigation/TTS wiring.
- Eleven identical Bootstrap snapshot merge sequences now call one local synchronous
  closure. Handle/action/invoke ordering, current capture lookup, snapshot identity,
  page adoption, early returns and the outer finally remain. Separate preview/final
  fallback assembly stays intact. Bootstrap itself decreases from 716 to 569 lines.
- Bootstrap preview result construction moved to a 109-line owner, retaining its
  separate schema, handle errors, action/nullish fallback precedence and shared
  state identities. The enclosing Bootstrap function decreases from 569 to 479 lines.
- Bootstrap polling moved to a 69-line owner. Its initial snapshot, deadline,
  1800 ms cadence, discovery/media decisions, timestamp persistence and 12000/15000 ms
  stabilization/fallback thresholds remain. Bootstrap decreases from 479 to 426 lines.
- Complete Bootstrap execution and its normalization/default-prompt helpers moved
  into one 494-line owner. Direct imports, factory initialization, capture adoption,
  merge ordering, phase-owner calls, result contracts and one outer finally remain.
- Media lease cleanup now covers URL resolution, initial reset and capture startup
  failures. Capture stops only after acquisition, owned pages close and attached
  pages remain open. Normalized base URL reuse keeps the repair net-negative.
- Complete media polling and timeout moved into a 497-line owner. Its seven
  operation inputs, fourteen dependencies and twelve same-source imports preserve
  retry state, delays, provider gates, asset resolution and error contracts. Root
  awaits the owner inside lease cleanup. Its complete execution body is 159 lines.
- Complete media execution and two surface-exit helpers then moved into a 217-line
  owner. Root forwards the public predicate, four direct imports keep their source
  modules and eleven dependencies preserve initialization and awaited cleanup.
- Complete invocation dispatch moved into a 243-line owner. Three direct imports
  and sixteen dependencies preserve account scoping, local lease ownership, cookie
  synchronization, navigation/operation routing, errors and authentication cleanup.
- Complete fetch preview no-key/music dispatch moved into a 144-line owner. Probe
  precedence, JSON/body selection, frame fallback, errors and diagnostic bounds
  remain; root awaits the owner before its original page/capture finally runs.
- Page-context music adaptation moved into a 55-line owner; complete remaining fetch
  execution and its two predicates fit in a 476-line owner. Direct imports, fourteen
  dependencies, helper API, request modes and nested/outer finally behavior remain.
- Server lifecycle moved to a 157-line owner with seven same-source imports and twelve
  initialized dependencies. A 221-line pure media-policy owner preserves progress,
  provider-gate and resubmission decisions; six imports retain native root bindings.
- Storage-state exporter auth/page signals and runtime capture moved into 184-line
  and 116-line owners. Complete blocks, private constants and browser callbacks stay
  intact; CLI exports, manual login, storage configuration and main cleanup remain.

Current new owner sizes: network capture 311, media URLs 115, program snapshot
107, program state 149, operation UI 324, composer 99, media asset selection 322,
video UI 298, payload 189, TTS 336, text 293, debug 160, shared page snapshots 79
and Bootstrap preview result 109, Bootstrap polling 69, Bootstrap execution 494,
media polling 497, media execution 217, invocation dispatcher 243, fetch preview 144,
fetch page music 55, fetch execution 476, server 157, media policy 221, exporter auth
signals 184, exporter runtime capture 116, browserless auth 334, response 66,
requests 403, payload 274, generateContent operations 326, video operation 177
and music operation 202 after its focused lifetime repair, material resolution 211
invocation options 183, standalone program-handle evidence 319, media 287, snapshot
115, interaction 265 and execution 441, broad image-edit capture 317 and page 155
effective lines.
All are below the 500-line owner limit.

## Verification

The latest browser-pool extraction has paired full Node results of 932/932, with identical
test identities/warnings and no skips. All 870 prior identities are retained and
44 server plus 18 media-policy contracts added; the preceding fetch batch added 16.
The preceding cleanup repair retains its
three failing-before/passing-after startup regressions and 22 preservation cases.
Paired nested package contract passes 1/1;
effective-line checker tests pass 19/19. Source reconstruction, frozen input hashes,
UTF-8/no-BOM, syntax, ratchet and Gateway/Neuro Git diff checks pass.
No Node formatter is configured. All verification gates are terminal.

Stopped-capture regression proof changed from 0/13 to 13/13. HTTP upgrade proof
changed from two passing preservation groups and four failing regressions to
6/6 passing. Snapshot tests serialize the callback into an isolated VM context;
they establish closure/contract behavior, not real-browser or provider E2E.
The latest sixteen browser-pool batches add 383 contracts: 25 payload, 20 TTS, 33 text, 15 debug,
13 shared-snapshot/real-root debug, 23 Bootstrap, 14 preview-result, 28 polling/cleanup,
25 media-operation/lease, 30 media-polling, 42 invocation-dispatcher and 37 fetch
operation/preview, 16 fetch execution/music, 44 server and 18 media-policy contracts.
Each structural batch passed the same expanded suite before and after its move. The first TTS
baseline exposed a cross-VM prototype mismatch in one test assertion (593/594).
Only that recorded argument comparison was normalized with structuredClone;
the failed receipt remains preserved and corrected paired runs both pass 594/594.
TTS lifecycle tests execute the real function bodies with controlled dependencies
and virtual time; actual-root probes separately verify fixture/diagnostic/control
behavior. These checks do not claim real speech synthesis or provider execution.
Text lifecycle tests execute original function bodies with controlled dependencies and
virtual time; actual-root helper/snapshot/input probes and source reconstruction provide
separate wiring checks. Independent review found no introduced text extraction defect.
The original deadline fallback reuses the latest polled candidate without another snapshot.
Debug regression proof uses the actual network capture owner on EventEmitter pages:
seven preservation cases pass before the fix, while eight exception cases expose an
extra request listener; all 15 pass after repair, with inherited listeners retained and
late events unable to mutate completed capture state. The first preparation run passed
3/15 because a boolean fixture input disabled capture through string-only parsing. Only
the fixture default changed to string true; failed receipts and unchanged-production
recovery proof remain preserved. This is isolated lifecycle proof, not browser/provider E2E.
Shared snapshot tests preserve filtering, 300/200 control limits, media source metadata,
nonfinite durations, raw anchor fields, existing uncapped media/anchors and body limits.
An actual-root debug probe composes nonempty shared snapshots and diagnostics without
navigation or capture. The accepted cleanup contracts pass before and after extraction.
Bootstrap contracts execute the actual operation body with controlled time and
dependencies, using the real capture owner and real handle/action/invoke merges.
They verify aggregate/state/snapshot identities, separate late action fields,
popup transfer, proxy before-snapshot alias, preview early returns/errors, style
selection, media target refresh, bounded polling and music stabilization. Nine
dependency failures plus a post-adoption merge failure retain inherited listeners
and stop every acquired owner once. Independent review found no introduced defect.
The first projection rejected a repeated lastSnapshot anchor before source edits;
checked reverse offsets and ordered restoration fixed only the evidence script.
Preview-result contracts cover the full key set, input/reference identities,
canonical and hint precedence, unchanged shared arrays, empty/absent values,
independent action fields, error ordering and actual root factory composition.
The owner was prepared from the original 95-line body before baseline, with root
still inline. After integration, the same Bootstrap harness uses the extracted
owner with controlled dependencies. No compatibility branch or copied test
implementation is retained. The package contract adds only the new owner path;
existing byte, manifest and checksum checks protect its packaged inclusion.
Polling contracts execute the extracted function body with a VM clock; production
still uses its original Date.now calls. They cover exact object/call ordering,
initial/deadline behavior, discovery evidence, media exits, timestamp persistence,
stabilization thresholds and six dependency failures. Three full-operation rejection
cases at polling snapshot/build/progress boundaries verify outer capture cleanup.
The same source is tested before and after integration, and the awaited owner call
stays inside the original finally. Its package path is protected by the existing
byte/manifest/checksum contract.
Bootstrap execution extraction adds no permanent tests; the unchanged 720-test suite passes
before and after the move. Two paired evidence-only actual-root calls preserve
missing/blank share/program errors with status 400 and zero page accesses. The
fixture removes its temporary root module. Static reconstruction preserves complete
bodies and the root remainder; all 26 dependencies precede initialization or are
hoisted functions, and eleven direct imports keep their original module sources.
Media startup regression proof records zero owned-page close calls instead of one
when resolution, reset or capture startup throws. The fix moves these steps inside
the existing lease finally and makes capture optional until acquired. Attached-page
counterparts, acquisition failure, success, eight post-capture errors, timeout,
resume and wait rejection retain their contracts. Real capture listeners restore
to inherited counts and stopped state ignores late traffic. Native-root tests keep
invalid/unsupported input errors and complete image/music/video fixture responses.
Independent review found no introduced defect. This is controlled lifecycle proof,
not real navigation, provider media output or a packaged-runtime validation.
Polling contracts add exact retry/reset budgets, template/create state reset,
provider gate sources and twenty-second deferral, ready-video/music distinctions,
one-shot player probing, 1600 ms settlement and its strict deadline boundary,
audio evidence, deferred byte codes and refreshed/initial asset precedence.
The same expanded suite runs the original inline operation before integration,
then the root and actual extracted owner bodies in one VM context afterward.
Existing lifecycle tests also run through the integrated owner, retaining
stop-before-close and error identity checks. Static review verifies all imports,
initialization and the awaited root call; inverse projection restores the complete
root. No real Playwright/provider/runtime success is inferred from this proof.
Media execution extraction reuses the same 775 contracts without additions. Exact
projection restores all three complete function bodies and the root remainder.
The public predicate remains forwarded; the awaited polling call and nullable
capture finally remain intact. Only the owner's expected package path is added.
Dispatcher contracts execute the actual function body with isolated environment
and controlled time/dependencies. They cover foreign/overlapping leases, recreation,
cookie policy and sync failures, account scope, navigation, route order and error
cleanup. A native-root missing-key probe confirms no context growth. These tests
do not establish real context initialization races, cookie parsing or fetch behavior.
The first preparation baseline passed 816/817: three account-root URL expectations
omitted the existing trailing slash. Only those assertions changed. Failed evidence
is preserved and recovery reconstructs all prior source bytes; corrected paired runs
pass 817/817. Native identities establish 42 additions, correcting the scout's
approximate count of 45. Independent projection/contract reviews found no blocker.
Fetch contracts run the original inline operation before extraction, then the
modified root body and actual preview owner body in one VM afterward. The serialized
browser fetch callback also runs, with fixed transport responses and timer tracking.
They protect preview/probe/music branches, frame selection, errors, diagnostic bounds,
page restoration before stop, preserved listeners, late websocket events, UTF-8 and
70001-byte binary results, body rejection, navigation retry and connected fallback.
The pending-result test's entered variable is the started Promise resolver; both
runs complete, disproving a review claim of a hang. Native identity comparison shows
37 additions; the review's 36 count omitted the native invalid-URL test. Timer clearing
is covered, but expiry, hung transport and live browser/provider behavior are not.
Complete fetch extraction adds twelve page-music cases for JSON/null/body precedence,
provider errors, handle/capture identity, program ordering and awaited cleanup, plus
four download/websocket error cases. The same VM harness executes the original root
before extraction and the bound execution/phase bodies afterward. Prior helper bytes
are reconstructed from its focused preparation changes. Native root/helper tests
also preserve forwarded fallback and test-only download predicate exports. Prepared
owner byte equality is backed by original-root inverse projection and paired runs;
all 870 identities pass. Shared page/capture ownership and nested timer finally remain.

Server tests run actual root-returned main bodies in a controlled VM and native
sendJson/readJsonBody functions. They protect UTF-8, route matching, forwarding and
response identity, health projection, status/error mapping, startup defaults and
ordering, TLS values, unreferenced eviction and WebSocket state/callback cleanup.
The shared fixture's six added exports reconstruct the preceding fixture exactly.
Native-root media-policy tests protect progress precedence, operation isolation,
the last-eight-event history bound and input immutability. Inverse projection and
import/initialization proof complement the controlled server dependency harness.
The first server-only candidate measured 689 and passed paired 914/914 plus package
1/1, but ratchet rejected the newly entered 501-700 range without a current exception.
Its receipts remain immutable. Only this turn's root/package integration was reversed
to exact accepted bytes; a fresh expanded baseline then paired with the 484-line
candidate. No exception, checker, policy or baseline changed. The final native
whole-file count 484 supersedes a standalone-fragment estimate of 483.
HTTP-before-TLS failure ordering, existing body accumulation and delegated client
cleanup remain unchanged; no real transport/provider/runtime success is inferred.

The separate exporter extraction has paired focused Node results of 25/25, retaining
nine existing identities and adding sixteen runtime contracts. Tests exercise native
public-root imports and temporary root internal exports, plus actual browser callbacks
in an isolated VM. They protect auth collection, page identity/deduplication, preferred
page order, cross-source key deduplication and transient/nontransient error identity.
Both package runs pass 1/1 with the exporter and owners covered by original byte,
manifest and checksum assertions. Source, encoding, syntax, checker 19/19, ratchet
and both Git checks pass. Acceptance uses a fresh serial baseline after reviewers
ended; earlier receipts are archived separately. Real manual login/provider/runtime
validation remains open. Browser-pool inputs and its preceding 932-test evidence
remain unchanged; that suite was not rerun for this isolated exporter change.

Browserless HTTP extraction passes paired 62/62 dedicated contracts with identical
identities/warnings, plus paired package 1/1, source/encoding/syntax, checker 19/19,
ratchet and Git checks. Temporary-root imports connect the native owners; request
VM tests preserve actual send bodies with fake fetch/timeout/archive. These are
controlled contract tests, without real provider HTTP or native cancellation proof.
Exporter 25/25 and browser-pool 932/932 stay historical, not rerun results.

Media payload extraction adds 28 native contracts for request shapes, candidate
order, MIME/Base64 semantics, response identity, PCM/WAV headers and byte isolation.
Paired expanded browserless suites pass 90/90, retaining all 62 HTTP identities.
The package contract passes 1/1; source/encoding/syntax, checker 19/19, ratchet
and Git checks pass. The 274-line owner preserves the original 254-line block;
root 1580 -> 1345. Five fixture exports reconstruct the predecessor fixture.

HTTP operation extraction adds 54 contracts. Paired expanded browserless suites
pass 144/144, retaining all ninety prior identities. Native-root text integration
uses fake fetch and real request/response/archive owners with temporary cleanup.
Controlled operation tests preserve image attempts, video polling, asset writes
and error propagation. Source/encoding/syntax, checker 19/19, ratchet, package 1/1
and Git checks pass. Owners measure 326/177; root 1345 -> 928. Existing deadline
and download semantics remain; real provider/runtime correctness is not claimed.

Music extraction passes paired 166/166, adding 22 preservation contracts and
reducing the root 928 -> 778. A separate lifetime repair changes owner 178 -> 202;
12 regressions fail before and pass after. Final 178/178 retains every prior
identity. Single settlement, listener cleanup and first-audio ownership suppress
late message/asset work; already issued writes cannot be cancelled. Package 1/1,
source/inverse/encoding/syntax, checker 19/19, ratchet and Git checks pass.

Material/options extraction passes paired 206/206, preserving all 178 identities
and adding 28 cases. Root 778 -> 490; owners 211/183. Main option assignment order,
material/context values, shared helper bindings and fatal tail are preserved.
The first baseline exposed an invalid Windows fixture URL; only the test URL was
corrected, and failed receipts remain. Accepted v2 source/inverse/encoding/syntax,
checker 19/19, ratchet, package 1/1 and Git checks pass. Independent review found
no blocking regression. Real CLI/provider/profile/release gates remain open.

Program-handle evidence extraction passes paired 236/236: all 206 browserless
identities plus 30 standalone parsing/selection cases. Probe 2474 -> 2183;
owner 319. Original bodies, shared bindings and entire live run tail reconstruct.
Package 1/1, source/syntax/checker 19/19/ratchet/Git pass. A review claim about
multiline imports was refuted by actual regex/log evidence; no source change needed.
Browser/profile/network/runtime execution remains outside this pure fixture scope.

Program-handle media extraction and shared merger reuse pass paired 275/275:
all 236 preceding identities plus 39 media/merge contracts. Probe 2183 -> 1813;
media owner 287; shared merger/input remain unchanged at 118/44. Exact source
and inverse proof preserve standalone predicates, scorer injection and the live
tail. Package 1/1, syntax/checker 19/19/ratchet/Git pass. Independent static review
found no blocker. Cookie omission on second-stage merge remains inherited.

Program-handle action/scalar and RPC reuse pass paired 301/301: all 275 preceding
identities plus 26 standalone contracts. Probe 1813 -> 1568; shared modules remain
unchanged. Exact body comparison and inverse projection preserve all remaining
UI/progress/proxy/assembly/network/live code. Standalone UI differences are tested;
package 1/1, source/syntax/checker 19/19/ratchet/Git pass. Static review found no
confirmed regression. Full three-mode/network-event composition remains follow-up.

Program-handle policy/proxy/invoke reuse passes paired standalone 326/326, retaining
301 identities and adding 25. Root 1568 -> 1344; shared assembly 112 -> 115 with
optional UI policy injection and its original default preserved. Complete pool
932/932 is freshly rerun before/after; identities/warnings match in both suites.
Full three-mode result schema and combined RPC/proxy/media/lane precedence are now
covered. Source/inverse, package 1/1, syntax/checker 19/19/ratchet/Git pass. The
host observation timed out, but the original 507-second native gate finished exit 0
without restart. Independent static review found no introduced defect.

Program-handle page ownership passes paired 363/363, retaining all 326 identities
and adding 31 interaction plus 6 snapshot cases. Root 1344 -> 959; owners 265/115.
Complete original bodies and root tail reconstruct; two exact shared app helpers
replace duplicates. Package 1/1 adds only the two owner paths. Source/syntax/checker
19/19/ratchet/Git pass. Snapshot JSON persistence is real; browser callbacks run in
isolated VM fixtures. Runtime DOM/keyboard/popup/provider behavior remains unproven.
Independent review found no introduced defect. Prior pool 932/932 is not rerun.

Program-handle execution ownership and shared metadata reuse pass paired 409/409,
retaining 363 identities and adding 21 metadata plus 25 execution cases. Root
959 -> 482; execution owner 436. Complete source/inverse proof preserves launch,
main/finally, network capture, output ordering and narrower standalone RPC policy.
Package 1/1, source/syntax/checker 19/19/ratchet/Git pass. First-baseline proxy
fixture grammar failure is retained with exact recovery; corrected V2 passes.
Independent review found no introduced defect. VM/native contract tests leave
real browser/provider/runtime and asynchronous capture lifetime work open.

Standalone capture stop repair adds fourteen lines, root 482 -> 496. Native
regressions reproduce all eleven late/queued callback failures before the repair;
final 433/433 retains baseline 422/422 and every previous 409 identity. Per-owner
stopped guards protect shared state after explicit stop, including page adoption.
Active request/response/media/cookie behavior and external listeners remain.
Package 1/1, source/inverse/syntax/checker 19/19/ratchet/Git pass. That checkpoint
left final context cleanup unchanged; the next repair closes this boundary.

Standalone execution-final cleanup grows its owner 436 -> 441. Baseline 433/433
passes; new exit tests pass 3/13 before with ten actual regressions failing, then
final 446/446 retains every prior identity. Four existing cleanup expectations are
intentionally strengthened. Acquired capture stops before context.close; detach
failure preserves success/error identity and context closure. Native/VM tests
cover adoption, inherited listeners, late text/Cookie and absent-owner startup.
Package 1/1, source/inverse/syntax/checker 19/19/ratchet/Git pass. Independent review
found no introduced defect. Real browser/provider/runtime/release work stays open.

Broad image-edit ownership passes paired 480/480, retaining all 446 preceding
identities and adding 14 capture, 7 page and 13 execution cases. Root 796 -> 390;
owners 283/155. Original root and owner bodies reconstruct exactly, including
live capture getters and serialized browser callbacks. Source/syntax/checker
19/19/ratchet/Git and unchanged package 1/1 pass. This manual runner is outside
the production package contract. Import tests prove loadability; source/VM tests
protect byte/output/order contracts without real browser/provider guarantees.
That structural batch left exception cleanup and capture lifetime unchanged.

Broad lifetime repair grows capture/root to 317/397. Baseline 480/480 passes;
29 new regressions fail before and pass after, while two preservation cases remain
green. Final 511/511 retains all preceding identities. Owned page/CDP/socket
listeners and six async read boundaries stop late publication/files; root finally
preserves original errors and closes context after capture stop. One old close-count
assertion shared by three failure tests intentionally changes. Source/inverse,
syntax/checker 19/19/ratchet/Git and unchanged package 1/1 pass. No pending-read
cancellation, active registry/map cap, native runtime or release success is claimed.

Desktop stylesheet ownership passes paired theme 8/8 and Python contracts 52/52,
typecheck and actual web/Tauri frontend builds. Each target's full 11-artifact
census exactly matches baseline. Six real Edge mocked built-UI scenarios at
390/900/1440 preserve selected computed styles and exact PNG bytes. Main visually
opened representative narrow/wide pairs. All 30 owners parse separately; source,
checker 19/19, ratchet and both staged/unstaged Git checks pass. Initial TS reader
load error was corrected with multiline regex mode; retry1 is accepted. No native
Tauri build, live provider or complete theme/state/accessibility acceptance here.

Independent ChatGPT proof ownership reduces the entry 953 -> 131 with PoW/VM/value
owners at 233/394/228. The same protocol filter passes 29/29 before and after,
preserving seven original proof identities and every function/test body. Default
all-targets, scoped rustfmt, source projection, checker 19/19, ratchet and staged/
unstaged Git checks pass. All 22 web/Tauri assets remain identical. No live provider
or native-release acceptance is inferred. At that checkpoint, difficulty/VM bounds remained open.

The subsequent [PoW difficulty repair](2026-09-15-chatgpt-pow-difficulty.md) rejects
provided invalid strings before config allocation, keeps bounded diagnostics and
compares decoded target bytes. Seven groups fail before (33 passed / 7 failed),
then all 40/40 protocol tests pass with the same frozen tests. The 29 prior identities
and four new preservation cases remain. PoW/tests measure 252/115. Default all-targets,
scoped rustfmt, source restoration, checker 19/19, ratchet and both Git checks pass.
Candidate tests passed before a separate terminal-idle guard interruption; the
retained observation and idle resumption close that window without rerunning tests.

The subsequent [Turnstile bounds repair](2026-09-15-chatgpt-turnstile-bounds.md)
adds shared input/execution/value/retention/work/output budgets and repairs raw
dynamic-locator matching against the unchanged whitelist. Before 48 passed / 17
failed; after 65/65 with frozen tests. Sixteen resource cases and one inherited
dynamic-function defect are repaired; 40 prior identities and eight preservation
cases remain. Entry/values/budget/tests 433/231/210/252. Default all-targets, scoped
rustfmt, source proof, checker 19/19, ratchet and both Git checks pass.

The subsequent [ChatGPT execution ownership](2026-09-15-chatgpt-execution-owners.md)
reduces execution 1067 -> 180; requirements/transport/policy/tests 158/167/182/418.
Paired full ChatGPT library tests pass 86/86 with all original function/test bodies
and public exports preserved. All-targets, scoped rustfmt, source proof, checker
19/19, ratchet and both Git checks pass. All owned native handles are terminal.

Latest accepted input union: 1788 files, 1783 unchanged neighbors after preparation.
Scope SHA-256: b696478dd56eb94353884e7eba2f906a490a786858b5398cc0abda80e2b554dd.

## Remaining work

Strict mode still exits 1: 2,083 scanned, 12 hard, 20 mandatory and 40 soft
findings. There are 32 files above 700 effective lines; clearance is now
113/145 (77.9%). Browser-pool, exporter, browserless, program-handle and broad
image-edit entries are cleared at 484, 495, 490, 496 and 397 lines; desktop CSS at 30.
ChatGPT proof is cleared at 131; difficulty hardening and VM logical resource bounds
are accepted. Real-provider threshold validation, synchronous scheduling/backpressure
and PoW CPU work remain separate follow-up areas. Execution is cleared at 180.
Its inherited successful non-SSE stream panic needs separate reproduction/repair.

Bootstrap execution, media lease cleanup, media polling, media execution, invocation
dispatch, fetch preview, page music, complete fetch execution, server lifecycle
and pure media policy are accepted. Exporter auth signals and runtime capture are
also accepted. Browserless auth/response/request owners have incremental proof;
payload, HTTP/music extraction, music lifetime repair and material/options ownership
are also verified. The browserless entry is 490 lines. Program-handle is 496 with
final execution cleanup verified; broad image-edit is 397 with capture/context
lifetime repair. Remaining >700 inventory comprises 20 Rust and twelve
C-like files, including runtime-profile/vendor findings. Continue strict debt and
remaining boundedness/runtime work while retaining all 511 accepted and 932 pool
Node identities.
New owners remain <=500. Runtime-profile vendor findings remain separate.
Capture acquisition before the existing finally and fallback without a connected
client remain inherited boundaries not declared repaired by this extraction.
The media operation's complete ranges measure 618 lines before lease repair, 616 after
repair and 159 after polling extraction. These correct the earlier 617/615/158
planning counts that omitted the closing brace; complete-file/owner counts and
gate results remain unchanged. Exact sources and block hashes are recorded in
the polling checkpoint's media-execution-measurements.json and review-correction.md.
Preserve the repaired lifetime boundary and paired operation suite. Every new owner
must remain <=500 lines. Existing
hypothetical page.off failures may mask primary errors in debug cleanup; no throwing
path was observed. Navigation helpers retain their owned Page; no replacement defect
was established.

S06 ownership and final build transfer remain reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target is 4200, with no persistent 4226. The latest
Rust batch changes only ChatGPT execution and its private policy/requirements/transport/test owners. No dependency,
checker policy, baseline, sibling source or release payload changed;
no deployment or commit was performed. Inherited dirty/staged/deleted files remain.

## Evidence checkpoints

- 20260914-browser-pool-network-capture-owner/scope.json: accepted 2026-09-14T15:33:48.446Z.
- 20260914-browser-pool-network-stop-hardening/scope.json: accepted 2026-09-14T15:45:39.872Z.
- 20260914-browser-pool-media-url-owner/scope.json: accepted 2026-09-14T15:56:59.175Z.
- 20260914-browser-pool-media-http-upgrade/scope.json: accepted 2026-09-14T16:05:11.584Z.
- 20260915-browser-pool-program-snapshot-owner/scope.json: accepted 2026-09-14T16:17:19.511Z.
- 20260915-browser-pool-program-state-owner/scope.json: accepted 2026-09-14T16:30:41.646Z.
- 20260915-browser-pool-operation-ui-owner/scope.json: accepted 2026-09-14T16:49:58.680Z.
- 20260915-browser-pool-composer-owner/scope.json: accepted 2026-09-14T17:20:26.197Z.
- 20260915-browser-pool-media-assets-owner/scope.json: accepted 2026-09-14T17:34:31.811Z.
- 20260915-browser-pool-video-ui-owner/scope.json: accepted 2026-09-14T17:47:39.145Z.
- 20260915-browser-pool-payload-owner/scope.json: accepted 2026-09-14T18:03:41.737Z.
- 20260915-browser-pool-tts-owner-v2/scope.json: accepted 2026-09-14T18:24:05.291Z.
- 20260915-browser-pool-text-owner/scope.json: accepted 2026-09-14T18:45:58.287Z.
- 20260915-browser-pool-debug-cleanup-v2/scope.json: accepted 2026-09-14T19:01:50.769Z.
- 20260915-browser-pool-debug-snapshot-owners/scope.json: accepted 2026-09-14T19:30:58.895Z.
- 20260915-browser-pool-bootstrap-snapshot-state/scope.json: accepted 2026-09-14T19:56:06.989Z.
- 20260915-browser-pool-bootstrap-preview-result/scope.json: accepted 2026-09-14T20:15:44.936Z.
- 20260915-browser-pool-bootstrap-polling/scope.json: accepted 2026-09-14T20:34:41.869Z.
- 20260915-browser-pool-bootstrap-execution/scope.json: accepted 2026-09-14T21:00:27.415Z.
- 20260915-browser-pool-media-lease-cleanup/scope.json: accepted 2026-09-14T21:13:16.211Z.
- 20260915-browser-pool-media-polling/scope.json: accepted 2026-09-14T21:34:16.242Z.
- 20260915-browser-pool-media-execution/scope.json: accepted 2026-09-14T21:52:40.778Z.
- 20260915-browser-pool-dispatcher-v2/scope.json: accepted 2026-09-14T22:12:45.113Z.
- 20260915-browser-pool-fetch-preview/scope.json: accepted 2026-09-14T22:31:19.430Z.
- 20260915-browser-pool-fetch-execution/scope.json: accepted 2026-09-14T22:49:05.874Z.
- 20260915-browser-pool-server-v2/scope.json: accepted 2026-09-14T23:24:34.988Z.
- 20260915-canvas-exporter-owners/scope.json: accepted 2026-09-14T23:44:51.701Z.
- 20260915-browserless-http-owners/scope.json: accepted 2026-09-15T00:11:46.441Z.
- 20260915-browserless-payload-owner/scope.json: accepted 2026-09-15T00:27:26.194Z.
- 20260915-browserless-http-operations/scope.json: accepted 2026-09-15T00:50:30.010Z.
- 20260915-browserless-music-owner/scope.json: accepted 2026-09-15T01:06:11.070Z.
- 20260915-browserless-music-lifetime/scope.json: accepted 2026-09-15T01:22:37.834Z.
- 20260915-browserless-material-owners-v2/scope.json: accepted 2026-09-15T01:44:22.225Z.
- 20260915-program-handle-evidence-owner/scope.json: accepted 2026-09-15T02:09:31.253Z.
- 20260915-program-handle-media-owner/scope.json: accepted 2026-09-15T02:35:58.719Z.
- 20260915-program-handle-contract-reuse/scope.json: accepted 2026-09-15T02:55:16.053Z.
- 20260915-program-handle-invoke-reuse/scope.json: accepted 2026-09-15T03:38:32.684Z.
- 20260915-program-handle-page-owners/scope.json: accepted 2026-09-15T04:15:51.449Z.
- 20260915-program-handle-execution-v2/scope.json: accepted 2026-09-15T04:45:00.846Z.
- 20260915-program-handle-capture-lifetime/scope.json: accepted 2026-09-15T05:01:44.798Z.
- 20260915-program-handle-final-cleanup/scope.json: accepted 2026-09-15T05:21:33.929Z.
- 20260915-image-edit-broad-owners/scope.json: accepted 2026-09-15T05:51:31.732Z.
- 20260915-image-edit-broad-lifetime/scope.json: accepted 2026-09-15T06:12:28.172Z.
- 20260915-desktop-style-owners/scope.json: accepted 2026-09-15T07:03:33.826Z.
- 20260915-chatgpt-proof-owners/scope.json: accepted 2026-09-15T07:53:21.515Z.
- 20260915-chatgpt-pow-difficulty/scope.json: accepted 2026-09-15T08:37:32.839Z.
- 20260915-chatgpt-turnstile-bounds/scope.json: accepted 2026-09-15T10:32:36.023Z.
- 20260915-chatgpt-execution-owners/scope.json: accepted 2026-09-15T11:03:17.815Z.
- 20260915-chatgpt-stream-response/scope.json: accepted 2026-09-15T11:46:21.327Z.
- 20260915-chatgpt-web-body-bounds/scope.json: accepted 2026-09-15T12:22:03.227Z.
- 20260915-chatgpt-official-body-bounds/scope.json: accepted 2026-09-15T12:59:41.322Z.
