# Lane R: AI Studio live probe decomposition

Coordinator owns scripts/probe-aistudio-live-request.mjs and its new
scripts/aistudio-live-probe modules. This production probe is distinct from
the previously split AI Studio worker and Lane F's seven probe test suites.
Gemini/S06 source, Rust, Cargo, browser profiles and real provider sessions
remain outside this batch. Structure is accepted at the final checkpoint below;
runtime hardening and inclusion in a new immutable release remain pending.

## First extraction batch

Baseline entry: 2756 effective lines. The entry is now 2210, still a hard
violation requiring continued work. Three completed leaf owners are input text
normalization/parsing/preview (35), RPC contract extraction/readiness (279),
and UI/auth/proxy diagnostics with final summary projection (242). All are
UTF8 without BOM. The dependency direction is entry to diagnostics to RPC
contract to input text; no cycle or new module-load side effect was added.
Existing exported probe APIs and the suppressed-main guard remain intact.

The TypeScript AST verifier compares all 85 original function bodies across
their unique current owners exactly after newline normalization. Retained
entry-level state declarations, exports and dispatch also match exactly.
No tests, serialized callbacks, data fields or assertions were dropped.

Evidence: target/effective-line-evidence/aistudio-probe. Baseline and updated
seven-file AI Studio test group pass 34/34 (7.851s and 8.587s). All affected
MJS files pass node --check. Full Node 22.22.2 passes 420 with one POSIX skip.
The nested package test explicitly adds the real probe and its three modules,
verifies bytes/support records/checksums, and imports the packaged diagnostic
module from outside the package. Its relative RPC/input imports resolve and
public function exports exist; the package fixture passes 1/1. This extends
the generic flat package fixture, so nested inclusion is directly covered.
Ratchet and diff pass. Independent read-only review found no extraction defect.

Current snapshot: 1161 files, 40 hard, 71 mandatory, 38 soft. Original debt
clearance stays 34/145; 111 files remain above 700. Neither baseline nor exception
registry changed. The latest full Python gate predates Q/R; focused packaging
and the current full Node gate cover this batch's direct dependencies.

## Remaining extraction and hardening

Still in the entry: request configuration, browser/proxy preflight, runtime-state
storage, request attribution, local WebSocket transport, page actions and the
large main orchestration. All must be divided by ownership before structural
acceptance. The self-contained browser init-hook string is a promising main
boundary; capture listeners and polling share mutable timing/activity state.
Any browser-launch extraction must preserve cleanup of partially created
browser/context handles on failure, not only return handles after success.

Existing unbounded JSON/string traversal, selected credential-bearing replay
headers, diagnostic material, storage paths, body capture and lifecycle behavior
were moved unchanged and remain explicit hardening work. This is an incremental
structure change, not a provider success, completed optimization or release.
No Cargo or live browser was run. GWP-20260908-06 remains unacknowledged and
requires fresh source/docs freeze acceptance before an immutable build.

## Attribution, transport and storage batch

Four additional owners now hold request identity/RPC pairing (168 effective
lines), the local WebSocket server (236), executable/proxy preflight (198), and
runtime-state/object-storage mirroring (163). The storage client remains a
single module-owned cached instance. The socket set, parser state and capture
callbacks stay together, while main retains the same close order. Immutable
RPC endpoint/browser-path constants moved to their sole responsible owners.

Entry is now 1462 effective lines, down from 2756. It is below the hard cap but
still mandatory split debt; structural clearance is unchanged at 34/145.
Current snapshot: 1165 files, 39 hard, 72 mandatory and 38 soft; 111 above 700.
All 85 original functions and 17 top-level state declarations/constants retain
exact unique source bodies. Entry public exports and main dispatch are exact.

Intermediate attribution/WebSocket and final preflight/storage probe suites
both pass 34/34. Final full Node 22.22.2 is 420 passed/1 POSIX skip. Syntax,
nested package 1/1, ratchet and diff pass. The actual package fixture explicitly
includes all seven extracted modules and the probe entry. Independent read-only
review found no import, state ownership, callback or packaging regression.
All owners are UTF8 without BOM. Evidence: attribution-websocket-after.log,
preflight-storage-after.log, transport-storage-equivalence/package/ratchet/diff/
node-all.log in this lane's evidence directory.

