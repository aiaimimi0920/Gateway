# Keepalive remaining ownership closure

Date: 2026-09-21. State: structural_green; full-goal and release work remain active.

Verified checkpoint: [`2026-09-21-keepalive-closure.md`](../../status/2026-09-21-keepalive-closure.md).
Parent 1338 -> 481 effective lines; all 29 keepalive Rust files are <=481.
Baseline keepalive 24/24 and final 28/28 pass, as do steward 3/3, all-target
compilation, checker 19/19, ratchet, scoped formatting, projection, encoding
and both Git checks. Strict >700 count decreases 29 -> 28. Global formatting,
remaining source/soft-limit debt, hardening and release remain open.

The coordinator owns `src/keepalive.rs` and its new auth-material, metadata,
headers, browser-policy, provider-probe and Suno runtime owners. The parent
starts at 1338 effective lines. Preserve the existing ChatGPT/Qwen subtrees,
public entry paths and all existing tests. Keep Gemini mapping/material code
in the parent and do not edit any reserved S06 implementation.

Move complete reader/header/policy functions. Extract the Chataibot/LumaLabs/
Producer admission probes with explicit `ControlFlow` to distinguish early
responses from successful fallthrough to session persistence, returning owned
material without cloning. Extract Suno cookie resolution and runtime probes;
its branch must return before generic session persistence. Preserve errors,
HTTP methods/paths, order, expiry handling and best-effort writeback semantics.
Every completed source owner must be below 500 effective lines.

Run paired keepalive tests, focused admission-probe regression cases,
all-target compilation, exact extraction projection for unchanged bodies,
scoped formatter, checker tests/ratchet, strict audit and encoding/Git checks.
Evidence: `target/effective-line-evidence/20260921-keepalive-closure/`.
The complete repository refactor and release remain the active objective;
this lane only clears keepalive when all its owners meet the local threshold.
The S06 final-build ownership reservation remains unchanged.
