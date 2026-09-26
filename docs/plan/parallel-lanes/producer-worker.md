# Lane N: Producer browser worker decomposition preparation

Coordinator scope: scripts/producer-browser-worker.mjs, future cohesive Producer
modules, characterization/package tests only. Rust/S06 remains independently
owned; no Cargo/build takeover is implied.

Current source remains2124 effective lines, still hard debt. Read-only scouting
located a large serialized page callback and a separate Node-side video flow.
The first planned extraction is pure conversation stream/material and video
prompt preparation, not a dependency-injected rewrite of the whole worker.

Five baseline characterization tests now pass, using only a selected pure
function region in a VM; they never import/run the executable browser worker.
They cover SSE frame commit/conversation IDs, tool and suggestion summaries,
nested media URL ordering, lyric/timestamp projection, and proposal/confirmation
prompt aliases. Evidence: target/effective-line-evidence/producer-worker/
pure-before.log. Production code is unchanged; no debt clearance is claimed.
The bounded test fixture currently supplies the unchanged string normalizer
and confirmation default. After extraction it must import production modules
directly rather than retaining a fallback loader that could mask missing wiring.

Security/resource issues such as recursive media traversal, diagnostic payload
size, stream reads, browser/profile ownership and final process exit remain
unresolved; characterization is not certification of these behaviors.

## Pure ownership extraction checkpoint

Producer entry2124 ->1838 effective lines, still hard debt and NOT a completed
large-file split. New request-fields33, conversation-stream174 and video-prompts86
effective lines separate alias coercion, SSE/material projection and prompt
preparation. Eleven moved function bodies and the complete main body (including
serialized page callback and cleanup) match the saved baseline exactly after
normalizing line endings/export prefixes. The callback keeps its own local
helpers, not Node imports. Evidence: pure-equivalence.log; an initial proof
invocation emitted an empty log and was rerun explicitly, so exit0 alone was
not accepted. A subsequent expected-count typo12 was corrected to the measured11.

Tests now import production owners directly; the old VM fallback is removed.
Paired baseline5/5 and post-extraction5/5 pass. Wiring adds one test; full Node
matrix at that checkpoint291 passed/1 POSIX skip. Synthetic official-packager
test passes1/1 and verifies all four Producer files, bytes, support manifest and
checksums. Checker, ratchet and diff pass. Independent read-only review confirmed
module closure and callback scope; direct field-boundary cases were added after
review; final focused7/7 and ratchet/diff pass (pure-final.log). No real
browser/provider or full worker replay is claimed. The source
and nested modules are not yet in an immutable release; GWP05 remains pending.

## Node video flow and transport checkpoint

Entry1838 ->1308 effective lines, still above700 and not a completed split.
New trace20, transport144 and video-flow371 owners preserve seven additional
function bodies exactly, including fetch options, result/error contracts and
status polling. At this pure-move checkpoint the complete main/page callback
also remained exact (flow-equivalence.log). Ten synthetic flow fixtures passed
before and after extraction, including pending/completed distinction, asset
selection and eight error boundaries. Tests now import the real flow and
transport owners; only page.evaluate responses and global fetch are synthetic.
They do not call providers or launch a browser. Trace output is disabled before
module import in that isolated Node test process. Package verification includes
all seven Producer files, not just the entry.

### Separate serialized-scope bug fix

Earlier read-only review was wrong to call the page callback fully self-contained:
its pending-status branch referenced outer STATUS_POLL_INTERVAL_MS. Coordinator
inspection and a second focused review located the missing lexical binding.
The actual callback was compiled in an isolated VM and driven through six
synthetic requests; baseline failed with ReferenceError on the first pending
status. Moving the unchanged5000ms constant inside the serialized callback fixes
that boundary with zero effective-line growth. The fixture now reaches completed
with the expected asset and observes the5000ms delay. This is a behavior repair,
separate from the pure-move equality checkpoint, not real browser validation.
Evidence: page-poll-before.log and flow-final.log (18/18 focused tests).

Remaining: main/page callback still needs cohesive decomposition; trace paths,
unbounded bodies/recursive material, whole-worker deadlines and browser/profile
cleanup remain open. No Cargo or immutable release was changed.