Remaining source extraction: local request shaping, page snapshots/dispatch,
UI actions, CLI I/O and the large main. Existing WebSocket admission/backpressure,
async persistence races, storage-path containment, proxy diagnostic secrets and
cleanup behavior were preserved, not certified hardened. No live provider,
browser or Cargo was run; all coordinator native sessions are terminal.

## Final structural checkpoint

The entry now has 484 effective lines, reduced from 2756. Fifteen cohesive
owners hold browser hooks (145), preflight (198), capture polling (73), CLI I/O
(32), diagnostics (242), input text (35), local dispatch (194), page observation
(78), publication (178), request configuration (92), RPC attribution (168), RPC
contracts (279), runtime storage (163), UI actions (259), and WebSocket capture
(236). Every completed owner is below 500, with no exception or baseline change.
The latest inventory scans 1175 files: 39 hard, 71 mandatory, and 38 soft;
110 files exceed 700. Original debt clearance is now 35/145 (24.1%).

Main retains browser/context/socket ownership, listener state and the original
cleanup order. Polling reads listener timing through a live getter after each
await boundary. Publication phases retain artifact/output order and exit codes;
the self-contained browser hook retains its original string literal. The AST
verifier proves 84 original helper bodies and 17 original state/constants exact.
It expands the three extracted phases, checks their argument bindings, restores
the hook declaration, and proves main's parsed leaf tokens exact. Public exports
and dispatch remain exact. There are 88 current functions versus 85 originally.

Nine focused probe suites pass 41/41, including seven new tests for hook event
retention, live polling state, retry behavior, and traffic/required/replay-ready
publication artifacts and exit codes. Full Node 22.22.2 passes 427 with one
POSIX-only skip (428 total). Full offline Python passes 283 tests in 514.468s
with four explicit skips. The checker suite passes 19/19, ratchet passes, and
the real nested package fixture passes 1/1 with all 15 modules byte-verified.
All 18 affected MJS files pass syntax and UTF8/noBOM checks. Independent phase
and page-owner reviews found no concrete extraction regression. Evidence is
main-phases-equivalence.log, main-phases-after.log and final-*.log in this lane's
evidence directory. All coordinator native sessions are terminal.

This checkpoint completes R's structural split only. Unbounded input/body/WS
collection, async persistence races, credential-bearing diagnostics, storage
containment, whole-worker cancellation and cleanup still require hardening.
Synthetic publication tests do not establish real browser/provider behavior or
fault-injected cleanup reliability. No Cargo or new immutable release ran;
GWP-20260908-06 still requires a transfer receipt and fresh source/docs freeze.

## Input admission hardening

After structural acceptance, CLI input now has a cumulative 16 MiB raw-byte
limit and a 30-second EOF deadline. One lazily allocated fixed buffer bounds
tiny-chunk bookkeeping; decoding uses only initialized bytes and retains split
UTF8, whitespace, empty input and leading-BOM behavior. EOF/error/close/overflow/
deadline settle once, clear the timer, detach listeners and pause the stream.
Malformed JSON now returns a fixed 400/aistudio_probe_invalid_json diagnostic
before capture directories or browser work. Native Node diagnostics were shown
to echo the synthetic malformed-input canary; the real CLI regression proves
the fixed output does not echo it. Stream errors still propagate their original
error; this is not a general diagnostic-redaction guarantee.

The entry remains 484 lines; CLI I/O grows from 32 to 80 and the new input test
file has 95. All are UTF8 without BOM. Six input tests cover the canary CLI,
split bytes/BOM, exact limit, cumulative overflow, deadline and error/close
cleanup. Probe suites pass 47/47; latest full Node is 433 passed/one POSIX skip
(434 total), nested package passes 1/1, checker 19/19, ratchet and diff pass.
Independent review found no concrete regression. Evidence: input-before.log,
input-after.log, input-node-all.log, input-package.log, input-checker.log and
input-ratchet.log. The earlier full Python gate predates this input-only change;
the directly affected package gate was rerun. The structural AST proof above
is historical and intentionally does not describe these behavior changes.

