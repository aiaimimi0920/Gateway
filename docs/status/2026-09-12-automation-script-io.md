# Credential automation script I/O

Accepted: 2026-09-13 00:36:07 UTC. Gateway-only hardening checkpoint.

The driver entry decreases from 150 to 140 effective lines. The Node entry-path
owner is 36 lines and the I/O owner is 81. The script contract, Rust fixture,
Node fixture and Windows stdin fixture measure 151, 131, 63 and 10. All seven
source/test owners remain below 500 effective lines.

One timeout now covers concurrent stdin writing, stdout reading, stderr draining
and direct-child exit. Input is explicitly flushed before its handle is dropped.
Stdout is checked during reads against the existing 2 MiB response budget, using
an 8 KiB read buffer and fallible allocation. Stderr drains to a sink without
accumulating diagnostics or introducing a new byte quota. Failure and timeout
cancel the local I/O futures, kill the direct child and await its exit. Existing
kill-on-drop protection remains for caller cancellation; no reader task detaches.

The initial twelve Node cases could not start: Node 22.22.2 rejects Windows
verbatim main-file paths with EISDIR. The Node-only path owner converts disk/UNC
spellings and requires the converted path to canonicalize to the same allowlisted
target. Other interpreter arguments, spawn configuration and serialization order
remain unchanged. Ordinary spellings preserve ESM and CommonJS relative imports;
the preserve-symlinks-main workaround failed the CommonJS probe and was rejected.

The launch-only baseline passed 10/12 script cases. Installed Tokio 1.51.0 uses
a 2 MiB Windows pipe buffer and can acknowledge writes before OS delivery. The
blocked/duplex fixtures therefore use 8 MiB input, with a 16 MiB fixture cap;
the closed-input case remains 1 MiB. This calibrated baseline passed 7/12 script
cases, exposing five I/O deadline failures. The original automation 10/10 and
HTTP 8/8 passed throughout all recorded phases.

The first I/O candidate reached 29/30 overall. Its remaining closed-input failure
exposed a fixture problem: Node/libuv retains Windows standard descriptors, and
destroying process.stdin does not close the underlying pipe. Matching libuv
1.51.0 source and three real Node probes confirm this. A Windows native-handle
probe returned ERROR_NO_DATA (232) with matching launcher/child PIDs. The Windows
closed-input fixture now uses the existing PowerShell transport to close its own
native stdin handle without exiting; other cases and non-Windows selection retain
Node. The fixture has no Add-Type compilation, additional dependency or shortened
process lifetime. Its twelve-second watchdog and the three-second decision budget
remain unchanged. All twelve test bodies and assertions are preserved.

With the corrected fixture frozen, restoring the original production I/O again
produced exactly 25 passes/five deadline failures. Restoring the byte-identical
81-line I/O owner passed all 30 tests, including exact-limit output, held-open
oversize output, both duplex directions, blocked input, invalid JSON, nonzero
exit, closed stdin, waiting timeout and cancellation. Original failed snapshots,
logs and unsuccessful probe attempts remain available. The first candidate
metadata listed the path owner twice; later captures use unique paths and retain
that original snapshot rather than overwriting it.

Exact source proof preserves all three driver signatures, the complete HTTP and
response-parsing bodies, the response limit, the frozen path owner and 328
neighboring inputs. Fresh serialized cargo check --offline --locked --all-targets,
scoped rustfmt, both fixture syntax checks, checker 19/19, adoption ratchet,
UTF-8/no-BOM proof and both repositories' git diff --check pass. Compilation
retains three existing upstream warnings. Global formatting still reports only
the two unchanged S06 runtime-mirror files. No matching Node/PowerShell fixture
processes or Cargo/rustc/rustfmt processes remained at the cleanup observation.

Immutable evidence is
target/effective-line-evidence/20260912-automation-script-io/scope.json, SHA-256
240a9e3f0d0c289d6efdfcdb39e2dfcbedcf1c6c223488350efe1df8b3cce0d1.
It binds all source phases, six test runs, compiler/checker/cleanup logs, native
pipe and Node-path probes, inventories and independent Git observations. The
accepted candidate is candidate-v3.json. Strict scans 1,626 files: 29 hard,
27 mandatory and 40 soft; 56 remain above 700. Clearance stays 89/145 (61.4%).

At capture, Gateway has 1,801 dirty entries: 175 modified, one unstaged deletion,
two staged deletions and 1,623 untracked. Neuro has 192: ten modified and 182
untracked. Both HEADs and inherited deletions remain unchanged. Historical
accepted scopes have not been regenerated.

Implementation-line ownership is the next structural review. Descendant process
trees, interpreter wrappers, synchronous path/serialization work, archive races,
scheduler shutdown and lock/state cardinality remain separate. The native stdin
fixture intentionally targets the Windows PowerShell console-stream implementation
used by this transport. S06 ownership, GWP-20260912-01, runtime-profile governance
and final freeze/build transfer remain pending. No release, packaged runtime/UI
acceptance or persistent deployment occurred.
