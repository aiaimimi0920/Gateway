# Media upstream response ownership

Accepted on 2026-09-12 as a structural checkpoint in the continuing Gateway plan.
LumaLabs, Suno and Udio response handling now has separate owners for browser
worker results, upstream errors, HTTP JSON decoding and media output materialization.
The entries retain typed execution plans and meaningful response orchestration.

| Entry | Original effective lines | Final effective lines |
| --- | ---: | ---: |
| `src/upstream/lumalabs_response_helpers.rs` | 717 | 149 |
| `src/upstream/suno_response_helpers.rs` | 1,123 | 82 |
| `src/upstream/udio_response_helpers.rs` | 1,059 | 127 |

All 26 scoped production/test files are at most 247 effective lines. Full item
comparison preserves 53 moved and 15 retained production items, all 68 existing
crate-visible paths, 122 tests and six fixtures. Child dependencies are acyclic;
Suno HTTP decoding imports its upstream-error owner directly. Existing upstream
module declarations already use basename resolution and remain unchanged.

Forty-one neighboring sources are byte-identical, including the preceding media
protocol/hardening files, three execution callers, `src/upstream/mod.rs` and
`src/error.rs`. The tests moved unchanged into provider-local responsibility groups.
Classification, error overrides, ordering, timeouts, readiness, count and MIME
contracts retain their original behavior in this checkpoint.

The first complete gate exposed unused-import warnings on explicit facade
re-exports. To retain the promised crate-visible paths, a documented
`#[allow(unused_imports)]` is limited to nine `pub(crate) use` declarations.
The final source was verified again; no file-wide suppression or checker policy
change was introduced. Earlier logs remain under `before-reexport-lints-*`.

## Verification

- Paired LumaLabs tests: 30/30 before and after.
- Paired Suno tests: 52/52 before and after.
- Paired Udio tests: 40/40 before and after.
- Existing public worker-diagnostic regressions: 7/7.
- Fresh offline, locked all-targets compilation: passed.
- Scoped official formatting, checker tests 19/19 and ratchet: passed.
- Full source/path/test preservation, encoding and independent Git checks: passed.

Global formatting reports only `gemini_canvas_runtime_mirror.rs` and
`gemini_canvas_runtime_mirror_tests.rs`, still owned by S06. Strict scans 1,478
files: 31 hard, 44 mandatory and 40 soft; 75 remain above 700. Structural clearance
is 70/145 (48.3%). Baselines and effective-line exceptions are unchanged.

Immutable evidence:
`target/effective-line-evidence/20260912-media-responses/scope.json`.
The directory contains complete pre-extraction sources, provenance-checked paired
baseline logs, final logs, body/signature comparisons and checker inventories.
Capture time: 2026-09-11 22:06:46 UTC. All native gates are terminal.

The pre-documentation capture records Gateway HEAD
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`, with 1,596 status entries:
161 modified, one unstaged deletion, two staged deletions and 1,432 untracked.
Neuro HEAD remains `bf818f0324024634bc890585efb78cc8e603d11a`, with 192 entries:
10 modified and 182 untracked. Inherited changes and staged deletions are retained.

## Remaining work

Worker failure-message overrides and Suno's invalid-JSON body preview still
publish raw diagnostics. They require a separate regression-led hardening
checkpoint. Original body/process-output allocation and successful JSON/image
materialization remain separate resource boundaries. Do not regenerate this
structural evidence after deliberate behavior changes.

No provider, browser, packaged runtime or release was exercised. S06 retains its
implementation and original cursor. GWP-20260908-06 still needs explicit
source/docs freeze and shared release-build transfer. No new release was built.