This bounds stdin collection only; JSON object expansion, page/body/WS capture,
storage/persistence and the complete worker lifecycle still need hardening.
No Cargo/new release or real provider/browser was run. All native sessions are
terminal; the shared build window remains unacknowledged.

## WebSocket ingress admission

The local capture server now rejects HTTP upgrade headers above 16 KiB including
their terminator and declared frame payloads above 16 MiB before waiting for the
payload. Pending input is checked before concatenation against 16 MiB + 16 KiB
+ 14 bytes. Oversized bursts are rejected even when they contain multiple valid
frames; this is an explicit pending-buffer admission policy. Rejected sockets
are destroyed and pending bytes released. This caps retained per-socket input,
not transient concat allocations, total server memory or network allocations.

Async socket persistence callbacks now catch failures, record a fixed diagnostic
and close the affected socket; error history is capped at eight entries. This
prevents the former second rejection escaping a data-handler catch and uncaught
error/close-handler rejections. Direct sendMessages persistence still rejects to
its caller. Concurrent capture writes, callback completion during server close,
connection counts, capture arrays, outbound backpressure and handshake deadlines
remain open work; this checkpoint does not certify complete transport safety.

The new real-loopback admission suite has five tests: oversize handshake,
exact-limit handshake, oversize declared frame, split TCP input plus normal
dispatch, and persistent storage failure without secret echo or unhandled
rejection. Before the change, two admission cases failed and two normal cases
passed. Latest full Node is 438 passed/one POSIX skip (439 total), including all
52 probe tests. Nested package passes 1/1, checker 19/19, ratchet, syntax and diff
pass. Source/test are 259/92 effective lines and UTF8 without BOM. Evidence:
ws-admission-before.log, ws-admission-after.log, ws-node-all.log, ws-package.log,
ws-checker.log and ws-ratchet.log. Independent review could not run because its
agent provider returned model_not_found; coordinator inspected the changed code.
All native sessions are terminal. No full Python rerun, Cargo, new immutable
release or real browser/provider execution occurred in this follow-up.

## WebSocket retained capture budgets

The server now admits at most 64 connections over its lifetime, including
already closed report entries. Received frame parsing shares a 4096-frame budget
across all connections and rejects excess complete frames before allocating
their records. Received preview/event-type text shares a 4 MiB UTF8-byte budget;
event types must be strings and their diagnostic previews are limited to 256
characters. Budget violations close the affected socket with a bounded error
record. These policies prevent unbounded retained inbound connection/frame/text
history; outbound history/queues and aggregate pending bytes remain separate
unfinished work. JSON decoding and transient parser allocations are not covered
by the retained-text budget.

Three new regressions failed against the prior implementation, then passed:
lifetime connection history, excessive tiny frames, and accumulated preview
bytes. A fourth proves exactly 4096 frames are admitted and a second connection
cannot reset that budget. Final transport suite passes 9/9. Full Node on the
production change passed 441 with one POSIX skip (442 total); the last extra
cross-connection test was then added and the complete transport suite rerun.
Nested package 1/1, checker 19/19, ratchet, syntax and diff pass. Source/test are
280/155 effective lines and UTF8 without BOM. Evidence: ws-capture-before.log,
ws-capture-after.log, ws-capture-final-focused.log, ws-capture-node-all.log,
ws-capture-package.log, ws-capture-checker.log and ws-capture-ratchet.log.
All native sessions are terminal. No Cargo/new release or real browser/provider
ran; the shared source/docs freeze still requires explicit transfer acceptance.

## WebSocket outbound admission

Outbound dispatch now shares a lifetime 4096-frame budget across sockets/calls,
rejects batches above that count and dispatch keys above 1024 characters, and
checks encoded payload size against 16 MiB before creating a frame. Per-socket
writable queue admission reserves the payload plus maximum frame header; a slow
reader exceeding the queue budget is disconnected. Sent previews, string event
types (256-character diagnostic limit) and retained dispatch keys now consume
the same 4 MiB text budget as received captures. Handshake/pong markers remain
separately bounded by the connection/received-frame limits. Valid normal dispatch
and only-undispatched selection retain their existing behavior.

