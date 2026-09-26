# ChatGPT SSE media-type matching

Status: verified, coordinator-owned, 2026-09-15.

Scope: upstream/chatgpt/common.rs and execution/tests/stream.rs only.
The accepted Official body-bound checkpoint is unchanged before this lane.
Before snapshot verifies 1797 source inputs, 22 assets, five published docs and
both repository statuses. Owners begin at 25 and 187 effective lines.

The original substring comparison admitted unrelated media types as SSE. Added four
HTTP regression groups for subtype suffixes, type prefixes, parameter tokens and
comma lists. Preserve case-insensitive matching, HTTP whitespace and parameters.
Match the essence before the first semicolon without allocating a lowercase copy;
parameter grammar remains outside this boolean routing predicate's responsibility.

Ran baseline and candidate upstream::chatgpt:: library tests serially with one
test thread; source/tests/assets frozen during gates. Closed with all-targets,
scoped formatter, checker tests, ratchet, strict inventory and both Git checks.
Owners remain under 500: production 25 -> 30 and tests 187 -> 274. Baseline
67 passed / 4 failed; candidate 71/71. Original 66 identities and the added
preservation case stay green. Source/checker 19/19/ratchet/Git checks pass.
Strict unchanged 2092/12/20/40; clearance 113/145 (77.9%).
[Full checkpoint](../../status/2026-09-15-chatgpt-sse-mime.md).
Scope SHA-256: 7754e92e8a41083c9c851dc46c67db354a65af4c4818e89a13fee238f8882d49.

HTML challenge matching and async proof-solver scheduling are separate open
follow-ups. Session-invalid classification is already independent of html_like
because auth_like || (html_like && auth_like) simplifies to auth_like; no MIME
session regression is claimed. S06 original implementation and final native
release-build coordination remain reserved under GWP-20260912-01.
