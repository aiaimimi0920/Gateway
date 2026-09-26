# Lane J: ChatGPT session worker

## Ownership and bounded checkpoint

Coordinator owns the entry, `scripts/chatgpt-web-session/`, focused Node tests
and this record. S06 Rust/Gemini source and Cargo remain separately owned.
GWP-20260908-05 is a requested integration window, not an acknowledged freeze.

The first checkpoint is a pure extraction, not completion of this large file:

- Entry: original 3577 effective lines, now 3285 after readable import wiring.
- `configuration.mjs`: 113, input aliases, booleans, timeout and launch options.
- `profile.mjs`: 183, executable/profile discovery and profile materialization.
- `errors.mjs`: 16, worker error creation and public serialization.
- No provider/network behavior, credentials, dependencies or UI were changed.
- Main still owns orchestration and eventual profile cleanup. Profile functions
  retain their original API; extracted modules do not import the entry.

## Evidence

Ignored evidence: `target/effective-line-evidence/chatgpt-profile/`.
`before.mjs` was copied from the actual pre-edit worker. Its helper suite passed.
TypeScript AST comparison of all top-level function bodies across the entry and
three new modules found original119/current119/unique119, zero changed bodies.
Constants were moved verbatim; only export/import wiring changed.

Fresh focused run: 11/11 (six new profile/config/error cases plus five existing
relay helper cases). Tests exercise real temporary directories, allowlisted
session file copying, source preservation, missing-profile errors, fresh profile
creation, configured discovery, input normalization and error serialization.
No browser or external provider is launched by these tests.
Entry syntax, checker19/19, ratchet and scoped whitespace checks pass. Default
focused runtime is Node20.20.2; this is not evidence for the declared Node22 gate.

## Remaining work and explicit risks

The entry remains hard debt; this checkpoint does not reduce the119-file debt
count or complete lane J. Login, mailbox, relay, credential and cookie owners
still require cohesive extraction and protecting tests.

The preserved profile implementation does not validate profileDirectory against
parent/absolute paths, bound recursive copying or reject directory links. A copy
failure after temporary allocation can leave that root behind. These are open
hardening tasks, not silently fixed or waived by pure-move evidence. Proxy and
auth seed values remain sensitive; tests use only synthetic fixtures. No full
browser lifecycle, cancellation, live provider or release acceptance is claimed.

All J edits postdate immutable release20260908-udio-worker-s06-051500. A new
integrated build must wait for an explicit shared source/docs freeze and Cargo
handoff; no old package or live4200 service was modified.

## Subsequent profile safety checkpoint

Separately from the exact-move checkpoint, profile materialization now rejects
non-child names, Windows special names/streams and linked descendants. The
operator-selected root is resolved once and may remain a legitimate NAS junction.
Failed clone/fresh construction removes only the temporary root it allocated;
successful roots still transfer to main's existing lifecycle ownership.

Three safety regressions failed against the unmodified extracted implementation.
After the fix, all14 focused tests pass. The cleanup regression runs the real
module in a bounded child with an isolated temp directory and asserts that it is
empty after linked-subtree rejection, avoiding cross-test temp census races.
Ratchet and scoped diff pass; independent read-only review found no concrete
regression in these boundaries. This is not a hostile-source race proof: trusted
source/temp ACLs are still required, and total recursive copy size/depth/time,
cleanup failures and full browser lifecycle remain open. Root allowlisted file
links and deeper links use the same check but lack individual focused cases.

Evidence: safety-before.log (three failing regressions), safety-final.log
(14/14), safety-ratchet.log. The original119-body equality proof applies only to
the earlier extraction, not this deliberate behavior change. This is unreleased.

The first failing baseline test run created three synthetic temporary roots:
`chatgpt-web-profile-e2VCuv`, `chatgpt-web-profile-fresh-16Aldf`, and
`chatgpt-web-profile-u9v32e` under the current user's Windows temp directory.
Their exact contents were inspected; a scoped cleanup command was rejected by
execution policy and was not retried through another route. They remain a known
test-artifact cleanup item, not leaked live browser processes or real profiles.