Three new outbound regressions failed before implementation. They now pass,
alongside a real paused-loopback-reader test proving queue overflow rejects.
Full Node passes 446 with one POSIX skip (447 total), including all 13 transport
tests. Nested package 1/1, checker 19/19, ratchet, syntax and diff pass. Source/test
are 300/188 effective lines, UTF8 without BOM. Evidence: ws-outbound-before.log,
ws-outbound-after.log, ws-outbound-node-all.log, ws-outbound-package.log,
ws-outbound-checker.log and ws-outbound-ratchet.log. The focused after log predates
the paused-reader case; the full Node result includes it.

Object serialization still precedes byte admission, and the capture text budget
does not bound transient JSON expansion. Aggregate pending buffers, copy cost,
handshake deadlines and persistence ordering/lifecycle remain open. Outbound
limit errors propagate to the caller; dispatch is not transactional and an
earlier message in a rejected batch may already have been sent. No Cargo/build,
full Python rerun or real browser/provider ran. All native sessions are terminal.

## Shared pending-buffer ownership

The transport now uses a dedicated pending-buffer pool with a 32 MiB shared
capacity budget across connections. Capacity grows geometrically from 16 KiB
rather than concatenating the complete prefix for every tiny TCP chunk. Growth
charges both old and new buffers while copying, rolls reservations back on
allocation failure, and rejects before allocation when the shared budget is
exhausted. The existing per-socket byte ceiling remains. Empty consumption,
rejection and socket close release reservations idempotently; partial consumption
compacts the tail only after completed frames have been inspected. Parser views
are dropped before releasing capacity or awaiting persistence.

Five focused buffer tests prove initialized-byte/tail behavior, geometric
allocation count, exact per-socket ceiling, cross-connection capacity recovery,
and growth/failure accounting. A real-loopback regression additionally preserves
one completed text frame and a partial next frame across compaction. Final
buffer/transport group passes 19/19. Full Node on the production change passes
451 with one POSIX skip (452 total); the final added compaction regression then
passed in the focused group. Nested package 1/1 explicitly includes the new
pending-buffer module; checker 19/19, ratchet, syntax and diff pass. Transport,
buffer, buffer test and transport test have 301/59/84/204 effective lines, all
UTF8 without BOM. Evidence: ws-pending-after.log, ws-pending-final-focused.log,
ws-pending-node-all.log, ws-pending-package.log, ws-pending-checker.log and
ws-pending-ratchet.log.

The pool measures owned buffer capacity, not RSS, garbage-collection timing,
kernel buffers or decoded JSON objects. Handshake deadlines, serialized capture
writes and complete worker cancellation/cleanup remain open. No Cargo/new
release, full Python rerun or real provider/browser ran. All native sessions
are terminal and the shared build-window transfer remains pending.

## WebSocket handshake lifetime

Each admitted socket now owns a fixed 10-second upgrade deadline. Traffic does
not refresh it. Expiry releases pending capacity and destroys an unfinished
socket with a fixed diagnostic; successful upgrade and socket close clear the
timer. A late callback cannot close an upgraded/already destroyed socket, and
the timer does not independently keep the process alive.

The new regression failed before implementation and now verifies expiration,
successful-upgrade cancellation, late-callback safety and early-close cleanup
using the actual loopback server with only timer scheduling controlled. Combined
transport/buffer tests pass 20/20, nested package 1/1, checker 19/19, ratchet,
syntax and diff pass. Transport/test are 311/247 effective lines, UTF8 without
BOM. Evidence: ws-deadline-before.log, ws-deadline-after.log, ws-deadline-package.log,
ws-deadline-checker.log and ws-deadline-ratchet.log. Full Node/Python were not
repeated for this focused lifetime change; earlier full gates predate it.
Persistence ordering and whole-worker lifecycle remain open. All native sessions
are terminal; no Cargo/build or real browser/provider ran.

