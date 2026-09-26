# Credential automation script I/O

Owner: parallel coordinator. State: hardening_green. Started: 2026-09-12 UTC.
Accepted: 2026-09-13 00:36:07 UTC.

Scope: src/credential_pool_automation/driver.rs and focused script I/O owners and
contracts. The accepted driver entry is 150 effective lines. The current script
path writes all stdin before starting its timeout or reading stdout/stderr, then
uses wait_with_output to accumulate both output pipes without an in-read bound.

Freeze real child-process contracts before the fix. Cover blocked stdin, stdout
and stderr backpressure, oversized stdout held open, exact-limit output, full
request/response JSON, existing error categories, timeout and caller cancellation.
Fixtures use the existing Node script runtime, unique temporary directories, a
finite watchdog and direct-child PID observations before assertions.

Bound stdout by the existing 2 MiB response budget. Drain unused stderr with
bounded storage. Apply the existing driver timeout across awaited input, output
and process-exit work, and explicitly terminate/reap the direct child on failure.
Preserve allowlisted path/interpreter selection, request serialization, HTTP
behavior, error text, success parsing and kill-on-drop cancellation protection.
Descendant process-tree ownership and synchronous path/serialization work remain
separate. Do not impose a new stderr byte quota or a request-size policy here.

Require paired frozen script contracts, original automation and HTTP contracts,
fresh all-targets, scoped formatter, checker 19/19, ratchet/strict inventories,
source/encoding/neighbor proof and both Git checks. All resulting source and test
owners must remain at most 500 effective lines. Serialize Cargo; do not edit Rust
during a running gate.

S06 retains its implementation/cursor. GWP-20260912-01, runtime-profile governance
and final freeze/build transfer remain pending. No dependency, checker-policy,
baseline, exception, release or persistent runtime changes belong to this lane.

Evidence: target/effective-line-evidence/20260912-automation-script-io/.

The initial frozen run passes the original automation 10/10 and HTTP 8/8, but all
twelve new script cases stop before fixture startup. An independent Node 22
process reproduces EISDIR for the Windows verbatim main-file path returned by
canonicalize. The --preserve-symlinks-main workaround still fails for a CommonJS
relative import and is rejected. Ordinary disk spellings load both ESM and
CommonJS relative dependencies while retaining the working directory.

The Node branch now converts verbatim disk/UNC spellings and requires the result
to canonicalize back to the same allowlisted target. Other interpreter arguments
are unchanged. The 36-effective-line path owner is frozen separately before the
I/O regression baseline; the original frozen snapshots remain intact.

The launch-only run passes 10/12 script cases. Installed Tokio 1.51.0 confirms
Windows ChildStdin uses a 2 MiB blocking buffer and may report write acceptance
before the OS write finishes. The initial 1 MiB inputs therefore do not force
write-side backpressure. The closed-input case also exposes the missing flush.
Calibrated fixtures use 8 MiB for blocked/duplex input and a 16 MiB fixture cap,
while retaining the 1 MiB closed-input case and all twelve test bodies/assertions.
The calibrated files are frozen again before changing production I/O.

The initial I/O candidate passes 29/30. The remaining closed-input case reveals
that Node/libuv retains the Windows standard descriptor after stdin.destroy().
Independent native WriteFile evidence observes ERROR_NO_DATA after an actual
stdin handle close. Only this Windows fixture now selects the existing PowerShell
transport; its native handle closes while the process remains alive. No Add-Type
compiler or new runtime dependency is involved. All twelve test bodies/assertions,
the 1 MiB closed-input request, three-second decision budget and twelve-second
watchdog remain unchanged. Failed Node/Python/native probe attempts are retained.

The corrected fixture is frozen before a fresh original-I/O run: 25/30, with
exactly five decision-budget failures. Restoring the unchanged I/O candidate
passes 30/30. Input write/flush, bounded stdout reads, stderr sink and child wait
run concurrently under one timeout; failures explicitly kill/reap the direct
child. The entry/path/I/O owners are 140/36/81 effective lines; all seven files
are at most 151. Three signatures, two unrelated bodies, the limit and 328
neighboring inputs are preserved.

Fresh all-targets, scoped formatting, both fixture syntax checks, checker 19/19,
ratchet, source/encoding/cleanup proof and both Git checks pass. Strict scans
1,626 files: 29 hard, 27 mandatory and 40 soft; 56 remain above 700. Clearance
stays 89/145 (61.4%). Global formatting retains only the two S06 findings.
Acceptance: docs/status/2026-09-12-automation-script-io.md. Immutable scope SHA-256:
240a9e3f0d0c289d6efdfcdb39e2dfcbedcf1c6c223488350efe1df8b3cce0d1.
The accepted source snapshot is candidate-v3.json; earlier snapshots remain intact.
Implementation-line ownership is next. Overall release and optimization closure
remain open.