## Credential and cookie extraction checkpoint

The next pure move reduces the entry3285 ->2987 effective lines. Credentials
materialization now belongs to `credentials.mjs` (218), cookie shaping and expiry
parsing to `cookies.mjs` (92). DEFAULT_BASE_URL moved once into configuration,
shared by main and credential output. Filesystem-only imports left the entry
where no longer used. Profile safety changes from the preceding checkpoint remain.

Before/after AST comparison for this checkpoint found103/103 unique top-level
function bodies, all identical. No compatibility forwarding or new dependency
was introduced. `credentials-before.mjs` records the actual dirty entry boundary.
The six new material tests cover cookies containing equals signs, replacement
deduplication, tolerant JWT expiry parsing, latest request material precedence,
input non-mutation, filename contracts and real synthetic credential file writes.

Fresh verification: focused20/20; full Node discovery210/210; checker19/19;
entry syntax, ratchet and scoped diff pass. Full Node output is `all-node.log`
in the lane evidence directory. These remain offline/default Node20 checks.

The existing credential writer still accepts operator-selected paths, writes
non-atomically without an explicit private mode, and retains rawSource debug
material. Conversation capture selects by pathname without origin authorization.
Cookie normalization and JWT expiry parsing are not authentication validation.
These unchanged contracts require separate hardening rather than being hidden
inside the exact move. Entry2987 is still hard debt; no debt-clearance percentage
or full lane acceptance is added. This checkpoint also awaits the shared release
window and is not contained in any existing immutable release.

## Mailbox decomposition checkpoint

Entry2987 ->2303. Five cohesive mailbox owners now separate reference decoding
(100 effective lines), code/marker parsing (72), recovery/poll orchestration
(126), service snapshots (182), and the IM215 adapter (208). Shared configuration
is131; the existing sleep primitive has a3-line timing owner. Dependency direction
is polling -> snapshot -> provider -> parsing, with reference decoding shared by
snapshot selection. None imports the entry and no cycle was introduced.

A read-only scout localized boundaries; the coordinator read the exact source
and performed the move. Two dependencies omitted by the scout's initial inventory
(IM215 normalizeBaseUrl and snapshot parseMailTimestamp) were preserved explicitly.
Current92/92/unique92 original top-level function bodies are unchanged at this
checkpoint, proven by AST against `mailbox-before.mjs`. Chinese verification-code
patterns remain literal UTF8, not escaped or replaced.

Eight new offline tests exercise reference precedence, contextual code parsing,
session/provider filtering, newest snapshot/marker-only behavior, credential-set
selection, IM215 key formats and detail loading, recovery fallback/non-mutation,
and direct immediate/poll success. Network calls are mocked; no real mailbox,
OTP or provider credential was used. Full Node218/218, entry syntax, ratchet and
scoped diff pass; evidence is mailbox-tests.log/mailbox-all-node.log and
mailbox-ratchet.log. The prior helper suites remain included.

This preserves existing mailbox behavior, not a security acceptance: service and
provider fetches lack bounded response bodies and abort deadlines; polling's
wall-clock check cannot interrupt a hanging fetch. Code extraction is heuristic,
not sender authentication. Snapshot/config selection and recovery freshness still
need explicit trust-boundary review. No new registration flow was added. Entry
remains2303-line hard debt; release and full lane acceptance are still pending.

## Login, auth navigation and page-state checkpoint

The coordinator extracted existing account login (438 effective lines), auth
navigation/OAuth readiness (308), and page-state observation/classification (96).
The entry decreases2303 ->1471, now mandatory debt rather than hard debt; it still
exceeds700 and is not accepted as a completed extraction. The dependency graph
is login -> navigation -> page-state, with mailbox polling imported only by the
login owner. safePageUrl belongs to page-state, not an entry-importing utility.

