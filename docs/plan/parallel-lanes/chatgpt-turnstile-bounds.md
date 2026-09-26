# ChatGPT Turnstile resource bounds

Coordinator scope, 2026-09-15, after accepted PoW difficulty hardening. Own only
proof/turnstile.rs, the minimal immutable OrderedMap entry accessor in proof/values.rs,
and new turnstile budget/test owners. Preserve proof exports, PoW, upstream challenge
fallback/error contracts, S06's original source and final build coordination.

Main read the full VM, value conversions and immediate upstream caller. The four
outer stages do not bound instruction count, opcode 23 recursion, repeated clones,
string/array growth or register retention. No recorded non-secret provider fixture
corpus or provider maximum size was found. Existing tests cover only small synthetic
programs, browser location primitives and a second stage. The graph still points to
pre-extraction proof.rs locations; current source is authoritative.

Implement request-local fail-closed limits: raw encoded input 2 MiB, XOR key 64 KiB,
8192 instruction slots and 8192 function dispatches across all stages, call depth 64,
value depth 64, 32768 value nodes, 2 MiB per measured value, 4096 registers, 8 MiB
retained state, 64 MiB cumulative value work, and 2 MiB encoded output. Value cost
uses a conservative 64-byte node charge plus UTF-8 string/key bytes; it is logical
accounting, not an exact process RSS measurement. Replacements subtract old retained
cost. A bounded value operation may create a temporary before admission; operands,
depth and cumulative work remain bounded, preventing unbounded doubling.

Validate raw byte lengths before trim/decode/copy, parsed JSON before recursive
conversion, and all stored/generated values before later use. Keep one shared
budget across stages and nested calls. Exhaustion returns None even after an earlier
result, retaining the caller's browser-challenge fallback. No new threads/services,
dependencies, profiles, policy/baseline/exception changes or release writes.

Use failing-before public-boundary tests for input, execution, recursion, retained
state and value growth. Preserve Base64 padding/trim, XOR, ordered map replacement,
apply/dynamic/conditional opcodes, stage behavior and request-local reset. Run the
complete ChatGPT protocol filter before/after, then default all-targets, scoped
rustfmt, checker tests, ratchet, strict and separate repository diff checks. Freeze
all prior accepted sources/assets during serialized native gates. Every owner must
remain <=500 effective lines. Runtime/provider/full release validation remains open.

Evidence: target/effective-line-evidence/20260915-chatgpt-turnstile-bounds/.
Baseline: 48 passed / 17 failed across 65 protocol cases. Sixteen resource cases
fail; one proposed preservation case reveals a raw dynamic-locator/display-text
mismatch. Include the narrow raw-String locator correction, retaining the exact
built-in whitelist and frozen test identities. Eight new preservation cases and
all 40 prior identities pass before repair.

Accepted: the identical frozen suite passes 65/65 after resource accounting and
raw-locator repair. Entry/values/budget/tests measure 433/231/210/252 effective
lines. Twelve opcode branch bodies, the exact whitelist, all prior PoW sources and
the values owner apart from its immutable accessor remain unchanged. Default
all-targets, scoped rustfmt, source proof, checker 19/19, ratchet and both repository
staged/unstaged diff checks pass. Independent semantic review found no introduced
defect; the budget scout's failed shell command is not credited as test evidence.

Input union 1784; unchanged neighbors 1780; all 22 web/Tauri artifacts are identical.
Strict 2079/12/21/40; 33 above 700; clearance 112/145 (77.2%). Native gates terminal.
Scope SHA-256: b7950d18d9718dc140f082529ad22be981f4ab7fd0d01191af4714042c745193.
[Full checkpoint](../../status/2026-09-15-chatgpt-turnstile-bounds.md).
Real-provider threshold validation, synchronous scheduling/backpressure, remaining
strict debt and full runtime/release gates remain open. Full goal stays active.