Final verification: Node303 passed/1 POSIX skip (304 total), synthetic package1/1,
checker and ratchet pass, diff check clean. Strict1116 scanned:40 hard,
75 mandatory,38 soft; exit1. Total115 above700 remains,30/145 original files
structurally cleared. Evidence: node-flow-final.log, package-flow-final.log,
checker-flow-final.log, ratchet-flow-final.log and strict-flow-final.log.

## Integrated interim release receipt

The partial Producer extraction and separate page-polling fix are included in
immutable20260908-producer-mailbox-s06-123700. Official build, provenance,
package/UI integrity and two isolated runtime checks passed; see
../../status/2026-09-08-producer-mailbox-s06-release.md. This supersedes earlier
unreleased statements for these exact checkpoints, without completing the1308
line entry split or certifying real Producer browser/provider behavior.

## Post-release page, profile and input extraction (acceptance pending)

Entry 1308 -> 620 -> 502 -> 453 effective lines. The serialized page callback
moves to page-video-flow.mjs (690); profile (121) and input (51) own their four
unchanged functions each. Nine modules now range from 20 to 690 effective lines.
The callback expression is exact after removing only its old indentation;
production uses a direct ESM import passed to page.evaluate. Tests import that
actual function and serialize it into an isolated VM, preserving the browser
scope boundary without a fallback source loader. No page globals, eval or
source-string assembly were introduced into production.

Page baseline 12/12, post-extraction 13/13; profile paired 3/3 and input paired
2/2. Cases cover confirmation, nested fields, stream material, status polling,
missing IDs/assets and error boundaries. The official synthetic package test
verifies all ten Producer files, bytes, support records and hashes (1/1).
Full Node: 320 passed, 1 POSIX skip (321 total); syntax and diff checks pass.
Evidence: page-equivalence.log, profile-equivalence.log, input-equivalence.log,
node-structure-final.log and package-structure-final.log.

The ratchet fails for the 690-line page closure's missing current soft exception.
Two independent approval attempts failed on provider authentication before any
review; they supply no approval. A concrete review request was sent to S06 in
the shared handoff. The exception registry and adoption baseline remain unchanged.
Do not count this file as accepted debt clearance until that gate is resolved.
Normalized page hash:
847e46604ed9883cb91ede4334f178572083d497d62d4752f7ee8355305787cd.

This extraction retains existing profile traversal/symlink/partial-copy cleanup,
response limits and cancellation issues. Tests use disposable synthetic paths;
they do not certify hostile profile safety or real provider/browser behavior.
The immutable 123700 release predates these new extractions.

## Profile containment and failure cleanup

Three regressions failed before this separate behavior change: profile name `..`
was accepted, a linked Network directory copied data outside the selected
profile, and a failed Preferences copy leaked its temporary root. Fixtures used
only a dedicated synthetic directory and removed their own output afterward.

The profile owner now validates a single Windows-safe child name before cloning,
rejects traversal/ADS/device names, inspects source descendants with lstat and
rejects links and special files. The operator-selected root may still be a
NAS-style directory link; realpath canonicalizes that explicit trust boundary.
Regular profile state files must be files. Copy/readdir failures propagate,
rather than silently accepting a partial clone, and failure removes only the
root returned by this invocation's mkdtemp. Cleanup failure is reported together
with the original failure. POSIX-created directories request mode0700.

Profile source is now 176 effective lines (was121). Focused 10/10 covers direct
clone validation, selection, linked profile/Network/nested descendants, allowed
operator root links, copy selection/source preservation and failure cleanup.
Full Node 327 passed/1 POSIX skip. This does not close filesystem TOCTOU races,
Windows ACL policy, copy byte/depth/time budgets, cancellation, or worker-exit
cleanup. Source body-equivalence claims apply only to the earlier pure move.
Evidence: profile-safety-before/after/final.log and node-profile-safety.log.
The page closure and its hash are unchanged; size approval remains pending.

## Normal-exit cleanup ordering

The Producer worker called process.exit from inside its browser try block when
printing a normal result, skipping finally. Five baseline child-process cases
failed the cleanup-order contract (four existing error paths already passed).
Main now returns its structured result; the top-level writer runs only after
main's context/browser/profile finally completes. Structured error exit code0
and existing result payload conventions are preserved, including an empty page
result. No worker deadline or browser process-tree kill behavior was added.

