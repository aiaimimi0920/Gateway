# ChatGPT Turnstile bounds acceptance, 2026-09-15

Gateway's browserless Turnstile VM now uses a shared request-local resource budget.
It bounds raw input, instruction slots, nested dispatch, value depth/nodes/bytes,
register retention, cumulative logical work and encoded output. Exhaustion returns
None even if an earlier instruction produced a result, preserving the upstream
browser-challenge fallback. The existing four-stage maximum remains in place.

The VM entry measures 394 -> 433 effective lines. The values owner changes 228 ->
231 only for an immutable ordered-entry accessor. The new budget owner is 210 and
the focused test owner is 252; all remain below 500. PoW and its accepted difficulty
tests, parent exports, upstream code and other prior sources remain unchanged.

## Diagnosis and implementation

Main read the full VM, conversions and actual caller before editing. The former
four-stage loop did not bound instruction count, opcode 23 recursion, register
growth, repeated clones or string/array doubling. Read-only scouts found no recorded
non-secret dx fixture corpus or provider size maximum. Existing local redactor,
accumulator and eventstream budgets informed the accounting mechanism.

The selected policy is raw dx <=2 MiB, key <=64 KiB, at most 8192 instruction slots
and 8192 dispatches across all stages, call depth <=64, value depth <=64, <=32768
nodes/value, <=2 MiB measured/value, <=4096 registers, <=8 MiB retained state,
<=64 MiB cumulative logical value work and <=2 MiB encoded output. Logical cost is
64 bytes/node plus UTF-8 string/key bytes. It is not an exact RSS measurement or a
wall-clock deadline. Bounded operands can create temporary clones/format strings
before admission; input/value/depth/work caps prevent unbounded repeated growth.

Raw lengths are checked before trim/decode/copy. Initial and opcode 14 JSON values
are admitted before recursive conversion. Reads charge cached costs; checked insert
validates generated values and subtracts replaced retained cost. Reflect.set clones
and reinserts through that boundary. Function calls share cumulative/depth counters,
including conditional recursion and value callbacks. Output length is checked before
Base64 allocation. Failed state is sticky until the request-local state is dropped.

One proposed preservation fixture exposed an additional existing semantic defect:
dynamic dispatch converted raw function locators to display strings, then tried to
match the original raw whitelist. Object.create could never match. The repair retains
raw String locators and the exact existing whitelist, enabling the ordered-object
fixture through both direct opcode 17 and opcode 7 application. No arbitrary function
evaluation or browser execution was added. This was a failing-before functional case,
not a passing preservation case; its frozen test name is retained in the evidence.

## Fresh verification

- Before: the complete ChatGPT protocol suite had 48 passed / 17 failed. All 16
  resource cases failed, plus the dynamic ordered-object fixture. All 40 prior
  protocol identities and eight new preservation cases passed.
- After: the same frozen suite passes 65/65, zero failed/ignored. All test identities
  remain; the focused test file hash is identical to failing-before preparation.
- Preservation covers input/key and instruction boundaries, request-local reset,
  conditional/null handling, array/string concatenation, register replacement,
  four stages and the existing fifth-stage limit, Base64 whitespace/padding, Unicode
  and malformed wire input. The fixed dynamic fixture verifies insertion/replacement
  order, JSON formatting, direct/apply invocation and result application.
- Default cargo check --offline --locked --all-targets and scoped official rustfmt
  --check pass. Checker tests 19/19, ratchet and both Gateway/Neuro staged/unstaged
  git diff --check pass. Paired protocol warning messages are identical.
- Source proof confirms twelve unchanged opcode branch bodies and the exact existing
  dynamic whitelist. Removing only the immutable accessor restores values.rs.
  The supplemental verifier initially searched past the opcode match and saw the
  later whitelist's `_` arm; narrowing its range repaired the evidence checker only.
- 1784 frozen inputs, 1780 unchanged neighbors and all 22 existing web/Tauri build
  artifacts are verified. All owned native gates were serialized, guarded and run
  with GATEWAY_PREBUILT_WEB_UI=1; their terminal process observations are idle.

Independent semantic review found no introduced opcode defect and confirmed the
locator correction, ordered replacement, stage behavior and failure propagation.
The separate budget scout provided partial source observations and attempted a
disallowed test command that failed in its context-mode shell; no test evidence or
clean-review conclusion is credited to it. Main checked its opcode 2/3 concern:
those arguments are subtrees of already admitted programs, including later stages
from measured retained values. Main reviewed each changed owner for bounds, input
encoding, checked arithmetic, lifecycle, secret exposure and synchronous work.

## Evidence and remaining work

Evidence root: target/effective-line-evidence/20260915-chatgpt-turnstile-bounds/.
Acceptance: scope.json at 2026-09-15T10:32:36.023Z.
Scope SHA-256: b7950d18d9718dc140f082529ad22be981f4ab7fd0d01191af4714042c745193.
Predecessor: 25d11272a2e55cfd7efe986c9f9a48166c696378237a6c6eaae53b631980a4c5.
publication.json binds the final documents, source proof and separate Git states.

Strict still exits 1: 2079 scanned, 12 hard, 21 mandatory and 40 soft findings.
There remain 33 files above 700: 21 Rust and 12 runtime-profile/vendor files.
Clearance remains 112/145 (77.2%). No checker policy, baseline or exception changed.
At acceptance, Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d, with
187 modified, 1 unstaged deletion, 2 staged deletions and 2272 untracked paths.
Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a, with 10 modified and
182 untracked paths. Publication adds this report and records final observations.

Real-provider threshold compatibility remains unverified because no recorded
non-secret challenge corpus was available. The synchronous solver and caller's
two solve attempts remain; scheduling/backpressure and PoW CPU work are separate.
Feature/language/provider/packaged runtime/UI/Docker/release gates stay open.
S06 and final-build GWP-20260912-01 remain reserved; the full optimization goal is
active. Persistent target 4200, no persistent 4226. Credentials, profiles, dependencies,
sibling source, releases and persistent services are unchanged. No deployment or commit.