The exact52/52/unique52 top-level function bodies match `login-before.mjs`.
The coordinator personally read all moved source after a read-only localization
pass; existing browser evaluate callbacks and Chinese selectors remain unchanged.
Eight focused tests cover real serialized page-state callback evaluation, preview
limits, classifiers, missing login seed, OTP field shapes, non-verification skip,
first ready navigation, cookie readiness, and exact button/fallback behavior.
Full Node226/226, focused8/8, syntax, ratchet and scoped diff pass.

Strict snapshot1078 scanned:42 hard,77 mandatory,38 soft;119 remain above700.
Strict exit1 is expected and does not establish goal completion. Evidence:
login-tests.log/login-all-node.log/login-final-ratchet.log/login-strict.log.

Preserved risks: login navigation/auth links and returned OAuth URLs require exact
origin checks before entering secrets; home recovery uses a URL prefix; compact
page-state errors contain raw snippets. Individual navigation/cookie waits do not
prove one total deadline, and locator handle cleanup is not yet independently
verified. These are explicit follow-up hardening gates, not claims that pure
extraction makes the authentication flow secure. No live account/browser test or
new package was run; GWP05's requested shared freeze has not been acknowledged.

## UI relay and diagnostic capture checkpoint

Entry1471 ->1043. UI relay orchestration has a287-line owner; request/response
capture has112; settled navigation30 and tolerant JSON parsing7 are separate
leaf responsibilities. Both main and UI recovery import the same navigation
function. The unchanged existing assistant/auth helper module is imported by the
UI owner, with no reverse dependency on the worker.

All34/34/unique34 checkpoint function bodies match ui-relay-before.mjs exactly.
Six focused cases cover prompt selection, no-prompt failure, actual serialized
assistant callback deduplication, navigation timeout clamping, paired capture and
event listener detachment, and existing header redaction. Full Node232/232 passes;
syntax, ratchet and scoped diff pass. Evidence is ui-relay-tests.log,
ui-relay-all-node.log and ui-relay-ratchet.log. No live browser was exercised.

Preserved capture risks remain explicit: record count/body reads are unbounded;
postDataJson retains request material even when raw flags are false; response
reads continue asynchronously; exceptional relay exits may skip finalization.
Only normal finalization listener cleanup is characterized by this checkpoint.
UI submit/editor waits and the nominal90-second loop do not prove a total task
deadline. No security or lifecycle completion is claimed by the pure move.
Entry1043 is still mandatory debt and all changes remain unreleased pending GWP05.

## Structural closure checkpoint (not full lane acceptance)

Final extraction moves probe/bootstrap parsing into255 effective lines, HTTP
relay into164, and existing proof-work construction into139. Browser cookie
import/export moves into its existing134-line cookie owner. Entry1043 ->447
after removing four independently identified unused imports. All22 nested owners
are at most438 effective lines; no exception or baseline change was needed.

The final checkpoint's20/20/unique20 top-level bodies exactly match
final-structure-before.mjs, including the unchanged browser probe callback.
Five added tests execute that callback with mocked fetch, test session-token
precedence, bootstrap parsing, zero/one-iteration proof loop, successful HTTP
relay metadata, and cookie import/export. Full Node237/237, checker19/19, syntax,
ratchet and scoped diff pass. Latest strict1087 scanned:42 hard+76 mandatory=118
above700,38 soft. Structural debt clearance is now27/145 (18.6%), not full S21.

Independent graph review found no missing imports, module cycles or apparent
serialized callback closure mistakes. The reviewer also reported accidentally
importing the executable entry, which runs main; that is not accepted as a
side-effect-free check or valid browser/runtime proof. A fresh coordinator process
census found no matching active Node/browser process for ChatGPT/Gateway. No
claim of live provider success is based on the reviewer's entry import.

