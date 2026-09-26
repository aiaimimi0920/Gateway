# Protocol usage fallback arithmetic

Owner: parallel coordinator. State: verified hardening checkpoint.
Started: 2026-09-12.
Accepted: 2026-09-12 06:51:20 UTC.

The previous Qwen Web and Xfyun parsers called unwrap_or with the eager expression
prompt_tokens + completion_tokens. Untrusted u64 counters can overflow even when
an explicit total_tokens value should make the fallback unnecessary.

Scope is Qwen response_value.rs, Xfyun frames.rs, their test-module declarations
and two focused usage contracts. Preserve exact component counters, authoritative
valid explicit totals, missing/invalid total fallback and absent prompt behavior.
Only the fallback becomes lazy and saturating. Request packing, stream frames,
original tests, external callers and previous structural evidence stay unchanged.

The fixed eight-test contract was captured before the production change. It
exercises the actual response/frame parsers. Four overflow tests failed on the
original implementation, including both explicit-total cases; all original 19
tests passed. Final Qwen is 16/16 and Xfyun is 11/11. The lazy, saturating fallback
preserves exact nonoverflow sums, authoritative totals, component counters and
optional-field behavior. The regression owners measure 74/77 effective lines.

Fresh all-targets, scoped formatter, checker 19/19, ratchet, source/encoding proof
and both Git checks pass. Ten signatures, eight unrelated bodies and 192
neighboring inputs are unchanged. Strict scans 1,545 files: 31 hard, 33 mandatory
and 40 soft; 64 remain above 700. Earlier structural evidence stays immutable.

Evidence: target/effective-line-evidence/20260912-protocol-usage/.
[Acceptance report](../../status/2026-09-12-protocol-usage.md).
Qwen pending-line/path limits and Xfyun transport/resource bounds remain separate.
S06 ownership and pending GWP-20260912-01 freeze/build coordination are unchanged.