The real Node child fixture executes the current entry with isolated synthetic
imports, tracks asynchronous close/removal completion and stdout, and verifies
one output after cleanup across ten success/error/launch/context/navigation
scenarios. Final full Node337 passed/1 POSIX skip; synthetic package1/1 and diff
pass. Ratchet still fails only the unchanged page closure's pending size review.
Evidence: lifecycle-before.log (5 failures), lifecycle-after.log (9 passed before
the added empty-result case), node-lifecycle-final.log (all10 included) and
package-lifecycle-final.log. Close/removal error suppression, hung cleanup,
external cancellation and unbounded stdin remain explicit unfinished work.

## Bounded stdin collection

A separate input owner now caps raw stdin at 16 MiB and requires EOF within
30 seconds before browser/profile setup. The fixed buffer is allocated lazily;
only initialized bytes are decoded once, avoiding an unbounded tiny-chunk array.
EOF, overflow, timeout, error and premature close clear the timer, detach all
four listeners and pause the stream. A settlement guard ignores late callbacks.
Raw whitespace/empty input behavior is retained for the existing JSON parser.

The oversized real-child regression failed before (generic parse500 rather than
the bounded413 result), then passed before any profile/browser allocation.
Direct-module tests cover split UTF8, exact-limit/empty input, byte overflow,
open-pipe deadline, stream errors/close and listener/timer cleanup. Final Node
343 passed/1 POSIX skip; synthetic package1/1 and diff pass. The only ratchet
failure remains the unchanged 690-line page closure awaiting independent review.
Evidence: stdin-before.log, stdin-child-after.log, stdin-unit-after.log,
node-stdin-final.log and package-stdin-final.log.

This bounds this reader's byte storage, not total process memory, parse work,
subsequent browser work or parent-side writes. Parent/child cancellation and
end-to-end oversized-input error propagation still need integration verification.
The immutable123700 release predates this hardening and the post-release split.

## Malformed-input diagnostic redaction

A real-child regression proved that native JSON.parse errors echoed a synthetic
credential canary from malformed stdin into the structured worker result.
Parsing now happens at the input boundary with a fixed400
producer_browser_invalid_json error and no retained native cause or payload.
Existing valid input and distinct missing-field validation errors are preserved;
the entry uses the same parsed object without a second parser. Its trace receives
the same sanitized error. Other upstream/trace error paths are not certified.

Focused input plus child lifecycle tests15/15 and synthetic package1/1 pass;
diff is clean. The earlier full Node343/1-skip checkpoint remains historical,
not a claimed rerun after this change. Ratchet still rejects only the unchanged
690-line closure's missing independent exception. Evidence:
input-diagnostic-before.log (canary echo reproduced), input-diagnostic-after.log,
package-input-diagnostic.log and ratchet-input-diagnostic.log.

## Node status response bound and abort handling

Two baseline regressions reproduced swallowed response-body errors (reported as
successful empty responses) and credential-bearing fetches permitting redirects.
Node-side status fetch now forbids redirects even if options request follow,
reads at most 16 MiB of decoded response bytes into bounded storage, and cancels
and releases its reader on completion/failure. Body errors propagate as failure;
network/body diagnostics use fixed messages rather than arbitrary error text.
The request timer remains active through body reading, and explicit reader
cancellation unblocks reads after headers. Timeout is classified from the owned
controller as504; oversized upstream bodies produce a distinct502 error code.

Six transport cases and ten conversation-flow cases pass on both the default
Node runtime and explicit Node22.22.2 (16/16 each). They cover byte overflow,
exact limit, split UTF8, body failure, redirect configuration, plus native Node
loopback requests proving a stalled response times out/closes its socket and a
redirect target is never visited. Servers and sockets are fixture-owned and
closed after tests. Video-flow fixtures now use actual Response objects instead
of a text-only stub. Synthetic package1/1 and diff pass; ratchet still fails only
the unchanged page closure's pending size exception. Evidence:
node-transport-before/after/final.log, node22-transport-final.log and
package-node-transport.log. No real Producer service was contacted.

This changes Node status transport only. Serialized browser fetch/SSE bodies,
page status polling, aggregate job deadlines and external caller cancellation
remain open. The byte cap is not a whole-process memory/decompression guarantee;
non-success HTTP response bodies remain returned within the cap, not redacted.

## Page phase split and structural acceptance

