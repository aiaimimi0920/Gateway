# S06 bounded upload diagnostics

## Implemented policy and resource boundary

Enabled upload diagnostics now run image decode, SHA-256 and optional raw-file IO
in one blocking worker, instead of on the async executor. The synchronous builder
itself is unchanged. An owned Bytes handle shares the selected upload allocation;
there is no additional payload copy. The complete result is awaited before the
first HTTP request and shared by the start/finalize snapshots.

Admission is independent of browser encoding: two process-wide permits, acquired
with try_acquire_owned BEFORE submission. The worker owns its permit and bytes,
including after caller cancellation. At most two jobs are submitted/running;
there is no diagnostics admission queue. Stuck jobs retain capacity rather than
allowing replacement work to grow without bound.

Explicit operational change: optional upload diagnostics are skipped when trace
is disabled, admission is saturated/closed, or the worker returns JoinError.
Uploads still proceed; no synthetic hash or new upload error is emitted. Both
upload snapshots are skipped when preparation returns None. This is a best-effort
diagnostics policy, not a byte-for-byte preservation claim for overload snapshots.
Trace enablement for preparation is sampled before each upload; changing process
environment mid-upload does not retroactively prepare metadata. The existing
snapshot/raw-output gates remain in place and may still suppress actual writes.

Admitted raw capture is awaited before network send. Cancellation does not abort
already submitted blocking jobs; they may finish optional output after caller
drop. No inline fallback, unbounded queue or detached retry is introduced.

## Fresh evidence

- `cargo test --offline --locked --lib gemini_canvas -- --quiet`: 588 passed,
  0 failed, 3 ignored; compile4m06s/tests9.22s. This includes five new admission
  tests, the existing trace-off/on upload parent with14 wire scenarios, and the
  isolated snapshot probe extended to compare real async PNG metadata with the
  original synchronous dimensions/SHA/output-path oracle.
- Five new tests cover disabled/saturated/closed gates, off-executor execution,
  shared Bytes storage, nonfatal worker panic, and cancellation retaining capacity
  until actual worker exit. Channel synchronization on a current-thread runtime
  proves responsiveness; worker completion reports back independently of the
  aborted caller, so assertion failures cannot be lost in a detached JoinHandle.
- Disabled/saturated tests use successful builders returning a marker, not panic
  builders: this API intentionally absorbs worker panic and would otherwise mask
  an accidentally executed test builder.
- `cargo check --offline --locked --all-targets`: exit0,1m47s.
- Formatter, checker tests19/19, ratchet and diff checks pass.
- First compile caught existing sibling test callers of the synchronous builder;
  its existing pub(super) visibility was restored. The failed compile is not a
  successful gate or a product regression fix.

Independent read-only review found no confirmed lifecycle or wire regression;
its test gaps led to the real async metadata comparison. Saturation/JoinError
are helper-tested, not separate end-to-end upload fault-injection scenarios.

## Size and residual scope

| File | Effective / physical |
| --- | --- |
| src/upstream/gemini_canvas_upload_debug.rs | 63 / 72 |
| src/upstream/gemini_canvas_upload_debug_tests.rs | 83 / 90 |
| src/upstream/gemini_canvas_upload_http.rs | 294 / 306 |
| src/upstream/gemini_canvas_trace_process_tests.rs | 299 / 311 |

Latest ratchet scan:1034 files,44 hard+77 mandatory=121 above700,38 soft.
Debt reductions belong to concurrent coordinator lanes, not this S06 leaf work.
No policy/baseline/exception, dependency, ingress limit or release mutation.

This bounds concurrent diagnostic jobs, NOT decoded pixels, heap, codec internal
allocations or execution duration. Enabled post-response JSON sanitization and
file writes remain synchronous. Diagnostic saturation currently has no new metric.
Remaining image normalization/ingress concurrency and pixel bounds, all remaining
S06 leaves, S07-S21, full provider/visual/runtime/release gates are still open.