Independent package review confirmed recursive script copying and identified a
coverage gap. The existing nested-worker package contract now copies the real22
modules plus entry/helper into its synthetic fixture, runs the official packager,
and verifies every resulting byte, support record and checksum. Focused package
contract1/1 passes. This fixture is not a new product release. The unchanged
Docker COPY scripts directive also includes nested modules; Docker was not built.

Evidence: final-structure-tests.log/all-node/checker/final-ratchet/strict/package
logs in this lane's ignored evidence directory (all-node log uses the full
final-structure-all-node.log name). Remaining security/resource obligations above
still apply. In particular, main calls process.exit before finally can complete,
so lifecycle cleanup must be corrected and proven separately. Source structure
is now green, but hardening, supported-runtime/full lifecycle and integrated
release remain pending; old release051500 does not contain these changes.

## Normal worker lifecycle correction

Main now returns success/error payloads instead of calling process.exit inside
its try/catch. Only the entry's final output call runs after main's finally has
awaited context close, browser close and own profile removal. The deadline timer
is no longer cleared before those cleanup awaits; finally clears it last.
Success/error payloads and exit codes remain unchanged for normal completion.

Six actual child-process tests execute the current worker with dependency mocks
and real stdout/exit behavior. They do not import or launch a real browser and
never remove a real profile. Scenarios: success, rejected context.close, navigation
failure, credential-write failure, browser launch failure and missing executable.
The baseline failed five cleanup-order cases; after the fix all6/6 pass. Each
asserts one result, emitted last after the expected owned cleanup sequence.
Full Node243/243, syntax, ratchet and scoped diff pass; evidence lifecycle-before,
lifecycle-after, lifecycle-all-node and lifecycle-ratchet logs.

This fixes normal/exceptional task-return cleanup, not all process lifecycle:
the existing hard timer can still force exit during a hanging operation or
cleanup; rejected close/removal remain best-effort. No descendant-process kill,
graceful timeout cancellation, bounded stdin or actual browser-tree shutdown
proof is claimed. Those gates remain open and this patch is unreleased.

## UI capture exceptional cleanup

UI relay now encloses the post-capture submit/poll path in try/finally. Its capture
owner exposes idempotent detach, removing both page listeners and clearing the
request-object lookup. Normal finalization uses the same operation. An active
flag prevents late response reads or saved handlers from changing detached
records, so returned summaries no longer mutate after finalization.

A real EventEmitter/mock-page regression first failed because editor submission
failure left both listeners attached. It now passes, along with an explicit late
response/idempotent-detach test. Focused8/8, full Node245/245, ratchet and scoped
diff pass (capture-cleanup-before/after/final/all-node/ratchet evidence logs).
Protocol response bodies and normal result envelopes are not modified.

This does not cancel an already pending Playwright response.text operation or
bound its allocation; total records, captured request bodies and diagnostic
secrets remain separate open audit work. No actual browser, provider or release
validation was attempted; this hardening remains outside immutable release051500.

## Diagnostic bodies are opt-in

Default UI diagnostics no longer call request.postData or response.text at all.
Request JSON/preview are null unless the new explicit
`CHATGPT_WEB_UI_REQUEST_CAPTURE_RAW_REQUESTS=true` flag is set. Response body
JSON/preview/length are null unless the existing RAW_RESPONSES flag is set;
unknown body length is not misreported as zero. Summary includes rawRequests.
The actual provider request, relay response and credential header capture are
unchanged. Operators enabling any RAW_* flag must treat resulting diagnostics
as sensitive data, not ordinary logs.

Two regression cases failed before the patch because default diagnostics read
private synthetic message bodies. Current focused10/10 and full Node247/247
pass, plus ratchet and scoped diff. The request test also proves explicit opt-in
still captures synthetic content; late-response cleanup tests now opt into raw
responses to exercise the real asynchronous branch. Evidence:
capture-bodies-before/after/all-node/ratchet logs.