At 14:32 UTC the coordinator replaced the 690-line serialized closure with Node
orchestration and two self-contained Playwright callbacks. Request shaping and
tool-result selection live in page-request-input.mjs (133 effective lines),
conversation transport/SSE interpretation in page-conversation.mjs (278), and
status polling/asset selection in page-status.mjs (127). The page-video-flow.mjs
orchestrator is 197 lines. Entry is now 437; all 14 Producer modules are 20-371.
Browser helpers remain local to each callback because page.evaluate serializes
functions without their module scope. Phase arguments/results cross the existing
Playwright serialization boundary; no runtime eval or browser globals were added.

The earlier assertion that further splitting necessarily required a new protocol
was too broad. Separate conversation/status phases provide a cohesive boundary.
The pending exact-hash soft-exception request is withdrawn; registry and baseline
were not changed. Seven extracted helper blocks are byte-identical after the
documented indentation/export transforms and trailing separator removal, as
checked by target/effective-line-evidence/producer-worker/check-page-phases.mjs.
The orchestration call sites intentionally changed from local calls to page.evaluate.

Before: 13 page fixtures passed. After: page plus real-child lifecycle 25/25,
full Node 22.22.2 matrix 351 passed/1 POSIX skip, package 1/1, checker 19/19,
ratchet and diff pass. VM fixtures serialize each callback into a fresh context
and clone both arguments and results, exercising closure and phase ownership.
The package contract includes all three new nested files and verifies real bytes.
Independent read-only review found no new regression; coordinator measurements
and fresh evidence take precedence over the review's stale historical citations.
Evidence: page-phases-before/after.log, page-phases-node22-all.log,
page-phases-equivalence.log, page-phases-package.log, page-phases-checker.log,
page-phases-ratchet.log and page-phases-strict.log.

Strict scans 1135 files, with 40 hard and 74 mandatory failures plus 38 soft files.
The structural debt counter advances to 31/145. This closes the Producer oversized
file gate only. Inherited unbounded browser bodies/status fetch, raw diagnostics,
recursive traversal, aggregate deadlines and external cancellation remain open.
No real browser/provider or new release was validated in this phase; immutable
20260908-producer-mailbox-s06-123700 predates these post-release improvements.

## Browser status polling deadline

Two pre-fix fixtures reproduced a completed result arriving after the status
budget while stalled headers or body reads were never aborted. The serialized
status callback now owns one AbortController and deadline timer for its whole
polling phase. Fetch and body consumption share that signal; the interval wait
is abortable and removes its listener/timer on either outcome. All terminal paths
clear the phase deadline. Timeout retains the established 504 code and latest
status payload; other transport/body failures return a fixed 502 diagnostic.
Previously swallowed body-read failures no longer become empty successful reads.

Five deadline/error fixtures plus the 13 page-flow fixtures pass on Node 22.22.2
(18/18). They cover stalled headers/body, interrupted polling delay with timer
cleanup, latest-status retention and sanitized network/body errors. Nested
package 1/1, ratchet and diff pass. Status owner is now 162 effective lines.
Evidence: page-status-deadline-before/after/final.log,
page-status-deadline-package.log and page-status-deadline-ratchet.log.
The earlier phase-equivalence log is a structural checkpoint before this explicit
behavior change; its status-body equality is not asserted for the hardened file.
The full Node 351/1-skip checkpoint likewise predates this focused hardening.

Browser body byte/depth bounds, redirects, conversation-phase budgets, total
worker deadlines and external cancellation remain open. This is synthetic
serialization/abort validation, not a live browser/provider completion claim.

## Browser status body admission and redirect policy

Three baseline failures reproduced oversized-body acceptance, a retained body
reader lock, and redirect-following configuration. The serialized status callback
now reads at most 16 MiB of decoded bytes, preserving split UTF8 characters with
one final decoder pass. The reader is cancelled and released on all terminal
paths; abort explicitly cancels pending reads. Overflow returns a fixed 502
producer_browser_status_response_too_large error. Status requests use redirect
error so credentials are not sent through an unintended redirect chain.

