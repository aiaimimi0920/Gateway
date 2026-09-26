# Lane G: Udio production browser worker

Owner: coordinator after partial default-agent submission.
State: structural_green; first lifecycle hardening verified; remaining audit open.
Date: 2026-09-08. Entire optimization goal remains active.

## Scope and counts

Original scripts/udio-browser-worker.mjs:1754 effective lines, clean before lane.
Current entry476 effective lines; twelve cohesive module owners:

| scripts/udio-browser/ | Effective lines | Responsibility |
| --- | ---: | --- |
| request.mjs | 110 | Input/default normalization and validation |
| responses.mjs | 166 | Error, challenge and media-result contracts |
| browser.mjs | 123 | Browser discovery and borrowed/owned page selection |
| native-flow.mjs | 175 | Native Create interaction and checkpoint flow |
| transport.mjs | 116 | Bounded page fetch and challenge retry deadline |
| diagnostics.mjs | 48 | Metadata-only best-effort diagnostic append sink |
| captcha.mjs | 321 | Captcha refresh and page widget lifecycle |
| auth.mjs | 138 | Cookie/CDP/localStorage token resolution |
| storage.mjs | 178 | Runtime state persistence and singleton object client |
| local-state.mjs | 91 | Local object-key containment and atomic file replacement |
| state-body.mjs | 56 | Bounded serialized runtime-state collection |
| state-scope.mjs | 28 | Provider-scoped state export from shared contexts |

No new file exceeds500. Main retains browser ownership and output protocol;
library modules do not exit the process. Dependencies are acyclic.
No existing manual Udio/probe-AI-Studio source or S06 Rust source was changed.

## Baseline and extraction proof

The initial implementation agent submitted auth/storage extraction only, omitted
the hasPersistableRuntimeState binding and copied normalizeString into both
modules. Coordinator repaired these before acceptance and completed all remaining
boundaries. Its shell-pipe malformed-stdin observation was not accepted as
accurate supplied-input proof; direct spawnSync inputs now exercise real JSON.

The coordinator reconstructed the untouched starting source from HEAD (the file
was independently verified clean before dispatch). It is saved under
target/effective-line-evidence/udio-worker-split/before.mjs.
Eight helper tests run against declarations from that original source without
executing main; a ninth test exercises safe invalid input in a real subprocess.
All nine passed before coordinator extraction and after extraction.

AST proof at the pure-move checkpoint:55 original/current/unique function bodies,
zero missing/changed bodies. Main and every embedded browser callback were exact.
The following explicit lifecycle hardening is separate from that checkpoint.

## Verified lifecycle fix

The existing success paths invoked process.exit inside main's try block. That
terminated the process before the finally block could persist refreshed state
or close an owned browser.

A real subprocess with a mocked browser first reproduced the bug: response was
successful but the lifecycle trace was[], rather than[persist, close].
Main now returns its existing JSON payload; main().then(printJsonAndExit) emits
and exits only after finally. The exit status and JSON schemas remain unchanged.
The borrowed CDP browser is deliberately not closed.

Seven subprocess lifecycle cases now pass: owned success, request failure,
state-persistence failure, borrowed success, launch failure, context-creation
failure and browser-close failure. Error-swallowing policy for state persistence
and close is preserved. No genuine browser was launched.

## Fresh gates

- Node focused suite16/16 passed.
- Python Udio source contracts7/7 passed. Existing assertions were retained and
  redirected to explicit owner files, not replaced by permissive concatenation
  of every module or deleted.
- Node syntax checks passed for entry, nine modules and test.
- Checker tests19/19 and adoption ratchet passed.
- Strict still exits1:1034 scanned,44 hard+77 mandatory=121 above700,38 soft.
  Original debt cleared24/145 (16.6%); this is not full S11/S21 completion.
- Independent read-only review found no missing imports or dependency cycle.
  Its lifecycle coverage concerns were addressed with the seven-case matrix.

Evidence: target/effective-line-evidence/udio-worker-split/final-node.log and
checker-tests.log, ratchet.log, strict.log. Python command:

    python -m unittest discover -s tests/python -p test_udio_browser_worker_contract.py -v

Focused Node command:

    node --test scripts/tests/udio-browser-worker.test.mjs

## Open hardening audit (do not mark lane complete)

