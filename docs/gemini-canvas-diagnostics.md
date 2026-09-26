# Gemini Canvas local diagnostics

These are optional local diagnostic artifacts, not a request replay format or an audit journal.
Do not enable them on a shared/untrusted host without reviewing the data-handling implications.

## Enablement and location

`GEMINI_CANVAS_IMAGE_EDIT_TRACE` enables diagnostics by environment-variable **presence**.
Setting it to `0` still enables it; remove it from the process environment to disable it.

Raw upload JPEGs require a second explicit opt-in:
`GEMINI_CANVAS_IMAGE_EDIT_TRACE_RAW_IMAGES=1`, as well as the trace variable above.
An absent raw-image variable, `0`, `true` or any value other than exactly `1` disables JPEG dumps.
The raw-image variable alone does not enable any diagnostics. JSON and sanitized TXT do not
require this second opt-in.

Snapshot JSON, request-header projections and request/response extras are built lazily after the
trace gate. With tracing absent, these builders, uploaded-reference/seed projections and the
upload-only image decode/hash/debug filename work are skipped. Operational request headers and
bodies are not removed by this gate. Upload request-contract strings and sanitized response-meta
previews are cached lazily: successful trace-off uploads skip their construction, while actual
response errors still force the same diagnostic fields even when tracing is absent. Snapshot and
error consumers share one cached value. Network send/body-read error mapping is unchanged.

Enabled upload diagnostics cache image dimensions, SHA256 and optional raw-image path once per
upload, shared by start/finalize snapshots. The cache is warmed before the HTTP request when tracing
is enabled, preserving raw capture even if sending the request fails. Enabled capture still performs
the existing synchronous image decode/hash and file writing; this is not a zero-cost tracing claim.

The runtime root is the current directory's `.runtime` when present, otherwise the parent's
`.runtime` when present, otherwise the current directory's `.runtime`.

- Trace: `<runtime>/gemini-canvas-image-edit-trace.log`.
- New JSON, response TXT and optional upload JPEG snapshots: `<runtime>/gemini-canvas-debug/`.
- Existing snapshots directly under the old runtime root are not migrated, deleted or rewritten
  by the new snapshot writer. They can contain older, unsanitized data.
- Returned upload debug paths identify the new snapshot location; production upload uses the
  original in-memory bytes, not the optional debug JPEG.

## Data handling

JSON snapshots and response TXT are sanitized and bounded diagnostic projections. They can omit
fields, truncate scalar text and omit oversized/malformed/deep structured values. Response JSON
`bodyLength` still measures the original response bytes, not the sanitized TXT length.

Named credentials and recognized token patterns are redacted, including nested JSON strings and
HTTP(S) URL query credentials/userinfo. URL fragments are omitted. This is not universal DLP:
arbitrary opaque secrets, user prompts, URLs and personal content may remain recognizable.

Upload JPEG diagnostics remain binary image data; textual redaction cannot remove sensitive image
content. Treat them and old artifacts as private. Do not attach an entire runtime directory to a
public issue. Enable the additional raw-image flag only when those bytes are actually needed.

## Snapshot writer limits

- At most 2 MiB per new snapshot, 32 stored files and 16 MiB of stored snapshot data.
- A zero-byte `writer.lock` coordinates cooperating Gateway writers; it is excluded from the file
  count. Lock contention skips optional output rather than waiting.
- Atomic replacement uses one create-new staging file, so temporary peak use can reach 33 data
  files and 18 MiB, plus the lock file. This is not a power-loss durability guarantee.
- Successful replacement publishes a complete new file. Quota, lock, write or rename failure does
  not truncate the previous snapshot. Existing snapshots are not automatically pruned.
- Normal failure cleanup removes only the current writer's staging file after closing its handle.
  A process crash can leave a `.pending-*` file; subsequent quota scans count it conservatively.
  Excess or unexpected contents cause new output to be skipped/rejected, not automatically deleted.
- Names must be a single ASCII `gemini-canvas-...` filename ending in `.json`, `.txt` or `.jpg`.
  Traversal, absolute paths, Windows alternate data streams and nested paths are rejected.
- Directory/entry/lock reparse or symlink checks protect final components. The runtime parent is
  trusted; this is not a sandbox against malicious concurrent parent replacement or external writers.

The trace log has its own non-destructive 1 MiB append cap, separate from these snapshot quotas.

## Production encoder working files are different

Legacy `gemini-canvas-image-edit-browser-source-*` and `gemini-canvas-image-edit-browser-encoded-*`
files are preserved, not swept or reused. New invocations use an exclusive UUID directory under
the canonical OS temporary directory, containing `source.<extension>` and `encoded.jpg`. These are
production working files, not opt-in diagnostics: their bytes can feed the upload even with tracing
disabled. Metadata paths describe the invocation and normally no longer exist after it completes.

Admission precedes a bounded source copy and blocking file preparation. Source above32MiB skips
this optional encoder and retains the existing upload fallback; output consumption accepts only
nonempty regular files up to8MiB, with a bounded read protecting against growth after metadata.
This is not a quota on a misbehaving external process's disk writes. Extensions are whitelisted;
Unix directories/source files use0700/0600, and output reads reject final symlinks/reparse points.
The OS temporary parent and same-user processes are trusted, not an adversarial filesystem sandbox.

At most two leases cover preparation, active/reaping processes and output reads. Blocking tasks,
the supervisor and any delayed reaper retain the workspace lease even if the caller is cancelled.
Cleanup removes only the invocation's two known files and then its empty directory; unrecognized
entries, legacy files and failed cleanup are left untouched. Cleanup is best effort, not crash
recovery or secure erasure. Within a Tokio runtime, final cleanup is dispatched to the blocking
pool and retains admission until its filesystem calls finish. Outside a runtime it runs inline.
Thus slow deletion does not occupy an async worker or create an unbounded cleanup queue; it can
still consume encoder capacity. Runtime shutdown can discard a queued blocking cleanup job and
leave files behind; there is no crash/shutdown cleanup guarantee.

Queueing/preparation elapsed time consumes the45-second process deadline; expired preparation
cannot launch a child. Blocking operations themselves are not preempted by that deadline.
Stdout/stderr are bounded to64KiB each. Timeout/cancellation/error terminates the supervised tree;
foreground reaping can add2 seconds. A delayed reaper retains the tree, workspace and slot until
successful root reaping. Persistent OS wait errors fail closed and can hold both slots indefinitely;
unconditional release would risk deleting files still in use. Gated fault-model tests now cover
foreground expiry and repeated wait errors with real workspace/permit retention; these do not
induce real OS wait faults. Windows child/tree cleanup and an explicitly invoked local Node/browser
PNG-to-JPEG round trip are verified, including decoded pixels, dimensions, checksum and cleanup.
Unix execution, broader photographic quality and packaged-product verification remain pending.
