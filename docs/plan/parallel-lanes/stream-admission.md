# SSE observation admission

Owner: parallel coordinator. State: accepted.
Started: 2026-09-12.

Scope is src/upstream/stream.rs, its usage.rs and completion.rs owners, a new
private observation-line owner and focused unit/public regression files.
Existing structural and stream-state evidence stays immutable.

At baseline, both observers copied entire chunks into a Vec, rescanned incomplete
lines and drained prefixes for each newline. Pending data/event fields had no
admission limit. The translated protocol decoder is bounded but has different whitespace,
EOF and error behavior; replacing either observer with it would change contracts.

Preserve forwarded chunks, public paths, upstream polling, usage merging, JSON
interpretation, invalid-UTF-8 handling and each observer's existing delimiter/
whitespace/EOF behavior. Use a shared line-admission owner with a 64 MiB raw
event budget, matching the established translated-frame byte ceiling, and a
65,536-line budget to bound empty-field metadata. Both budgets include delimiter
bytes/lines. Limits apply per observed event, not per transport chunk.

Admit before copying, grow partial-line storage geometrically within the byte
budget, and release large temporary storage. On overflow or reservation failure,
clear pending parser material but retain the last valid snapshot. Discard through
that observer's next frame delimiter without retaining discarded bytes; resume
normal observation afterward. Forward the original chunks and errors unchanged.
Line scanning and copying must be linear in input bytes. Existing JSON parsing,
absolute poll time, process RSS and upstream allocations remain separate bounds.

First capture public regressions against the unchanged observers. Cover oversized
chunk allocation requests, cumulative chunk/frame admission, line-count pressure,
recovery, byte identity, fragmented UTF-8/CRLF, parser-specific whitespace and EOF.
Then add small-budget primitive tests for exact limits and geometric growth.

Evidence: target/effective-line-evidence/20260912-stream-admission/.
The 123-input pre-edit snapshot and fixed seven-test public contract are captured.
The unchanged observers pass four tests and fail three: cumulative byte admission,
empty-field line pressure and a 67,109,888-byte observer allocation request.
Default-feature original stream 14/14, HTTP SSE 3/3 and stream-state 13/13 pass.
The implementation is in place. Entry/usage/completion/admission owners measure
110/213/164/140 effective lines; unit tests and public contract/helper measure
194/191/52. Exact proof retains 24 signatures, 20 other function bodies, four other
structs and 120 neighbors. Only two capture fields, four constructor/feed bodies
and private module/import wiring change. The seven-test public contract and its
allocator helper remain hash-identical. Fresh default-feature final regression
7/7, admission unit 12/12, paired original stream 14/14, HTTP SSE 3/3 and stream-state
13/13 pass. Fresh all-targets, scoped formatter, checker 19/19, ratchet, encoding,
source/neighbor proof and both Git checks pass. All native gates are terminal.

The immutable scope was captured at 2026-09-12 03:08:06 UTC. Strict scans 1,504
files: 31 hard, 39 mandatory and 40 soft; 70 remain above 700. Clearance remains
75/145 (51.7%). Global formatting still reports the two unchanged S06 files.
[Acceptance report](../../status/2026-09-12-stream-admission.md).

S06 ownership and GWP-20260912-01 are unchanged. No release, runtime profile,
checker policy, baseline or dependency change is included.
