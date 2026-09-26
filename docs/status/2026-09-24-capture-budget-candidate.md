# Program-handle capture budgets

Status: applied to active scripts after the old provider matrix was cancelled
because it accepted empty test filters. Candidate proofs are retained below;
fresh active Python and Node gates pass. The integration and corrected matrix
are tracked in [the verifier checkpoint](2026-09-24-line-filter-validation.md).
The candidate remains under
`target/effective-line-evidence/20260924-integration-closure/capture-candidate/`.

## Implemented boundary

The standalone program-handle capture has a per-state budget for input processing,
evidence retention and pending body/cookie reads. Page adoption preserves the same
budget and outstanding reads. A stopped owner cannot publish a late result or a
queued asynchronous failure into the adopted state.

Default limits are 4 MiB per input string, 64 MiB cumulative input, 8 MiB per
evidence charge, 64 MiB cumulative evidence charges, 1,024 charges and eight
concurrent native reads. Injected test limits may only reduce these values.
Evidence charges include complete source text, headers and derived hints/pairs;
depth and node limits bound traversal before escaped JSON measurement. These are
logical accounting limits, not a claim of exact process-heap usage.

Exhaustion records a payload-free error, removes this capture's listeners and
causes the execution owner to reject successful handle/summary publication. The
execution owner checks capture health around each snapshot and before final
output; its existing `finally` closes the context. Existing evidence is not
evicted by the new budget. The earlier RPC retention/preview behavior remains
unchanged for traffic within the new limits.

## Explicit remaining native-read limitation

Playwright's `response.text()` has no per-call byte limit or abort signal at this
boundary. The candidate bounds admission before starting reads, then bounds
processing/retention after the promise settles. It does not bound one native body
allocation. Native reads remain charged until they settle, including after a page
switch. `stop()` removes listeners and blocks publication; native cancellation
still relies on page/context closure. This candidate does not close body-allocation
hardening or the entire S18 acceptance gate.

Replacing capture transport requires separate evidence that request identity,
response ordering, page adoption and provider protocol text remain intact. A
post-read string-length check must not be described as an allocation bound.

## Verification and merge conditions

Evidence:
`target/effective-line-evidence/20260924-integration-closure/capture-proof-2026-09-24T14-35-01-830Z/`.

The unchanged twelve original suites plus fourteen new boundary cases pass
254/254 across thirteen suites. New cases cover UTF-8 and cumulative bytes,
escaped JSON, structure limits, admission reuse, shared body/cookie admission,
page adoption, late failures, listener cleanup, secret-free errors and rejection
of successful output after exhaustion. All six changed/new files pass syntax,
strict UTF-8/no-BOM and effective-line checks. All 428 active script hashes still
match the snapshot used to prepare the candidate.

| Candidate file | Effective lines |
| --- | ---: |
| `gemini-canvas-program-handle-capture-budget.mjs` | 81 |
| `gemini-canvas-program-handle-network-capture.mjs` | 248 |
| `gemini-canvas-program-handle-execution.mjs` | 448 |
| `tests/gemini-canvas-program-handle.fixtures.mjs` | 58 |
| `tests/gemini-canvas-program-handle.network-fixtures.mjs` | 19 |
| `tests/gemini-canvas-program-handle.capture-budget.test.mjs` | 132 |
| `../tests/python/test_gateway_nested_worker_package_contract.py` | 240 |

The candidate initially lacked installed Node dependencies; those import failures
were environment failures. Existing installed dependencies were copied into the
candidate, with no active dependency or lockfile change. No provider was called.

The package contract now also includes the new budget module and imports the
packaged network-capture owner. That import verifies the new transitive dependency
is present; the fixture still checks module bytes, manifest records and checksums.
This is a seventh candidate file, with its active-source hash preserved separately.

The old matrix released its source snapshot after controlled cancellation and
successful preservation/cleanup checks. All seven active files now match the
validated candidate after line-ending normalization. Syntax, checker/ratchet and
diff gates pass. Active Python passes 295 tests with four opt-in skips; active
Node passes 1,959 with one platform skip at test-process concurrency one. The
package contract passes in the full Python command, and the production audit
reports zero vulnerabilities. The native-read
limitation remains open until a separately tested transport solution closes it.

## Real browser boundary evidence

`capture-browser-2026-09-24T15-01-07-011Z/summary.json` records five passing
loopback-only Chromium cases against the candidate owner: normal text, known-size
oversize, chunked oversize, gzip decoded oversize and concurrent stalled reads.
Small text retains its protocol hints. Oversize results are not published, the
owned listeners detach, and limit errors contain no request URL. The concurrency
case starts only the first pending body. In every case all started native reads
settle after context closure and stopped state stays unchanged. Owned sockets
finish at zero. This validates context-owned cancellation for these fixtures;
`stop()` alone still does not abort the native read.

`cdp-body-bound-2026-09-24T14-54-20-775Z/summary.json` is a separate transport
experiment. A fresh CDP session with a 64 KiB resource buffer rejects five finite
oversized bodies (known length, chunked, gzip, Unicode and binary) before returning
their body to that session. Small text and a Windows-1252 sample have the same
decoded hashes as Playwright. Simultaneous Playwright reads still return the full
large fixtures: the additional session's limits do not constrain the existing
`response.text()` implementation. No production CDP replacement is applied.
Request identity, redirects, cross-origin frame/worker targets, page adoption and
ordering need separate proof before such a replacement can close the native-body
gap. The experiment does not measure an exact browser or Node heap cap.

The later [native transport validation](2026-09-24-native-body-transport-validation.md)
finds two additional constraints. A page-only CDP session misses cross-site iframe
and dedicated-worker responses; an isolated child-target observer matches all nine
fixture responses. More critically, memory-cached scripts bypass the configured
resource buffer, including gzip without `Content-Length`. A decoded-byte check
before the body command rejects all six oversized cache/network/Service Worker
fixtures while retaining both small controls. The production transport remains
unchanged. The later [native response candidate](2026-09-24-native-response-candidate.md)
adds tested lifecycle, missing-byte, decoding, cancellation and OOPIF ownership
boundaries. It passes 290 program tests and records 23 browser scenarios, including
an explicitly unsupported in-flight page-adoption case. Integration remains open.

The all-suite candidate runner is `run-capture-acceptance.mjs`. Its first run
passed 1,958 tests and failed one checker contract because the isolated copy lacked
AGENTS/README/CI inputs. Those four files were copied unchanged. The next run
passed 1,957 tests but hit two unchanged browserless fixture VM timeouts at the
existing one-second limit. Both failed records remain under `capture-acceptance-*`.
The final run limits test-process concurrency to two; no test timeout or assertion
was relaxed. `capture-acceptance-2026-09-24T15-01-52-500Z/summary.json` records
1,959 passed, zero failed and one platform skip across all 1,960 Node tests.
The packaged-module contract passes 1/1 and the checker suite passes 19/19.
All seven candidate files pass their effective-line and UTF-8/no-BOM checks;
all six JavaScript modules pass syntax checks. The 491 protected active-source
and contract-input hashes remain unchanged. This candidate evidence is separate
from the active-source Node gate.
