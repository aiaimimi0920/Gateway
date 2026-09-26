# ChatGPT keepalive ownership

Date: 2026-09-21. State: structural_green; release waiting for build ownership.

The coordinator owns `src/keepalive.rs` and the new
`src/keepalive/chatgpt_web/` subtree for this extraction. S06's Gemini sources,
original cursor, runtime profiles, dependency manifests and existing releases
remain outside this source scope.

The pre-change root had 4568 effective lines. Extract the complete ChatGPT refresh
family into worker wire types, input construction, refresh policy, OAuth
transport, runtime projection/persistence, and provider orchestration. Keep the
existing public `crate::keepalive` paths and every function body, serde attribute,
error, timeout and persistence order. Shared multi-provider header and input
readers stay in the parent. Each new owner must remain at most 500 effective
lines; the remaining Qwen/Suno/shared root is explicitly unfinished debt.

Verification uses paired `cargo test --locked --lib keepalive::tests --
--test-threads=1`, source-body preservation, locked all-target compilation,
scoped rustfmt, effective-line checker tests/ratchet, UTF-8 checks and Git diff
checks. Evidence lives in
`target/effective-line-evidence/20260921-keepalive-chatgpt/`.

The historical S06 build reservation requires current coordination before a
new immutable package. The requested release destination is
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.

Verified result: root 4568 -> 3190 effective lines; seven new owners are
178/179/248/92/191/305/289 lines. Exact formatted source projection preserves
all original function/test bodies and serde contracts. Paired keepalive tests
pass 24/24, steward tests 3/3, all-targets compilation, scoped formatter,
checker tests 19/19, ratchet, encoding and both Git diff checks pass.
The first missing-field-visibility compile failure and corrected retry are
retained. Global formatting still reports three untouched files; strict still
reports 29 files above 700. No release, full root clearance or resource
hardening completion is claimed.

See [the acceptance and remaining-risk report](../../status/2026-09-21-keepalive-chatgpt-owners.md).
Next: Qwen worker/refresh ownership, then remaining provider dispatch and tests.
