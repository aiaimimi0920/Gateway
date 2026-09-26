# SSE observation admission acceptance

Accepted hardening checkpoint, captured at 2026-09-12 03:08:06 UTC.
The Gateway plan and final release remain incomplete.

## Behavior and evidence

Usage and completion observers now share a private line-admission owner. Each
observed event is limited to 64 MiB of raw input and 65,536 completed lines,
including its delimiter bytes/line. Limits reset at that observer's event
delimiter and do not apply to the transport chunk as a whole.

Admission happens before copying. Partial-line capacity grows geometrically
within the byte limit. Completed line buffers larger than 64 KiB are released.
Overflow or a failed reservation clears pending parser material and discards
input through the next matching delimiter without retaining discarded bytes.
The last valid snapshot remains available, and later events resume observation.
The original Bytes objects and errors continue downstream unchanged.

The same seven public tests change from four passes/three failures to 7/7.
The old implementation admitted oversized events and excessive empty data fields,
overwriting previous valid usage. The rejected-chunk probe recorded a
67,109,888-byte observer allocation request before the fix. The unchanged final
test verifies that no single request in that probe exceeds 65,536 bytes.
This measurement concerns observer allocation requests during one poll; it does
not measure process RSS or the already allocated upstream chunk.

The contract also covers cumulative admission across chunks, recovery, forwarded
byte identity, fragmented UTF-8/CRLF, invalid UTF-8, observer-specific whitespace
and carriage-return behavior, EOF, and many events in a single transport chunk.
The fixed public contract and test-only allocator helper remain hash-identical
to their pre-fix snapshots. The allocator delegates to System and tracks only
requests on the measuring thread.

Twelve primitive tests pass, covering exact byte/line boundaries, completed-line
accounting, ignored/empty fields, discard recovery within and across chunks,
different CR delimiters, UTF-8, capped geometric growth, large-buffer release and
empty input. No live provider, Redis service or browser was needed.

## Scope and verification

| File | Before effective | Accepted effective |
| --- | ---: | ---: |
| src/upstream/stream.rs | 109 | 110 |
| src/upstream/stream/usage.rs | 217 | 213 |
| src/upstream/stream/completion.rs | 165 | 164 |
| src/upstream/stream/observation.rs | new | 140 |
| src/upstream/stream/observation/tests.rs | new | 194 |
| tests/stream_observation_limits.rs | new contract | 191 |
| tests/stream_observation_limits/allocation.rs | new test helper | 52 |

All seven files are at most 213 effective lines. Exact proof preserves 24 function
signatures, 20 other bodies, four other structs and 120 neighboring inputs.
Only two private capture fields, four constructor/feed bodies and private module/
import wiring change. Public paths, upstream polling, usage merging, JSON
interpretation, tracked-stream cleanup and unrelated archive behavior are preserved.
The earlier structural and stream-state evidence remains immutable.

Fresh default-feature gates pass: public regression 7/7, admission unit 12/12,
paired original stream 14/14, HTTP SSE 3/3, and stream-state public contract 13/13.
Separate fresh all-targets compilation, scoped official formatting, checker
19/19, ratchet, encoding/whitespace, exact source proof and both repository Git
diff checks pass. All native sessions are terminal. Cargo warnings remain in the
inherited Gemini image-edit/music files; none originate in this scope.

Line admission scans each input segment forward without rescanning buffered
prefixes or repeatedly draining a full chunk. Its scanning/copying work is linear
in input bytes. Existing JSON parsing/allocation, absolute poll latency, upstream
chunk allocation, concurrent-stream limits and process RSS remain separate
resource boundaries. No task, lock, I/O operation, public error or dependency was
added. Reservation failure is handled by the discard path; allocation-failure
injection was not part of this validation.

## Inventory and handoff

Strict scans 1,504 files: 31 hard, 39 mandatory and 40 soft. The 70 files above 700
remain open, and accepted clearance remains 75/145 (51.7%). Global formatting
still reports only the unchanged S06 runtime-mirror source/test files.
No checker policy, baseline, exception or runtime-profile file changed.

At acceptance capture, before this report was added, Gateway HEAD was
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,640 status entries: 166 modified,
one unstaged deletion, two staged deletions and 1,471 untracked. Neuro HEAD was
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and
182 untracked. Existing changes and staged deletions are preserved.

Evidence: target/effective-line-evidence/20260912-stream-admission/scope.json,
before.json, contract.json, source proof and separate baseline/final logs.
S06 retains its implementation and original plan cursor. GWP-20260912-01 remains
pending; the source/docs freeze and final release-build transfer remain open.
No release was built or live service changed.
