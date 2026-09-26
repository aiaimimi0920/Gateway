# Browser-pool action contract and UI parsing checkpoint

Accepted at 2026-09-14 04:18:57.746 UTC. S18 and whole-plan release remain open.
Entry 8,703 -> 8,575 effective lines. Six functions move to a 133-line pure module:
action merging/extraction, quoted/numeric scalars, player duration and UI-state
inference. Only export modifiers change. The module imports the existing input
normalizer; root named bindings, exports and callers are preserved by exact source
reconstruction. No startup I/O, registry, new parser grammar or probe consolidation.

Five pre-extraction private exports grow the fixture 76 -> 81 lines. New tests
have 71/28 lines and add 21 contracts: empty/partial action data, first-value merge,
quoted and line-delimited inputs, alias/quote/number handling, total player time,
localized UI controls and generating/ready/retry precedence. Frozen snapshots and
buttons protect non-mutation. Chinese literals are intentional UTF-8 test data.
Two synthetic request/response cases run real capture handlers through invoke
assembly and assert prompt/duration/aspect/action fields, then detach listeners.
Those cases do not open browser/network resources or validate provider traffic.

Paired serialized complete Node suites pass 143/143, identical identities/warnings
and no skips. Package 1/1; its only change adds the owner path to byte/manifest/
checksum assertions (164 -> 165 lines). Source/encoding/syntax, checker 19/19,
ratchet and both Git checks pass; all gates terminal. Existing offline-browser
tests and cleanup run unchanged. Independent review found no blocker. No Node
formatter is configured. Permissive legacy parser edge cases remain outside scope.

Evidence: target/effective-line-evidence/20260914-browser-pool-action-owners/scope.json.
SHA-256: f311dd64e34ca40741c68b7f5e3cb0f6739d3e5a03e67c14c56fd7000221e78c.
Union 724; unchanged neighbors 721; published web assets preserved. Strict:
1,881 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance stays
105/145 (72.4%). No Rust/dependency/policy/baseline/exception/release changes.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,121 entries
(182 modified, one unstaged deletion, two staged deletions, 1,936 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: media target candidate collection/scoring/deduplication. Preserve scoring
bindings used by contract merge and later audio selection; measure exact source
and add focused candidate contracts before extraction. Keep invoke orchestration
separate. S06 scope/cursor and final build transfer remain reserved under
GWP-20260912-01. Full strict/language/provider/release/runtime-profile/packaged
runtime/UI/Docker gates remain open. No deployment; persistent target stays 4200
with no persistent 4226.
