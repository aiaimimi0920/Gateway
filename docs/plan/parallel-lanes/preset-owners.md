# Provider preset ownership

Owner: parallel coordinator. State: accepted. Started: 2026-09-13 UTC.
Accepted: 2026-09-13 02:09:28 UTC.

Scope: src/preset.rs and private preset/ owners. The original entry had 2,884
effective lines. Retain the public data types and compile_provider_account at the
entry, and extract provider-family factories and feature-gated registry lookup.
Preserve all existing public function paths with explicit reexports. Split the
original tests by merge, registry, API, session, media and search contracts.

The complete original source and both routing callers were read before selection.
Preserve struct fields/serde attributes, header and extra-body merge precedence,
optional-field fallback, preset registration order, feature guards, aliases,
unknown-preset behavior, protocol constants and every factory/test body. Do not
change runtime execution modes, auth headers, default models, URLs or allocation
while extracting these pure template owners. They own no running process or task.

Require paired default preset, routing-config and credential-routing tests,
paired no-default-feature implementation-line tests, fresh locked offline
all-targets compilation, scoped formatting, checker 19/19, ratchet/strict reports,
exact source/public-path/test proof, UTF-8/no-BOM checks and both Git diff checks.
The existing all-presets-present test is a default-feature contract; do not run or
rewrite it as a no-default-feature registry contract. Every extracted owner must
remain at most 500 effective lines. Serialize Cargo; do not edit Rust during gates.

Evidence: target/effective-line-evidence/20260913-preset-owners/.

S06 keeps its implementation/cursor. GWP-20260912-01, runtime-profile governance
and final freeze/build transfer remain pending. No dependency, baseline, exception,
checker-policy, release or persistent deployment change belongs to this scope.

The entry is now 223 effective lines. All seventeen source/test owners are at
most 297. Exact proof preserves 64 production definitions, 63 public paths, all
50 tests, 34 registry feature guards and the 58-factory registration sequence.
No visibility changes are needed; 374 neighboring inputs remain byte-identical.

Paired preset/config/credential tests pass 50/50, 68/68 and 26/26. Paired disabled
implementation-line tests pass 41/41. Fresh all-targets, scoped formatting,
checker 19/19, ratchet, source/encoding/cleanup proof and both Git checks pass.
Cargo is terminal. Strict: 1,651 scanned, 27 hard, 27 mandatory, 40 soft; 54 above
700. Clearance: 91/145 (62.8%). Only the two unchanged S06 global-format findings
remain. scope.json SHA-256:
a243b0688433d42542ae8800e93d850607d4d3a0c559b92b071094e1f0c23238.
[Acceptance report](../../status/2026-09-13-preset-owners.md).
Database-routing ownership is next; wider completion gates remain open.