This removes unnecessary default body allocation and private prompt retention,
but does not claim complete diagnostic sanitization: URLs/header values, record
counts, raw opt-in payload limits and output path/permission controls remain open.
No real browser, provider, package or shared build was run in this checkpoint.

## Diagnostic cardinality bound

Capture retains at most the first128 eligible requests. Excess requests increment
a saturating droppedRequests counter exposed in the returned summary; their
headers and bodies are not read. Truncation is explicit, and late conversation
material after that limit must not be treated as fully captured. Actual provider
traffic is not dropped or changed.

A response atomically claims/removes its tracked request before awaiting raw
body data, so duplicate response notifications cannot start additional reads.
Together with the lifetime request cap this bounds tracked request objects and
raw-response read starts to128 per capture. Individual opt-in body sizes and
deadlines, URL/header sizes, and backend Playwright allocations remain unbounded
by this patch; no total-memory bound is claimed.

Two regression cases failed before the fix. Focused12/12, syntax, ratchet and
scoped diff pass after it:10,000 requests retain128, report9872 drops and perform
only128 header reads; duplicate raw responses perform one body read. Evidence:
capture-count-before/after/ratchet logs. Last full Node run remains247/247 from
the preceding checkpoint; this bounded change has focused verification only.
This remains unreleased pending the shared build/source freeze.

### Default diagnostic metadata projection

The default capture previously retained Set-Cookie, API-key, redirect and
arbitrary unknown header values, and entire request URLs. Three new regression
tests failed against that implementation. Default header projection now retains
only validated MIME/charset and numeric content-length values; other values are
redacted metadata. Existing authorization/cookie/token classifications remain.
Projection is limited to64 own fields,128-character names, and32 cookie names
of128 characters parsed from at most4096 input characters. A null-prototype
output prevents prototype-like names from mutating the projection object.

Captured URLs always discard userinfo, query and fragment, keep at most2048
characters of origin/path, and replace inputs over8192 characters or non-HTTP(S)
URLs with a marker. Actual provider requests are unchanged. This intentionally
narrows diagnostic fidelity: multipart/unknown MIME parameters are redacted,
and first64 header projection is not a full wire record.

Five new cases cover secret-bearing default values, bounds, URL stripping,
prototype/inherited keys and oversized URLs. Initial focused15/15 after the
first3 regressions; final full Node254/254, syntax, ratchet and diff checks pass.
Evidence: metadata-before/after/node/ratchet logs under the lane evidence root.
Current ui-capture159 effective lines, new test59; both UTF8 without BOM.

Independent read-only review found no default-value redaction bypass in this
scope. This is not a universal secret scrubber: paths, header/cookie names and
syntactically valid MIME values may still be sensitive. RAW_HEADERS deliberately
retains its explicit opt-in raw behavior, and raw request/response body reads,
backend allocations and read deadlines remain unbounded. No total-memory or
production provider readiness is claimed. GWP05 transfer remains pending and
these changes are not yet included in an immutable release.

### Credential publication replacement

Direct target truncation was replaced with an exclusive random same-directory
staging file, owner-only POSIX mode0600, file sync/close, then rename. Newly
created parent directories request0700. Serialization occurs before staging
creation. Cleanup removes only the current call's staging file; simultaneous
publication/cleanup failures retain both errors in AggregateError.

The baseline hardlink regression failed because direct writing modified the
previous inode as well as the target. The replacement passes it. Windows
concurrent12-writer testing then exposed real EPERM rename contention; bounded
retries now handle only Windows EPERM/EACCES/EBUSY, at most8 attempts and710ms
scheduled delay. Destination deletion is never used as a replacement fallback.
Tests wait for all writers to settle before fixture cleanup.

