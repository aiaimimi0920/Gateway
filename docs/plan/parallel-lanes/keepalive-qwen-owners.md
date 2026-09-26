# Qwen keepalive ownership

Date: 2026-09-21. State: structural_green; release_pending.

Verified checkpoint:
[`2026-09-21-keepalive-qwen-owners.md`](../../status/2026-09-21-keepalive-qwen-owners.md).
Parent 3190 -> 2329 effective lines; six extracted owners are <=363.
Paired keepalive 24/24 and Qwen 49/49 pass, as do all-target compilation,
scoped formatting, checker 19/19, ratchet, exact projection and Git checks.
Remaining parent debt and the shared release reservation stay open.

The coordinator owns `src/keepalive.rs` and the new
`src/keepalive/qwen_web/` subtree. Preserve the accepted ChatGPT subtree,
non-Qwen dispatch and S06 scope. The pre-change parent has 3190 effective
lines; saved source and verification evidence live in
`target/effective-line-evidence/20260921-keepalive-qwen/`.

Extract wire types, worker input, sign-in policy/HTTP transport, browser worker,
runtime material persistence, and refresh orchestration into cohesive owners
below 500 effective lines. Preserve all function/test bodies, serde attributes,
public paths, timeouts, direct-signin/browser fallback order, and best-effort
Redis/PostgreSQL persistence. Shared readers, headers and browser admission
remain parent-owned. The management route's browser-only refresh remains
distinct from the normal HTTP-first refresh path.

Run paired keepalive and Qwen library tests, all-target compilation, scoped
formatter, checker tests/ratchet, exact formatted source comparison, encoding
and Git checks. Do not claim full root clearance until the remaining provider
dispatch and tests are also below the ceiling. Existing lifecycle/body bounds
are separate hardening work. The historical S06 shared-build reservation is
unchanged; this structural increment does not authorize a competing release.