## Capture write serialization

Capture persistence now has one coalescing writer. Concurrent callers share a
single drain promise and dirty flag, so no snapshot/promise queue grows with
traffic and writeFile calls cannot overlap. Each iteration serializes the latest
capture when the write begins. Stop closes admission, discards queued work and
waits for the active write; main stops the writer before publishing a failure
artifact, preventing an older asynchronous capture write from overwriting it.
Final cleanup also stops the writer. Five tests prove coalescing 10000 requests,
latest-state sampling, failure recovery, active-write drain and late-write
suppression. Independent read-only review found no introduced race.

Probe suites pass 72/72; full Node passes 458 with one POSIX skip (459 total),
nested package 1/1, checker 19/19, ratchet, syntax and diff pass. The package
fixture explicitly includes capture-writer.mjs. Entry/writer/test are 489/28/82
effective lines and UTF8 without BOM. Evidence: writer-after.log,
writer-node-all.log, writer-package.log, writer-checker.log and writer-ratchet.log.
Direct file writes are not yet atomic against process/filesystem interruption.
Browser event-task rejection propagation, complete callback drain at shutdown
and bounded worker cancellation remain open. No full Python rerun, Cargo/build
or real browser/provider ran. All native sessions are terminal; shared transfer
remains pending.

## Browser event ownership

Browser capture state and event bindings now live in browser-capture.mjs. Main
creates the owner before browser launch, attaches at the original pre-navigation
point, activates at the original capture-armed point and passes its live match
time getter to polling. Request identity/attribution, response content selection,
early page diagnostics and WebSocket entry behavior remain unchanged. The exact
block verifier proves retained listener source and state source (apart from
owner indentation), plus the main wiring. This structural step creates a clear
owner for the remaining callback admission/error/drain work without enlarging
the entry above its preferred size.

Entry drops from 489 to 357 effective lines; browser owner/test are 148/83.
All are UTF8 without BOM. Existing probe 72/72 and three new owner tests pass;
full Node is 461 passed/one POSIX skip (462 total), package 1/1, checker 19/19,
ratchet, syntax and diff pass. Independent review found no import or lifecycle
ordering regression. Evidence: browser-capture-baseline.json,
verify-browser-capture.mjs, browser-capture-equivalence.log,
browser-capture-after.log, browser-capture-focused.log,
browser-capture-node-all.log and browser-capture-package/checker/ratchet.log.
Browser callback rejection, unbounded retained arrays and shutdown drain remain
unchanged and open. No Cargo/new release or real browser/provider ran. All native
sessions are terminal; the shared build-window transfer remains pending.

## Browser callback admission and logical drain

Matching request/response callbacks now share a 64-task limit with individual
30-second deadlines. Unmatched events are filtered before admission. Callback
and synchronous listener failures produce fixed diagnostics, stop admission and
detach owned page/context/WebSocket listeners. Stop normally waits for admitted
callbacks before final publication; failure/timeout aborts their logical tasks.
Post-await signal checks prevent late native completions from mutating capture
arrays. Main checks failure through the polling getter and stops capture before
success or failure publication. Closed owners cannot attach new listeners.

Four new regressions cover rejection propagation/detachment, successful drain,
overload versus unmatched traffic, and timeout followed by late completion.
Probe suites pass 79/79; full Node passes 465 with one POSIX skip (466 total),
nested package 1/1, checker 19/19, ratchet, syntax and diff pass. The final closed
attachment guard and its assertion then pass the seven-test owner suite and
ratchet. Entry/browser owner/task owner/test are 360/172/60/155 effective lines,
UTF8 without BOM. Evidence: callback-tasks-after.log,
callback-tasks-final-focused.log, callback-tasks-node-all.log,
callback-tasks-package.log and callback-tasks-checker/ratchet.log.