Evidence in the lane root: credential-before.log (1 failed,1 passed,1 POSIX
skip), credential-after.log, credential-faults.log (exposed Windows contention),
credential-retry.log, credential-final-node.log (259 passed,1 POSIX skip),
credential-final-focused.log (6 passed,1 POSIX skip after adding retry contract),
credential-final-ratchet.log (pass). Fault injection executes only the extracted
publication function with mocked filesystem operations: write/sync/close/rename
failure, cleanup error retention, bounded retry and no destination removal.
Real temporary-file cases cover linked inode preservation, rename failure,
concurrent complete payloads, no normal-exit staging residue and serialization
failure. Credentials250 effective lines; new test116; syntax/diff checks pass.

This is atomic replacement under the filesystem's rename contract, not a crash
durability/ACL claim: parent directory fsync, crash-orphan recovery, Windows
private ACL verification, untrusted-parent/path races and payload size budgets
remain open. Existing directories are not chmodded. No credential material from
real accounts was read or written. GWP05 transfer remains pending; unreleased.

### Bounded worker stdin

The pipe read previously preceded all deadlines and accumulated an unlimited
string. It now accepts at most16MiB raw input and requires EOF within30 seconds,
returning structured input-too-large/input-timeout/premature-close errors before
browser/profile allocation. Accepted bytes use one lazily allocated fixed16MiB
buffer, preventing per-chunk bookkeeping growth for tiny writes; only initialized
bytes are decoded once at EOF. Blank input still maps to an empty JSON object.
All terminal paths pause input, remove the four listeners and clear the timer.

Three baseline regressions failed. Final focused12/12 includes UTF8 split across
individual bytes, empty/exact-limit input, overflow, controlled deadline, stream
error/close, and real child-process structured overflow exit before allocation.
The child harness mocks browser/filesystem dependencies, not stdin/stdout/exit.
Full Node265 passed/1 POSIX skip before the final child-overflow case; ratchet,
syntax and diff checks pass. Evidence: stdin-before/after/node/ratchet and
stdin-final-focused logs. Entry469 effective lines, stdin tests71, lifecycle75.

The deadline test drives a controlled timer; it is not a30-second wall-clock
measurement. Parsing/output string allocations are additional to input storage,
so16MiB is an input-byte limit, not a total-worker-memory claim. The existing
browser deadline remains separate, and forced-timeout descendant cleanup still
needs closure. No shared build/release activity before GWP05 acknowledgement.

### IM215 transport limits and cancellation

Each IM215 request now uses a15-second abort deadline spanning headers and body,
refuses redirect following with mailbox credentials, and reads at most4MiB
decoded transport bytes into fixed storage before JSON parsing. Stream errors
propagate rather than becoming empty mailbox responses. Reader cancellation,
lock release and timer removal run on completion/error. Existing successful
JSON/plain-text payload and API-key header formats remain covered.

Two baseline regressions failed (missing signal/redirect policy and swallowed
stream error); the unbounded infinite-stream baseline was deliberately not run.
The first implementation then failed real loopback stalled-body validation:
the abort signal fired at15 seconds but the acquired body read remained pending,
and the test hit25 seconds. An isolated run reproduced it. Explicit reader
cancellation on abort fixed this boundary; real native-fetch validation now
confirms both rejection and response-socket closure around15 seconds.

Evidence: im215-before/after/node/deadline-isolated/deadline-trace logs retain
the failed checkpoints; im215-cancel-reader.log passes5/5. Final
im215-final-node.log passes272 with1 POSIX permission skip; no failures or
cancellations. Six transport cases include real same-server redirect rejection,
oversized stream cancellation, exact4MiB boundary and byte-split UTF8 JSON.
Ratchet, syntax and diff checks pass. Module240 effective lines, test78.

These are synthetic loopback tests, not IM215/OpenAI provider-live evidence.
Per-request limits do not bound the full mailbox list/detail loop or aggregate
poll deadline, and native/decompression/parsed-object allocations are additional
to retained bytes. Other mailbox transports remain to be hardened. No shared
build/release mutation while GWP05 source/docs freeze remains unacknowledged.

