# ChatGPT PoW difficulty acceptance, 2026-09-15

Gateway's provided PoW difficulty now has a bounded public input contract. The
repair rejects blank, odd-length, non-hex, whitespace, Unicode and oversized
strings before randomized browser configuration allocation. Valid input contains
2-128 even-length ASCII hex characters, representing 1-64 bytes; either case is
accepted. Missing/non-string upstream fields keep their existing 0fffff default.

Only proof/pow.rs and the new proof/pow/input_tests.rs source owner changed.
PoW measures 233 -> 252 effective lines, including two test-module declaration
lines; the focused tests measure 115. Both remain below 500. The 17-line production
increase belongs to this existing PoW input boundary and needs no new owner.
S06's original Gemini/media implementation and final native build coordination
remain reserved. The full optimization objective is still active.

## Root cause and preserved contracts

The original generator decoded difficulty with an unrelated fallback target, then
used the raw string byte length to slice that target and the SHA3-512 digest.
Empty strings immediately succeeded. Malformed strings could generate a proof
under the fallback target; long inputs could panic. The actual execution.rs
caller forwards provided strings unchanged, so the public boundary was affected.

The shared validator checks byte length before scanning at most 128 bytes. Public
rejection returns a fixed, bounded HTTP 503 diagnostic without seed/difficulty
echo, preserving CHATGPT_WEB_REVERSE_ADAPTER and chatgpt_web_proof_token_failed.
The internal generator validates defensively before decoding at most 64 bytes,
removes the malformed-input fallback and derives comparison width from target.len().
Valid generation retains its configuration, prefixes, nonce order, JSON/hash
serialization, 500000-attempt limit and exhaustion payload/error behavior.

## Fresh verification

- The complete ChatGPT protocol filter had 33 passed / 7 failed before repair.
  All seven new rejects_* groups failed for the intended acceptance/panic defect;
  the failure was not a compile error. The same frozen suite passes 40/40 after.
- All 29 prior protocol identities remain; four new preservation cases stay green.
  Tests verify actual emitted Base64 JSON, 18 browser fields, prefix, nonce/counter
  relation and SHA3 hash threshold for minimum, mixed-case, maximum and default
  difficulty. Legacy requirements retain their wire shape.
- cargo check --offline --locked --all-targets passes with default features.
  Scoped official rustfmt --check, checker tests 19/19, ratchet and both Gateway/
  Neuro staged/unstaged git diff --check pass. Paired warning messages are identical.
- Exact source restoration removes only the three documented production edits and
  the test-module declaration, recovering the original file. Configuration, valid
  attempt loop, serialization and exhaustion behavior remain unchanged.
- 1782 frozen inputs, 1780 unchanged neighbors and all 22 existing web/Tauri build
  artifacts are verified. The regression test hash is unchanged from failing-before
  preparation. rust-toolchain.toml is included in this batch's initial manifest.

The independent clean default-role reviewer found no introduced defect. Main
reviewed the input/encoding/diagnostic boundaries, bounded allocation, resource
lifetime and CPU behavior of both changed owners. No handles, tasks or global
state were added. Maximum-width mixed-case hex was suggested as an optional extra
fixture; separate mixed-case/minimum and maximum-width cases already cover the
responsible paths, so the frozen tests were retained.

All owned native gates are serialized and use GATEWAY_PREBUILT_WEB_UI=1. Candidate
Cargo passed and exited 0, then its runner's terminal-idle check observed two
other Cargo processes and one rustc process. They were preserved. Their project/
cwd could not be established before they exited. A separate hash-bound idle
resumption preceded closing; the passing tests were not rerun. Both the failed
terminal observation and successful resumption are retained. No native gate
remains active at acceptance. Frontend builds and live provider calls were not run.

## Evidence and remaining work

Evidence root: target/effective-line-evidence/20260915-chatgpt-pow-difficulty/.
Acceptance: scope.json at 2026-09-15T08:37:32.839Z.
Scope SHA-256: 25d11272a2e55cfd7efe986c9f9a48166c696378237a6c6eaae53b631980a4c5.
Predecessor: bb30a9da7d721f479e340d6a76651b598d4c3866f0c367bf4fe5121199284b51.
publication.json binds the final documents, source proof and separate Git states.

Strict still exits 1: 2077 scanned, 12 hard, 21 mandatory and 40 soft findings.
There remain 33 files above 700: 21 Rust and 12 runtime-profile/vendor files.
Clearance remains 112/145 (77.2%); this batch adds no oversized owner or exception.
At acceptance, Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d, with
187 modified, 1 unstaged deletion, 2 staged deletions and 2268 untracked paths.
Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a, with 10 modified and
182 untracked paths. Publication adds this report and records final observations.

VM recursion/instruction/value growth, seed/config limits and synchronous PoW CPU
work remain separate. Feature/language/provider/packaged runtime/UI/Docker/release
gates remain open; this checkpoint does not claim provider or release readiness.
GWP-20260912-01 retains S06 and final native-build coordination. Persistent target
4200, no persistent 4226. Credentials, profiles, dependencies, checker policy,
baseline, exceptions, sibling source, releases and persistent services are unchanged.
Inherited dirty/staged/deleted content is preserved. No deployment or commit.