Independent review correctly emphasized that logical cancellation does not abort
native Playwright calls or filesystem writes; those operations can finish later.
Its suggested synchronous listener interleaving is not possible across an await
here, but closed-owner registration is now explicitly rejected. The suggested
success publication/write race was checked against publication.mjs: it awaits
persistCapture before producing the summary, using the serialized writer.
Failure publication additionally stops that writer before its direct final write.
The earlier exact extraction proof remains historical before these intentional
behavior changes. Native operation/process cancellation, bounded retained browser
arrays, atomic writes and full runtime validation remain open. All native test
sessions are terminal; no Cargo/build or real browser/provider ran.

## Retained browser capture budgets

All browser network/page/console/frame/WebSocket records now pass shared
admission: 4096 appended records, 1 MiB encoded bytes per record and 16 MiB total
charged bytes. Preflight bounds each record's traversal to 2048 nodes/depth 8
and checks raw UTF8 strings before allocating escaped JSON. Final charging
includes JSON escaping and punctuation. Budget failures use a fixed 413 code,
close callback admission and detach listeners; console handling no longer
swallows this specific failure. Ordinary request/response metadata remains exact
when admitted, preserving replay data rather than silently truncating headers.

Independent review identified that later WebSocket frame mutations also need
to count against their containing entry's 1 MiB ceiling. WeakMap owner charging
now enforces that ceiling in addition to global count/bytes. Five budget tests
and ten browser-owner tests pass, including escaping/multibyte accounting,
aggregate limits, cycles/depth/breadth, console floods, oversized headers and
nested WebSocket growth. Final full Node passes 473 with one POSIX skip (474
total); final nested package passes 1/1, checker 19/19, ratchet, syntax and diff
pass. Browser owner/task owner/budget owner/tests have 176/66/48/182/49 effective
lines, UTF8 without BOM. Evidence: browser-budget-final-focused.log,
browser-budget-final-node-all.log, browser-budget-final-package.log,
browser-budget-checker.log and browser-budget-ratchet.log.

These are retained browser-event payload budgets, not a whole capture artifact
or process-memory limit. The local raw WebSocket server has its separately
verified budgets. Native response.text/header allocations, cancellation of
Playwright operations, atomic file replacement and final real-provider/release
validation remain open. All native test sessions are terminal; no Cargo/build
or real browser/provider ran.

## Atomic capture JSON publication

Root and publication JSON artifacts now use a same-directory exclusive temporary
file, write/sync/close, then rename over the destination. Failed replacement never
falls back to deleting the destination. Cleanup removes only a temporary file
successfully created by this invocation; an open collision cannot delete someone
else's file. This covers capture snapshots, initial/final page and frame JSON,
RPC summaries/contracts/pairs, and success/failure summaries. Runtime-state mirrors
and screenshot PNGs remain separate paths. Per-file atomic visibility does not
provide a multi-file transaction, directory fsync or a power-loss guarantee.

Four real-filesystem tests cover UTF8 replacement/noBOM, partial temporary-write
failure, replacement against an existing directory, and foreign temporary-file
preservation after create failure. Focused atomic/publication/CLI group passes
15/15; full Node passes 477 with one POSIX skip (478 total), nested package 1/1,
checker 19/19, ratchet, syntax and diff pass. Independent review found no confirmed
defect. Entry/writer/test are 361/21/66 effective lines and UTF8 without BOM.
Evidence: atomic-after.log, atomic-node-all.log, atomic-package.log,
atomic-checker.log and atomic-ratchet.log. Latest line inventory scans 1188 files,
with 39 hard/71 mandatory/38 soft; original clearance remains 35/145.

Native I/O deadlines, Playwright body allocation/cancellation, runtime mirror
containment/atomicity and real-provider/release validation remain open. A separate
read-only coordination audit confirms no GWP-20260908-06 receipt; the earlier
GWP-05 ownership transfer ended at 13:32 UTC. All native sessions are terminal;
no Cargo/build or real browser/provider ran in this checkpoint.

## Runtime object-key containment and mirror publication

Runtime reads and mirrors now resolve relative object keys through one storage
path owner before I/O. It bounds key length/segments, rejects traversal/absolute
paths, ADS and common Windows path aliases, and rejects existing symlink/junction
components below the configured root. Both slash styles remain accepted for
local paths; remote S3 object keys are unchanged. Mirror directories are checked
again after creation, and remote/local mirror files use atomic replacement.