| Owner | Verified protection | Remaining work |
| --- | --- | --- |
| Entry/request | Rejects invalid JSON shapes, missing auth and invalid target assets | Bound stdin; audit cancellation/deadline through persistence |
| Storage | Singleton client; empty state cannot overwrite authenticated state; local containment, atomic replacement and16MiB serialized-state limit verified | Bound serialization allocation and S3 deadlines; configured root/parents require trusted local ACLs against concurrent directory replacement |
| Auth/browser | Cookie fallback detaches CDP; borrowed selection uses exact origin/path; export/auth-cookie/localStorage scope verified | Real CDP validation; audit later navigation and cancellation |
| Responses/transport | Error preview8000 chars, retry callback once with deadline, target assets required; page JSON fetch bounded16MiB | Native Playwright response collection and total deadlines remain open |
| Diagnostics | Disabled sink returns without I/O; fixed metadata projection omits free text, URLs and secrets; serialization/I/O cannot fail worker | Log-file growth/rotation and operator-controlled path trust remain open |
| Captcha/native-flow | Widget-owned operations and finite retry counters retained | Audit script-load timeout, response bounds and failure cleanup |

No real credential/network/provider/profile/runtime was used in verification.
No dependency or policy/baseline change, Cargo invocation or release mutation.
The last immutable release predates this lane. Integrate only after the next
explicit shared source/docs freeze and a new build/package/runtime gate.

## Next goal work

Continue the open hardening items with failing regressions before fixes.
Lane H AI Studio worker remains source-unchanged: its agent returned localization
only, no extraction. It is reserved for coordinator continuation, not accepted.
S06 continues its independent ingress/diagnostic work and retains Cargo.

## Local storage hardening checkpoint

The next goal turn reproduced four real failures before fixing them: local reads
escaped root through parent traversal, writes overwrote outside files, ambiguous
keys were accepted, and an existing nested junction redirected storage outside.
Baseline storage suite1/5 passed,4/5 failed as expected.

Local I/O now has a dedicated88-effective-line owner. It rejects absolute/parent/
empty/dot segments, Windows separators/ADS/device names and ambiguous names.
The configured root is canonicalized (a trusted NAS root junction is allowed),
while descendant symbolic links/junctions and unexpected target types fail
closed. State writes use an exclusive0600 temporary file, flush/close then rename;
an existing hard-linked outside inode is not overwritten. Cleanup removes only
a temporary file whose exclusive creation succeeded.

Seven storage tests and16 worker tests pass together23/23; Python contracts7/7,
syntax, ratchet and global Gateway diff check pass. The three changed/new files
are UTF8 without BOM or embedded NUL bytes. Evidence:
storage-before.log, storage-final.log and storage-python.log under the lane
evidence directory. No production storage directory was accessed by these tests.

Remote S3 key semantics remain unchanged. Reads still return null on invalid/
missing state; persistence errors remain best effort at the worker boundary.
This guards malformed keys and already-existing descendant links, not a hostile
local actor swapping directory entries between checks and I/O. Configured root
and its parents must remain protected by operator-controlled filesystem ACLs;
portable Node path APIs do not provide a cross-platform directory-handle walk.
The following checkpoint adds runtime-state byte bounds; other audit rows remain open.

## Serialized state size checkpoint

Runtime state now has a16MiB serialized-byte limit, distinct from media assets
and total process memory. Two new regressions first failed: an oversized local
file was read, and an oversized replacement overwrote the previous state.
Both now pass. Local reads reject oversized file metadata before collection and
also enforce the limit incrementally, covering growth after the metadata check.
Writes reject oversized serialized bytes before local publication or S3 upload.

The shared body collector handles Node SDK async streams without calling the
unbounded transformToByteArray conversion. It retains fixed64KiB blocks, so
one-byte chunks cannot produce millions of retained chunk objects. Empty chunks
are skipped; malformed chunks and overflow close the async iterator. An adapter
offering only an unbounded conversion method fails closed. Tests use synthetic
streams, not a live S3 service; no remote compatibility certification is claimed.

Fresh combined Node matrix31/31 passes:16 worker,9 local storage,6 stream-body
tests. Python Udio contracts7/7, checker tests19/19, adoption ratchet and scoped
diff check pass. Evidence: body-before.log (2 expected failures), body-final.log
(31 passes), body-checker.log and body-ratchet.log in the lane evidence directory.

This bounds serialized bytes collected/published, not all allocation: browser
storageState, JSON stringify/parse, source-provided chunks and the final buffer
copy still allocate. S3 deadlines and other open audit rows remain unfinished.
Local ACL trust requirements from the previous checkpoint are unchanged.

## Shared-context export checkpoint

A failing regression confirmed that borrowed-context persistence exported other
sites' cookies and localStorage. Export now projects only provider-matching cookie
domains and exact origin entries before serialization and auth-preservation checks.
Main passes the configured base URL; custom origins and ports remain supported.
Host-only/domain-cookie distinction, secure cookies, IP suffix refusal and foreign
partition refusal are explicit. Unknown top-level storage exports are omitted.
Provider auth refresh is retained rather than disabling all borrowed persistence.
Unrelated auth cannot overwrite the prior authenticated provider object; neither
the original context state nor the existing input object is mutated.

