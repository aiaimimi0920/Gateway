# Provider preset ownership

Accepted: 2026-09-13 02:09:28 UTC. Gateway-only structural checkpoint.

The preset entry decreases from 2,884 effective lines to 223. It retains the two
public data types and account-override compilation. Seven private factory owners
group OpenAI-compatible, native API, Google, Qwen, session-chat, search and media
templates. Registry lookup has its own owner; explicit reexports preserve the
existing public API. The original tests now have seven contract-specific owners.

| Source under src/preset/ | Effective lines |
| --- | ---: |
| Parent src/preset.rs | 223 |
| openai.rs | 205 |
| native_api.rs | 105 |
| google.rs | 297 |
| qwen.rs | 171 |
| session_chat.rs | 185 |
| search.rs | 207 |
| media.rs | 254 |
| registry.rs | 175 |
| tests.rs | 18 |
| tests/compile.rs | 228 |
| tests/session_compile.rs | 182 |
| tests/registry.rs | 64 |
| tests/api_presets.rs | 141 |
| tests/session_presets.rs | 161 |
| tests/media_presets.rs | 251 |
| tests/search_presets.rs | 103 |

Exact formatted-source proof preserves all 64 production definitions, 63 public
entry paths, 34 registry feature guards, the 58-factory registration sequence and
all 50 original test bodies/attributes. No visibility changes are needed. All 374
neighboring inputs remain byte-identical, including both routing callers, feature
declarations, manifests, protocol contracts and effective-line governance inputs.

Struct fields and serde attributes, header/body precedence, optional fallback,
model defaults, auth/session configuration, execution modes, URL/path values,
aliases, unknown-preset behavior and allocation remain unchanged. Template owners
create data only; the extraction adds no network, task, connection, process or
cleanup lifetime. The parent retains the original concrete type ownership.

Paired baseline and final gates pass with matching test identities and warnings:

| Gate | Baseline | Final |
| --- | ---: | ---: |
| Default preset tests | 50/50 | 50/50 |
| Default routing-config tests | 68/68 | 68/68 |
| Default credential-routing tests | 26/26 | 26/26 |
| No-default-feature implementation-line tests | 41/41 | 41/41 |

The all-presets-present test retains its existing default-feature contract. The
disabled-feature gate exercises implementation-line guards without rewriting that
test or claiming it is a disabled-feature registry test. Fresh locked offline
all-targets compilation passes with the three existing upstream warnings. All
Cargo gates are serialized and terminal.

Scoped formatting, checker 19/19, adoption ratchet, source/encoding proof, Cargo
cleanup and both Git diff checks pass. Global formatting still reports only the
two unchanged S06 runtime-mirror files. Strict scans 1,651 files: 27 hard, 27
mandatory and 40 soft. There are 54 files above 700, down from 55; clearance is
91/145 (62.8%). All seventeen source/test owners are at most 297 effective lines.

Immutable evidence:
target/effective-line-evidence/20260913-preset-owners/scope.json, SHA-256
a243b0688433d42542ae8800e93d850607d4d3a0c559b92b071094e1f0c23238.
Raw and canonical hashes are recorded separately for the mixed-line-ending
original. The saved original, projection, applied patch and final candidate remain
intact; no baseline, receipt or historical acceptance was regenerated.

At acceptance Gateway has 1,832 status entries: 177 modified, one unstaged
deletion, two staged deletions and 1,652 untracked. Neuro retains 192 entries: ten
modified and 182 untracked. Both HEADs and inherited deletions are preserved;
these counts precede this final report update.

Database-routing ownership is the next structural review. Its complete source
was read during the serialized Cargo gates, without modification. S06 retains its
implementation/cursor; GWP-20260912-01, runtime-profile governance and final
freeze/build transfer remain pending. Overall strict, release, packaged runtime,
UI/Docker and persistent-service acceptance remains open. No release, persistent
deployment, dependency, baseline, exception or checker-policy change was made.
