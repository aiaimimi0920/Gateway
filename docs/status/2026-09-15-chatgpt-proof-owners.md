# ChatGPT proof ownership acceptance, 2026-09-15

Independent Gateway ChatGPT proof structure is accepted. The entry decreases
from 953 to 131 effective lines. Three private owners isolate PoW generation
(233), request-local Turnstile execution (394), and ordered values/wire conversion
(228). All results are below 500. S06's existing Gemini/media scope and final
release scheduling remain reserved; the full optimization objective stays open.

Main read the original implementation, public parent exports and actual upstream
call sites before editing. Original function and test bodies remain unchanged,
apart from minimum module visibility and official rustfmt formatting. Three public
exports and seven fully-qualified proof test identities are preserved. OrderedMap
entries stay private; shared internal types/functions use pub(super), with no
crate-wide exposure. Program state and mutation order remain request-local.

The initial read-only scouts incorrectly reported a clean tree and absent callers;
main rejected those claims using actual Git observations and execution.rs calls at
194/270/291. Pre-implementation and final candidate reviewers found no introduced
defect. A speculative unused-json concern was rejected because value conversion
uses json!. Owner and review decisions are recorded in evidence/review.md.

## Fresh verification

- Same complete ChatGPT protocol filter before and after: 29/29 passed, zero
  failed/ignored, 2953 filtered out. All test identities and warning messages match.
- The seven original proof tests retain token prefixes, legacy JSON shape,
  decoded VM output, browser primitives, second-stage programs and failure codes.
- cargo check --offline --locked --all-targets passes with default features.
- Scoped official rustfmt --check passes for all four files. Global formatting
  and the reserved S06 findings are not declared cleared by this scoped gate.
- Original bodies equal raw projection tokens while preserving literals and
  identifier boundaries; current files equal exact official rustfmt output.
- Checker tests 19/19, ratchet and both Gateway/Neuro staged/unstaged diff checks
  pass. Strict remains exit 1 for other recorded debt.
- 1780 frozen inputs, 1776 unchanged neighbors and all 22 existing web/Tauri build
  artifacts are verified. Parent exports, upstream callers, other Rust sources,
  tests and previously accepted desktop/browser changes stay byte-identical.

All native gates were serialized, guarded and used GATEWAY_PREBUILT_WEB_UI=1.
No frontend rebuild or live provider call was performed. An unrelated Beaver
compiler appeared after the baseline started and was preserved. Both paired gates
and closing returned terminal native-idle receipts. The existing unused HashMap
and prebuilt frontend snapshot warnings are unchanged in the paired protocol runs.

The first supplemental token checker rejected an official rustfmt-added trailing
comma in the now-multiline pow_generate signature. Only the checker was corrected:
compare original/raw projection, then enforce exact formatter output. Production
source and tests did not change for this evidence repair. rust-toolchain.toml was
additionally observed equal to HEAD, with a modification time preceding baseline;
that supplemental file was not in the initial input manifest.

## Evidence and remaining work

Evidence root: target/effective-line-evidence/20260915-chatgpt-proof-owners/.
Acceptance: scope.json at 2026-09-15T07:53:21.515Z.
Scope SHA-256: bb30a9da7d721f479e340d6a76651b598d4c3866f0c367bf4fe5121199284b51.
Predecessor: d2c23bd4db0f3bc837bc29a4d4399cc74a6e983e426506933b085635e5ea93fe.
The scope records source/token hashes, exact test identities, gate receipts,
review and script hashes, asset preservation and separate repository states.
publication.json adds final document/source observations and both Git diff gates.

Strict: 2076 scanned, 12 hard, 21 mandatory and 40 soft. Files above 700 decrease
34 -> 33; clearance is 112/145 (77.2%). Remaining >700 findings comprise 21 Rust
and 12 runtime-profile/vendor files. No checker policy, baseline or exception was
changed. Prior frontend/browser evidence is preserved, not rerun in this Rust batch.

At acceptance, Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d:
187 modified, 1 unstaged deletion, 2 staged deletions and 2265 untracked paths.
Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a:
10 modified and 182 untracked paths. Final publication adds this status document.
Inherited dirty/staged/deleted content remains intact; no commit was performed.

Next reproduce malformed/overlong PoW difficulty slicing before a separate input
hardening patch. VM recursion, instruction/value growth and other resource bounds
remain separate; four outer stages do not establish a total execution bound.
This extraction proves structural preservation, not safety of all inherited input
paths. Full feature/language/provider/packaged runtime/UI/Docker/release validation
remains open. GWP-20260912-01 retains S06 and final native-build coordination.
Persistent target 4200; no persistent 4226. Credentials, runtime profiles,
dependencies, sibling source, release payloads and persistent services are unchanged.