The tests use synthetic Playwright-shaped cookies/state, not a real profile or
remote storage. Cookie-domain projection assumes browser-produced domain validity;
it is not a replacement for a public-suffix validator or token authorization.
The full context is still allocated by storageState before projection, so this
does not establish total-memory bounds. Selection and auth fallback remain open.

Fresh matrix36/36 passes (16 worker,11 storage,6 body,3 scope); Python7/7, ratchet,
syntax and Gateway diff check pass. All five changed/new source files are UTF8
without BOM/NUL. Evidence:origin-before.log (confirmed failing export case),
origin-final.log, origin-python.log and origin-ratchet.log in the lane directory.
No new Cargo build or release; last immutable version predates this checkpoint.

## Borrowed selection and token scope checkpoint

Two real subprocess regressions first failed: a lookalike URL context preceding
the valid one was selected, and a missing provider page still selected/persisted
an unrelated context. Selection now searches all contexts for an exact same-origin
target/fallback path and fails without creating a new context or using the first
unrelated account. Borrowed browser ownership/cleanup behavior remains unchanged.

Two further regressions reproduced foreign-domain same-name auth-cookie use and
permissive chunk-name parsing. Cookie fallback now applies the same provider
domain/secure/partition scope as export. Auth chunks require exact names, safe
integer indices contiguous from zero, no duplicate index, and no leading-zero or
suffix aliases. Existing valid unchunked/chunked tokens still work. LocalStorage
fallback checks origin inside the browser callback before reading any entry,
covering page navigation between worker selection and callback execution.

Fresh combined Node42/42, Python7/7, checker19/19, ratchet and diff pass. The old
Python source assertion requiring startsWith was replaced with exact-scope and
no-first-context assertions; the subprocess regressions prove behavior rather
than merely matching source. The CDP cookie fixture now contains a real-shaped
provider domain; the prior fixture omitted a mandatory browser-cookie field.
Evidence:selection-before.log (2 failures), auth-before.log (2 failures),
auth-final.log, auth-python.log, auth-checker.log and auth-ratchet.log.
No live session, credential store, provider request or shared Cargo was used.

## Diagnostic metadata checkpoint

Two failing regressions proved credentials/provider text were written verbatim and
cyclic details could escape the best-effort logging boundary. The sink now projects
a fixed set of finite nonnegative numeric counters, booleans and explicit small
enums. Arbitrary error code/message/body, URLs, paths, model strings, provider
statuses, identifiers and nested values are omitted, not regex-redacted. Track
arrays contribute only trackCount. This intentionally prioritizes privacy over
free-text debug detail while retaining stages, status, timings and lifecycle flags.
The public stdout/error protocol is unchanged.

Metadata extraction does not invoke getters or traverse input objects. The entire
serialization and I/O path is best effort. New diagnostic files request0600;
existing files and Windows ACLs remain operator-owned. Entries have a fixed field
budget independent of payload size, but append-file growth and total filesystem
capacity are not bounded by this checkpoint. Stage names come from internal
literal call sites and are capped to80 lowercase/underscore characters.

Fresh all-Udio Node matrix54/54 passes (including9 manual-helper tests), Python7/7,
checker19/19, ratchet, syntax and diff pass. Three focused regressions cover secret
omission, cyclic/getter safety and large input with bounded entry size. Both changed
files are48 effective lines, UTF8 without BOM/NUL. Evidence:diagnostic-before.log,
diagnostic-final.log, diagnostic-python.log, diagnostic-checker.log and
diagnostic-ratchet.log. No real secrets/network/runtime or release was touched.

## Page JSON response bound checkpoint

A regression reproduced unrestricted page-fetch response collection. The browser
callback now consumes a reader with a16MiB byte ceiling instead of response.text.
Fixed64KiB decoding blocks bound retained chunk bookkeeping; UTF8 sequences split
across input/block boundaries retain their text. The callback has no imported
runtime dependency because Playwright serializes it into the browser realm.

Oversize responses become502 transport failures with a fixed diagnostic message;
stream read failures become504 transport failures rather than successful empty
bodies. Normal absent bodies still return empty text. Readers are cancelled on
failure and always release their lock; the existing abort timeout is cleared.
This bounds collected response bytes, not browser/network allocation or the
complete request lifecycle. Native Playwright response.text paths are separate
and remain open; this is not a whole-worker memory or cancellation guarantee.

Fresh all-Udio Node59/59, Python7/7, ratchet, syntax and diff pass. Five focused
cases cover overflow/cleanup, UTF8 boundary decoding, stream failure, absent body
and tiny/empty chunks. Source116/test62 effective; both UTF8 without BOM/NUL.
Evidence:transport-before.log, transport-final.log, transport-python.log and
transport-ratchet.log. No real browser/network or existing release was touched.
