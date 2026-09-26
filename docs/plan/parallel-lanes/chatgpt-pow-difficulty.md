# ChatGPT PoW difficulty input boundary

Coordinator scope, started 2026-09-15 after proof ownership acceptance. Modify
only proof/pow.rs and its focused input tests. Preserve root proof/Turnstile/value
owners, parent exports, upstream field defaults and S06 reservations.

Before repair, input used raw string length to slice SHA3-512 and a fallback target.
Empty strings were accepted immediately; malformed strings used an unrelated target;
long inputs could panic. Reproduction uses the public build_proof_token boundary.
Valid difficulty must contain 1-64 decoded bytes: 2-128 even-length ASCII hex
characters, accepting either case. Missing/non-string upstream fields retain the
existing 0fffff default. Provided invalid strings must fail with a bounded error,
not silently substitute a target. Keep the existing provider and failure code.

Add seven invalid-input regression groups and four independent preservation cases.
Preservation checks actual token prefix/JSON/nonce and hash threshold for valid
minimum, uppercase/lowercase, maximum-width and legacy/default challenges.
Run all ChatGPT protocol tests before and after; record exact failing-before
identities, old identity preservation, all-targets, scoped rustfmt, checker,
ratchet, strict and both Git states. Native gates are serialized and idle-guarded
in GATEWAY_PREBUILT_WEB_UI=1 mode. No dependencies or persistent services change.

Resource policy: validate byte length before scanning/decoding/copying arbitrary
input, avoid echoing invalid difficulty/seed in errors, and derive compare width
from decoded bytes. Do not alter valid challenge generation or attempt limits.
VM recursion/value bounds and the surrounding synchronous CPU loop remain separate.
S06/final-build GWP-20260912-01, profile/vendor governance and full release gates
remain open. New or changed files must remain <=500 effective lines.

Evidence: target/effective-line-evidence/20260915-chatgpt-pow-difficulty/.
State: accepted difficulty boundary; whole-goal completion is not claimed.

The public boundary now rejects invalid difficulty before config allocation with
a fixed HTTP 503 message and preserved provider/code. The generator validates
before bounded decode and compares using decoded target length. Configuration,
nonce/hash/JSON, 500000 attempts, token prefixes and valid exhaustion are unchanged.
PoW 233 -> 252 effective lines, focused tests 115; both remain below 500.

All seven regression groups fail before (33 passed / 7 failed), then the identical
complete ChatGPT protocol suite passes 40/40. Four new preservation cases and all
29 prior identities remain. Default all-targets, scoped rustfmt, source restoration,
checker 19/19, ratchet and both staged/unstaged repository diff checks pass.
Independent read-only review found no introduced defect. Candidate tests passed
before a separate terminal-idle guard observed other Cargo/rustc processes. Their
project was not established; no process was stopped. Retained failed observation
and hash-bound idle resumption prove closing started after the window cleared,
without repeating the passing suite. All owned native handles are terminal.

Input union 1782; unchanged neighbors 1780; all 22 web/Tauri assets are identical.
Strict 2077/12/21/40; 33 above 700; clearance 112/145 (77.2%).
Scope SHA-256: 25d11272a2e55cfd7efe986c9f9a48166c696378237a6c6eaae53b631980a4c5.
[Full checkpoint](../../status/2026-09-15-chatgpt-pow-difficulty.md).
VM/CPU and remaining strict/provider/runtime/release work stay open.
