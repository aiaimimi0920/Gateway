# ChatGPT proof ownership

Coordinator-owned independent ChatGPT scope, accepted 2026-09-15 after desktop
stylesheet acceptance. S20 residual structural work targets only
src/protocol/chatgpt/web_reverse/proof.rs and new proof/{pow,turnstile,values}.rs.
S06's existing Gemini/media implementation and final release scheduling remain
reserved under GWP-20260912-01. No shared release window is inferred.

The entry decreased from 953 to 131 effective lines. PoW configuration/generation,
request-local Turnstile execution and value conversion now measure 233/394/228.
Main read the complete original, parent exports and actual upstream call sites.
Every function body and existing test body is preserved, allowing only imports,
minimum parent visibility and official rustfmt normalization. The three public
exports and seven original fully-qualified test identities remain unchanged.
All new/extracted owners, including the entry, remain <=500 effective lines.

Paired complete ChatGPT protocol tests pass 29/29 before and after with identical
test identities and warnings. All-targets, scoped rustfmt, checker 19/19, ratchet
and both staged/unstaged repository diff checks pass. Raw projection tokens match
the original bodies and current files exactly match official rustfmt output.
Inputs 1780, unchanged neighbors 1776; all 22 web/Tauri assets stay byte-identical.
Owned native gates were serialized with idle guards and prebuilt web mode.
No provider network, credentials, runtime profile, dependency,
checker policy/baseline/exception, release or persistent service changes.

Scout corrections: the checkout is dirty (main observed 186 modified, one
unstaged deletion, two staged deletions and 2261 untracked Gateway paths), and
upstream/chatgpt/execution.rs contains actual calls at lines 194, 270 and 291.
The read-only scout's clean-tree and no-external-caller claims are rejected.
The prior plan's S11 browser-worker heading does not explicitly assign this Rust
protocol file; this reservation identifies it as independent residual scope.

Inherited malformed PoW difficulty slicing and recursive VM opcode/resource
bounds require separate reproductions; this structural batch does not repair or
claim their safety. No assertion changes or new behavior belong in this move.

Evidence: target/effective-line-evidence/20260915-chatgpt-proof-owners/.
Scope SHA-256: bb30a9da7d721f479e340d6a76651b598d4c3866f0c367bf4fe5121199284b51.
Strict 2076/12/21/40; 33 above 700; clearance 112/145 (77.2%). Structural scope
accepted; full optimization/native release/product validation remains open.
[Detailed acceptance](../../status/2026-09-15-chatgpt-proof-owners.md).
