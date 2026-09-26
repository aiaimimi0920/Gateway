# Protocol usage fallback arithmetic

Accepted hardening checkpoint: 2026-09-12 06:51:20 UTC.
Owner: parallel coordinator. The entire Gateway plan remains in progress.

Qwen Web and Xfyun usage parsers now evaluate their fallback total lazily with
saturating_add. A valid explicit total remains authoritative even when the two
component counters would overflow. Missing, null or invalid totals use the exact
sum when representable and u64::MAX otherwise. Original component counters,
missing-prompt behavior, surrounding response text and terminal state are retained.

## Reproduction and implementation

Both parsers previously called unwrap_or(prompt_tokens + completion_tokens).
The eager addition panicked in debug builds for overflowing untrusted counters,
including responses that supplied a valid total. Only these two expressions
changed in production. Two test-module declarations connect the fixed contracts.

The contracts were captured at 2026-09-12 06:31:51 UTC, before the production fix.
They exercise the actual Qwen response accumulator and Xfyun frame parser, with
exact boundary sums, optional fields, fallback overflow and reported-total
precedence. Xfyun usage aliases and both protocols' response behavior are covered.

- Baseline Qwen: 14 passed, two failed; all original 12 passed.
- Baseline Xfyun: nine passed, two failed; all original seven passed.
- All four failures report attempt to add with overflow.
- Final Qwen: 16/16; final Xfyun: 11/11, with all eight fixed regressions passing.

Qwen contract SHA-256:
6fecff3c33b0f8964e6a6e8a0b3df7c3b4deacd4ea4a1a2c41439820960b3037.
Xfyun contract SHA-256:
3f81d465f90bb4b11c91aac6ebc9895111eda4a8fa6ec49287b558b372ef27d1.
Both contracts remain byte-identical across the baseline and final runs.

## Scoped ownership and preservation

| File | Effective lines |
| --- | ---: |
| src/protocol/qwen/web_reverse.rs | 28 |
| qwen/web_reverse/response_value.rs | 85 |
| qwen/web_reverse/usage_arithmetic_contract.rs | 74 |
| src/protocol/xfyun_websocket.rs | 30 |
| xfyun_websocket/frames.rs | 184 |
| xfyun_websocket/usage_arithmetic_contract.rs | 77 |

All ten signatures, eight unrelated function bodies and 192 neighboring inputs
are preserved. Request packing, stream behavior, original tests, public paths,
feature selection, upstream callers, dependencies and checker policy are unchanged.
The earlier structural scope remains immutable. Arithmetic is constant-space and
does not add allocation, blocking work, resource ownership or public API surface.

## Fresh verification

Paired default-feature protocol tests, separate cargo check --offline --locked
--all-targets, scoped formatter, checker 19/19, ratchet, source/neighbor proof,
UTF-8 without BOM and whitespace checks all pass. Gateway and Neuro git diff
--check pass independently. Every native gate is terminal.

Global formatting still reports only the two unchanged S06 runtime-mirror files.
Strict scans 1,545 files: 31 hard, 33 mandatory and 40 soft. There are 64 files
above 700; accepted clearance stays 81/145 (55.9%). Strict still exits 1.

The pre-report snapshot records Gateway HEAD
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,694 dirty entries: 169 modified,
one unstaged deletion, two staged deletions and 1,522 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and
182 untracked. Inherited changes and deletions are retained.

## Evidence and continuation

Immutable acceptance: target/effective-line-evidence/20260912-protocol-usage/scope.json.
Its directory contains the pre-edit snapshot, fixed contracts, red/final Cargo
logs, bounded proof scripts and non-Cargo receipts.
[Lane](../plan/parallel-lanes/protocol-usage.md).

Splitter and browser-executor runtime ownership are the next structural review.
Qwen pending-line/path bounds and Xfyun transport/accumulation bounds remain
separate. S06 retains its implementation/cursor; GWP-20260912-01 and the explicit
freeze/build transfer remain pending. No release was built, live service changed
or runtime-profile policy migrated.