Focused body/deadline/page fixtures pass 21/21; full Node 22.22.2 matrix passes
359/360 with one POSIX-only skip. Exact-limit acceptance, overflow cancellation,
released locks, split UTF8 and redirect configuration are covered. Deadline
fixtures now provide real Response/ReadableStream bodies. Nested package 1/1,
ratchet and diff pass. Status owner is 205 effective lines; current ratchet
scans 1137 files, still 40 hard plus 74 mandatory and 38 soft.
Evidence: page-status-body-before/after.log, page-status-body-node22-all.log,
page-status-body-package.log, page-status-body-ratchet.log and
page-status-body-diff.log. The redirect assertion checks the browser request
configuration; no real-browser redirect test or provider request was performed.

Remaining work identified by independent read-only review includes browser
conversation/SSE limits, provider diagnostic redaction, recursive URL traversal,
aggregate worker cancellation and profile-copy budgets. The 16 MiB reader does
not bound parsed-object overhead, decompression, traversal or the whole process.

## Browser conversation transport budget

Six baseline failures reproduced unrestricted POST/SSE bodies, swallowed body
errors, redirect-following and a renewed stream timeout after submission used
part of its budget. The serialized conversation callback now owns one elapsed
deadline across POST and SSE, with the same abort signal on both fetches and
body readers. Each decoded response is capped at 16 MiB; readers explicitly
cancel on timeout/overflow and release their locks. Both requests refuse redirects.
Transport/body failures return fixed 502 diagnostics, timeout returns a fixed
504 producer_browser_conversation_timeout, and overflow has a distinct code.
Existing HTTP/SSE error and successful conversation payload contracts remain.

Ten transport fixtures plus 13 page-flow fixtures pass on Node 22.22.2 (23/23).
Coverage includes POST and SSE overflow, exact-limit acceptance, stalled-reader
abort/cancellation, body error propagation, combined elapsed budget and redirect
configuration. Nested package 1/1, ratchet and diff pass. Conversation owner is
319 effective lines; all Producer owners remain below 500. Evidence:
page-conversation-transport-before/after/final.log,
page-conversation-transport-package.log, page-conversation-transport-ratchet.log
and page-conversation-transport-diff.log. The full Node 359/1-skip checkpoint
predates this change; this checkpoint claims focused verification only.

This closes admission and elapsed-time handling for the serialized conversation
callback. The separate browserContextFetch/browserContextReadSse functions used
by the Node video path still need equivalent bounds. Provider HTTP/SSE diagnostic
payload redaction, parsing/traversal complexity, total multi-phase job deadline,
profile copy and worker-wide cancellation remain open. No live-provider or
new-release completion is claimed.

## Monotonic budget across page phases

Five baseline regressions showed late bootstrap/creative/confirmation/status
results could continue work or publish success, and every phase received a fresh
minimum of five seconds even when less time remained. The Node orchestrator now
uses performance.now() for its total elapsed budget, rejects admission when it
is exhausted, passes only the remainder to browser callbacks, and rejects results
that arrive after the deadline. Created timestamps retain wall-clock semantics.

Five deterministic phase-budget fixtures plus all browser transport/page fixtures
pass 36/36 on Node 22.22.2. Full Node matrix passes 374/375 with one POSIX-only
skip. Nested package 1/1, ratchet and diff pass. Orchestrator is 208 effective
lines. Independent read-only review found no new defect in the serialized
transport/deadline changes and reproduced the focused 36/36 result. Evidence:
page-flow-deadline-before/after.log, page-flow-deadline-node22-all.log,
page-flow-deadline-package.log, page-flow-deadline-ratchet.log and
page-flow-deadline-diff.log.

This bounds admission and elapsed execution within the page-flow phases. It does
not cancel a hung Playwright IPC connection, profile setup, refresh, launch or
cleanup; whole-worker cancellation remains separate. Node-path browserContext
transport, diagnostics, traversal and copy budgets also remain open.

## Post-phase offline integration gate

At 15:08 UTC, full Python discovery completed 283 tests in 566.477 seconds with
zero failures and four opt-in skips. Console Redis, splitter E2E and recovery
Docker gates were disabled explicitly. This complements the latest Node 374
passed/1 POSIX skip checkpoint and verifies package/source contracts against
the post-phase checkout. Evidence: post-phase-python-all.log.
The GWP-20260908-06 shared build/source/docs window is still awaiting an explicit
S06 receipt; no new native release or global completion is claimed.

## Node-path browser transport admission