Three storage tests cover invalid key families, a real linked descendant, and
contained runtime read/write with repeated mirror replacement. Final storage/
atomic focused group passes 7/7 on Windows. Probe 94/94 and full Node 480 passed/
one POSIX skip (481 total) passed before the final repeated-mirror assertion;
that assertion then passed in the focused group. Package 1/1, checker 19/19,
ratchet, syntax and diff pass. Runtime/path/test are 164/31/57 effective lines,
UTF8 without BOM. Evidence: storage-path-after.log,
storage-path-final-focused.log, storage-path-node-all.log,
storage-path-package.log and storage-path-checker/ratchet.log.

Independent review's suggestion that Windows rename generally cannot replace
an existing file was contradicted by the real atomic replacement test and the
new repeated-mirror assertion. Existing open-file/filesystem errors still fail
without deleting the destination. Configured root authority may itself follow
a link. These checks do not protect against concurrent ancestor replacement or
recursively validate links inside a browser profile; neither is claimed complete.
Remote body/deadline/client lifecycle, native browser cancellation and real
provider/release validation remain open. All native sessions are terminal;
no Cargo/build or real provider/browser ran.

## Remote runtime-state request lifecycle

S3 Get/Put requests now receive an AbortSignal and a shared 30-second request
deadline; downloads keep that deadline through body consumption. Declared and
actual runtime-state bodies above 16 MiB are rejected. Node readable bodies are
consumed incrementally with geometric 64 KiB-to-16 MiB storage, destroyed on
abort/overflow, and their abort listeners removed. The former unbounded
transformToByteArray fallback is rejected. Uploads enforce the same byte ceiling
before allocating their Buffer. SDK failures use fixed diagnostics, and main
finally destroys/releases the cached S3 client.

Six tests cover split/empty/exact-limit bodies, declared/streamed overflow,
small-body allocation, stalled-body timeout, diagnostic redaction, and mocked
SDK Get/Put integration with abort signals and idempotent client cleanup.
Focused storage group passed 8/8 before the small-allocation test; final full
Node includes all six new tests and passes 486 with one POSIX skip (487 total).
Nested package 1/1, checker 19/19, ratchet, syntax and diff pass. Independent
review verified the standard Node SDK body/call contract and found no direct
resource leak. Entry/runtime/request owner/test are 362/159/68/120 effective
lines, UTF8 without BOM. Evidence: object-storage-after.log,
object-storage-node-all.log, object-storage-package.log and
object-storage-checker/ratchet.log.

An SDK/custom handler that ignores abort may still finish remotely after timeout;
the wrapper bounds waiting and consumes late rejection, not remote transaction
rollback. Standard Node readable bodies are supported; browser Blob/Web streams
or arbitrary transform adapters are not accepted by this Node-only worker.
Upload JSON serialization precedes byte admission. Local runtime file reads,
filesystem deadlines, profile races and real-provider/release validation remain
open. All native sessions are terminal; no Cargo/build or live S3/provider ran.

## Bounded local runtime-state read

Local storage-state JSON now uses an abortable 64 KiB stream with the same
16 MiB actual-byte ceiling and 30-second waiting deadline as remote bodies.
The local boundary maps I/O, timeout and JSON failures to fixed diagnostics;
UTF-8 BOM compatibility is retained. Reading/parsing occurs before Chromium
launch, avoiding browser allocation for invalid state. Package coverage includes
the new owner. This does not validate the Playwright cookie/origin schema or
guarantee cancellation of a stalled native filesystem operation.

Focused tests passed 17/17, including real local file admission and a CLI
secret-canary failure before executable launch. Full Node passed 489 with one
POSIX skip (490 total); nested package passed 1/1. Checker 19/19, ratchet,
syntax and diff checks passed. Independent review found no actionable defect.
Entry/reader/test measure 360/28/51 effective lines, UTF-8 without BOM.
Evidence: runtime-file-after.log, runtime-file-node-all.log,
runtime-file-package.log, runtime-file-checker.log, runtime-file-ratchet.log
and runtime-file-diff.log under the lane evidence directory.