### Shared bounded mailbox service transport

The proven IM215 byte/deadline/reader cancellation mechanism now lives in
`mailbox-http.mjs` (44 effective lines), with no provider-adapter dependencies.
IM215 retains its existing oversized-response classification and text/JSON
return behavior. Code lookup, service snapshot and recover-by-email now share
the same15-second request/body deadline,4MiB response limit and redirect refusal.
Endpoint-specific HTTP status error names remain unchanged; transport read
failures propagate while malformed JSON retains the previous empty-object
fallback. Snapshot177, poll124 and IM215208 effective lines.

Two new baseline tests failed before service wiring. Four new cases now verify
all3 endpoints carry deadline signals and reject redirects, propagate stream
failure, reject oversized bodies, and preserve403 classification. The real
loopback IM215 abort/socket-close regression exercises the shared implementation.
Focused16/16 before the last2 cases; full Node276 passed/1 POSIX skip, no failed
or canceled tests. Ratchet and diff checks pass. Evidence:
mailbox-service-before/after/node/ratchet logs.

The package contract now requires23 nested ChatGPT modules. The official
synthetic packager test passes1/1 and verifies copied bytes, support-file records
and checksums for the new module (mailbox-service-package.log). This is not an
immutable product build. No real mailbox or provider credentials were used.
Aggregate polling/list/detail budgets, cancellation propagation from callers,
and recovery fallback diagnostics remain open. GWP05 transfer is still pending.

### Aggregate mailbox polling cancellation

Shared transport now honors caller signals before fetch, forwards abort reasons
to its controller and removes forwarding listeners on every exit. Polling owns
one aggregate controller/deadline (existing5-second minimum, now finite10-minute
maximum), propagating it through code, snapshot, provider config, IM215 list and
each detail request. Between fallback stages it checks cancellation, and before
accepting returned codes it also checks elapsed wall-clock time. Retry sleep is
abortable and cannot deliberately extend beyond the remaining budget.

Independent review identified the missing public caller-signal hook on the
aggregate polling API; it now accepts optional signal, preserves its reason and
cleans that forwarding listener too. Existing account-login callers still rely
on the owned polling deadline; whole-worker cancellation is not thereby solved.

Eight new cases verify pre-canceled requests, acquired-reader cancellation,
successful listener cleanup, real5-second aggregate expiry at the first request
and at an IM215 detail request (no subsequent fallback/detail starts), explicit
caller abort, pre-canceled polling and rejection of a wall-clock-late code.
Baseline pre-cancellation failed. Intermediate focused12/12 and6/6 runs passed;
final full Node284 passed/1 POSIX skip, no failures/cancellations. Syntax,
ratchet and diff checks pass. Evidence: mailbox-cancel-before/after and
mailbox-budget-after/node/final-focused/final-node/final-ratchet logs.

Current effective lines: HTTP49, IM215211, snapshot179, poll152. Immediate code
lookup/recovery remain separately bounded requests, not aggregate operations.
Blocking CPU work, raw last-error diagnostics and browser hard-timeout descendant
cleanup remain open. No real mailbox/provider call or immutable release mutation;
GWP05 source/docs freeze and build receipt still await acknowledgement.

### Timeout diagnostic minimization

Mailbox polling no longer retains/interpolates an arbitrary transport Error in
its timeout payload. It records only whether an attempt failed and emits fixed
diagnostic text, preserving the504/mailbox-code-timeout contract. The baseline
regression exposed a synthetic key/address in the old message. The same case
also verifies that a failed request crossing the wall-clock deadline cannot
start another fallback, even before the timer callback runs.

Focused9/9 and full Node285 passed/1 POSIX skip; ratchet/diff pass. Evidence:
mailbox-diagnostic-before/after/node/ratchet logs. Caller-supplied cancellation
reasons remain caller-owned and are not normalized by this change. Broader
worker error serialization is not certified secret-free. Still unreleased.