Seven baseline failures reproduced unbounded fetch/SSE response consumption,
swallowed body errors/raw exception disclosure, redirect following, and retained
SSE reader locks. Both Node-path browserContext wrappers now call one independently
serialized browser-transport.mjs callback. It enforces a 16 MiB decoded-byte cap,
shares the fetch deadline with body reading, explicitly cancels pending reads,
releases locks and refuses redirects. Fixed transport diagnostics preserve the
existing fetch/stream timeout/failure codes; overflow has a distinct code.

The existing SSE terminal-event regex behavior is retained, including its early
terminal marker recognition. This work does not certify complete SSE-frame
semantics or linear-time scanning; accumulated-text rescanning remains an open
performance item within the new byte cap. HTTP response diagnostics and page
metadata also remain unredacted. These limitations must not be hidden by the
transport admission gate.

Eleven browser transport fixtures plus 16 existing Node transport/video-flow
fixtures pass on Node 22.22.2 (27/27). They cover overflow, exact limit, split
UTF8, abort of stalled reads, error redaction, redirect policy, and terminal
reader cancellation/unlock. Nested package 1/1, ratchet and diff pass. The new
callback is 72 effective lines; transport wrappers plus Node fetch are 40.
Producer now has 15 modules, all below 500. Evidence: browser-context-before/
after/final.log, browser-context-package.log, browser-context-ratchet.log and
browser-context-diff.log. The prior full Node 374/1-skip checkpoint predates this
change; no new native release, full-suite or real-provider result is claimed.

## Complete terminal SSE frames with incremental detection

Three regressions reproduced premature cancellation after a fragmented final
header, a nonterminal event name beginning with final, and an event field later
overridden in the same frame. The browserContext SSE detector now waits for a
complete frame delimiter, recognizes exact final/error names, and respects the
last event field. CR, LF and fragmented CRLF plus long event whitespace are
covered. Raw terminal data is retained before cancelling/releasing the reader.

The previous whole-text regex rescans are removed. Detection visits each decoded
character once and retains only bounded prefix/event state, independent of the
number of body chunks. The body remains limited to 16 MiB; final output text is
decoded separately from accepted bytes. This is a terminal-frame detector, not
a replacement for the downstream conversation/SSE payload parser.

Initial focused 14 cases had 3 failures; after correction, context/video cases
24/24 passed, then an additional CRLF/whitespace case was included in the full
Node 22.22.2 gate: 389 passed/1 POSIX-only skip (390 total). Nested package 1/1,
ratchet and diff pass. Independent read-only review found no new detector or
serialization defect. Browser transport is now 113 effective lines. Evidence:
browser-sse-frame-before/after.log, browser-sse-frame-node22-all.log,
browser-sse-frame-package.log, browser-sse-frame-ratchet.log and
browser-sse-frame-diff.log. This supersedes the earlier retained-regex limitation.

Eager allocation of the maximum byte buffer, downstream parsing/traversal,
provider diagnostics and whole-worker cancellation remain separate work. No
real-provider, immutable release or whole-plan completion is claimed.

## Response buffer allocation sizing

Four allocation-probe regressions measured 16 MiB reservations for tiny Node,
browserContext, page-conversation and page-status responses. All four readers
now start with a 64 KiB minimum and grow geometrically only when accepted bytes
need more space. The 16 MiB response admission check still runs before growth;
no backing buffer can exceed that cap. Previous bytes are copied before the new
chunk is appended, preserving repeated-growth and split UTF8 behavior.

Six allocation/growth cases plus the relevant transport matrix pass 40/40 on
Node 22.22.2. Tiny-response probes now observe at most 64 KiB per allocation;
repeated-growth fixtures retain distinct prefix/middle/suffix content. Existing
exact-limit, overflow, abort, reader cleanup and SSE cases pass. Nested package
1/1, ratchet and diff pass. Evidence: response-allocation-before/after/final.log,
response-allocation-package.log, response-allocation-ratchet.log and
response-allocation-diff.log. The earlier full Node 389/1-skip checkpoint predates
this focused change. Capacity expressions were subsequently line-wrapped only.

This reduces small-response buffer reservations, not total process memory.
Growth temporarily holds old and new buffers; fetch chunks, decoded strings,
parsed objects and garbage-collection timing add overhead. Downstream traversal,
diagnostic output and whole-worker lifecycle still need separate bounds.