Inventory is 1194 files: 39 hard, 71 mandatory, 38 soft. Original debt clearance
remains 35/145; 110 files exceed 700. Native browser body allocation, callback
drain, profile races and real-provider/release acceptance remain open. No
Cargo/build or live provider ran; the shared build transfer remains pending.

## Local WebSocket publication drain

Socket data parsing now completes synchronously before persistence yields,
including frames coalesced with the upgrade request. Close stops admission,
destroys sockets, waits for their close handlers and pending persistence, and
returns the same Promise on repeated calls. Sends after close are rejected;
already admitted send persistence is tracked too. Main closes this capture
source before success or failure publication, preserving final closedAt fields.

Two real-loopback regression tests cover blocked persistence, coalesced frames,
idempotent close, late sends, final-state stability and concurrent admitted
send persistence. Independent review identified the initially untracked send
path; it was fixed before acceptance. Full Node initially exposed an older
transport test attaching its client close listener after draining server close;
the listener now registers before initiating close. Final full Node: 491 passed,
one POSIX skip (492 total). Focused admission 17/17, package 1/1, ratchet, syntax
and diff pass. Entry/transport/admission test are 362/334/297 effective lines,
UTF-8 without BOM. Evidence: ws-drain-after.log, ws-drain-node-all.log,
ws-drain-package.log and ws-drain-ratchet.log.

Drain waits for persistence and has no independent native-filesystem deadline;
this remains a lifecycle limitation. No real browser/provider or Cargo/build
ran. Original 110-file debt and shared release-transfer requirement remain.

## Nonblocking browser fetch previews

The page hook returns the original fetch Response without waiting for its body.
Preview capture reads at most 4096 bytes into a fixed buffer, with a five-second
deadline and eight admitted clone/reader lifecycles. Tee branch cancellation is
not awaited by the fetch wrapper. Independent review identified that admission
must remain charged until cancellation settles; that correction prevents repeated
timeouts from creating unlimited pending clone cancellations. Pending cancellation
can exhaust preview slots, but never delays the application's fetch response.

Four new VM/real Response tests cover stalled bodies, original response identity,
complete original-body consumption, byte admission, deadlines and cancellation
slot ownership. The existing hook fixture now supplies browser timer/decoder
globals and waits for the asynchronous diagnostic event. Focused 8/8; full Node
495 passed / one POSIX skip (496 total); package 1/1, ratchet and diff pass.
Hook/new tests/existing hook tests are 171/75/74 effective lines, UTF-8 without
BOM. Evidence: fetch-preview-after.log, fetch-preview-node-all.log,
fetch-preview-package.log and fetch-preview-ratchet.log.

The preview ceiling does not cap native stream chunk allocation or the original
consumer's buffering. UTF-8 truncation can yield a replacement character. Real
browser/provider validation, other page instrumentation boundaries and the
shared release window remain open. No Cargo/build or live provider ran.

## Message diagnostic projection

Message previews no longer stringify arbitrary message graphs before forwarding.
A 64-visit/four-level/16-entry projection caps retained strings and keys, uses
own property descriptors without invoking accessors or toJSON, and marks cycles
and unsupported values. Projection failures produce a fixed marker while the
original message and transfer references still reach postMessage. Independent
review prompted path-local cycle tracking and sparse-array position preservation.

Four regression tests cover circular/BigInt values, unchanged forwarding,
throwing accessors/toJSON, wide inputs, throwing proxy enumeration, shared
references and sparse arrays. Focused hook group 12/12; full Node 499 passed /
one POSIX skip (500 total); nested package 1/1, ratchet and diff pass. Hook/test
are 204/58 effective lines, UTF-8 without BOM. Evidence: message-preview-after.log,
message-preview-node-all.log, message-preview-package.log and
message-preview-ratchet.log. Native enumeration and arbitrary Proxy execution
are not bounded by the projection's retained-node limit. The historical
three-argument forwarding shape remains unchanged. No Cargo/build/live provider
ran; shared release transfer and broader optimization remain outstanding.