## Bounded media traversal

Five baseline failures reproduced excessive nesting, wide payloads, URL floods,
ancestor cycles and unclassified complex page status payloads. Both the Node
collector and the independently serialized status collector now use an explicit
stack with depth 64, 100,000 total scheduled nodes and 1,024 collected URLs as
limits. Scheduling is charged before queue growth; URL admission is checked
before append. Only current ancestors are retained in the cycle set, preserving
repeat-reference output and the original depth-first ordering. Limit/cycle
failures return a fixed 502 producer_media_payload_too_complex diagnostic.

Focused traversal/material/page/video cases pass 37/37. Full Node 22.22.2 matrix
passes 402/403 with one POSIX-only skip, including exact depth/URL/node limits
and the serialized status error boundary. Nested package 1/1, ratchet and diff
pass. Independent read-only review found no new order, budget or error-mapping
defect. Conversation material is 199 effective lines and page status is 249.
Evidence: media-traversal-before/after.log, media-traversal-node22-all.log,
media-traversal-package.log, media-traversal-ratchet.log and
media-traversal-diff.log.

These limits govern the media collectors. They do not bound JSON decoding,
other SSE/proof/diagnostic transformations, operator profile copying or the
whole worker process. The immutable release still predates the post-release
N/O batches; GWP-20260908-06 remains without a shared-window receipt.

## Bounded tool-content projection

Five baseline failures reproduced original-object retention at the depth limit,
wide and branching summary expansion, cyclic output, and prototype-named field
loss. Tool content now uses a shared 256-visit projection budget, depth four,
five array entries, 32 object entries and 120-character property names. Existing
240-character string, lyric-preview and timestamp-sample contracts remain.
Depth/budget exhaustion emits markers rather than returning original objects.
Null-prototype output records preserve prototype-named fields as ordinary data.

Call-site inspection confirmed that the Node video flow consumes root job_id/
jobId from this projected content. A new end-to-end summary regression exposed
field-order loss in the first bounded implementation; these control fields are
now projected before diagnostic detail, preserving existing identity behavior.
Seven budget/control fixtures plus material/video fixtures pass 24/24 on Node
22.22.2. Nested package 1/1, ratchet and diff pass. Evidence: tool-summary-before/
after/final.log, tool-summary-control-before/final.log and
tool-summary-control-package/ratchet/diff.log. The latest full Node 402/1-skip
checkpoint predates this focused change.

The budget applies to each compacted tool-content value, not to the number of
SSE frames/tool returns or the final worker output. This is size/prototype safety,
not credential redaction. Provider diagnostics, aggregate output, profile-copy
budgets and whole-worker lifecycle still require work; release remains pending.

## Stream admission and incremental tool-return parity

Two isolated page-flow fixtures reproduced a missing video job when creation or
confirmation delivered an event:part tool return. The serialized page parser
now feeds the existing wrapped part into its message projections, preserving
the jobId control field through status polling to the completed media URL.
This accepts the same {part: ...} wire envelope already handled by the Node
parser; no evidence justified adding a speculative direct-part envelope.

Both Node and serialized page SSE parsers now admit at most 4096 frames and
65536 lines. Lazy line iteration avoids allocating a full split array before
admission, including excessive data lines within one frame and ignored comments.
Excess produces fixed 502 producer_browser_stream_too_complex diagnostics.
Existing transport byte caps remain in force; the exported parser itself does
not establish a separate byte cap. Independent review identified an inherited
CR-only delimiter gap; a failing fixture and CR/LF/CRLF-aware lazy iteration
close it consistently with the lower-level browser transport.

Evidence under target/effective-line-evidence/producer-worker: page-part-before
has the two expected failures, stream-budget-before six, and stream-cr-before one.
Final focused transport/stream/video fixtures pass 44/44. Full Node 22.22.2
passes 420 with one POSIX-only skip; nested package 1/1, ratchet and diff pass.
Owners are conversation-stream 225 and page-conversation 342 effective lines;
test owners are 51 and 104. All are UTF8 without BOM. No release was built.

These limits bound frame/line material, not per-frame JSON width, every emitted
summary collection, whole-process memory or total worker execution. Existing
trailing data-whitespace normalization is unchanged. Output/diagnostic budgets,
credential redaction and aggregate lifecycle hardening remain open.
