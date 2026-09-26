# LumaLabs, Suno and Udio protocol ownership

Owner: parallel coordinator. State: `structurally_verified`.
Started: 2026-09-12.

Scope is `src/protocol/lumalabs.rs`, `src/protocol/suno.rs`, `src/protocol/udio.rs`,
their new private production/test directories, and the three enabled module
declarations in `src/protocol/mod.rs`. Original entry effective counts are
1,487, 1,175 and 1,360. All three complete sources and original tests have been
read directly, and byte-equal snapshots are recorded before production edits.
Upstream execution, browser workers, provider state, S06 and release ownership
remain outside this extraction.

Each protocol gets explicit owners for request options, generation/action wire
construction, decoded results, output envelopes and worker contracts. LumaLabs
runtime material and event URL recovery have their own owners. Suno/Udio typed
field decoding stays within each provider; no new cross-provider utility layer
is introduced. Keep operation/song/clip schemas, defaults and the meaningful
canonical entry behavior at the public roots. Move complete functions, types and
their implementation bodies without changing ordering, allocations or lifetimes.

Preserve all public paths, constants, error codes/messages, alias precedence,
numeric conversions, polling bounds, output readiness and MIME/URL behavior.
The 29 LumaLabs, 22 Suno and 24 Udio tests remain unchanged, including local
upstream-plan rejection contracts and four fixtures. Split tests by runtime
configuration, requests, outputs and worker contracts; keep every final owner
below 500 effective lines without baseline or exception changes.

Evidence is under `target/effective-line-evidence/20260912-media-protocols/`.
Run the three original test filters before and after extraction, complete
item/type/constant/public-path comparisons, all-targets compilation, official
formatter, checker tests, ratchet, strict inventory and independent Git/encoding
checks. Source behavior hardening requires separate failing regression evidence.

Before source moves, the original default-feature library filters pass 29/29,
22/22 and 24/24 respectively. All 75 tests run offline; no provider, browser,
runtime state or release is exercised by this baseline.

Initial all-targets compilation exposed Rust's child-module lookup under explicit
`#[path]` entry attributes. The enabled declarations now use normal basename
resolution, matching the existing Kiro pattern. Exactly three redundant path
attributes are removed; every feature condition, disabled path and other module
declaration is preserved. The failed diagnostic and pre-edit module snapshot are
retained separately before the fresh compile/test gate.

All three extraction comparisons pass: 152 moved and eight retained production
items, 27 constants, 75 original tests and four fixtures. The entries are now
119/119/114 effective lines, and every owned source/test file is below 500.
Fresh all-targets compilation and repeated original-test gates pass: 29/29,
22/22 and 24/24. Scoped formatting, checker 19/19, ratchet, encoding, public-path
and dependency checks, and both independent Git diff checks pass. All 35 scoped
files are at most 288 effective lines; 15 neighboring protocol-family files are
unchanged. Global formatting still reports only the two S06 runtime-mirror files.
Strict now scans 1,454 files: 31 hard, 47 mandatory and 40 soft, with 78 above 700.
Structural clearance is 67/145 (46.2%). Immutable evidence is `scope.json` in the
directory above. [Acceptance report](../../status/2026-09-12-media-protocols.md).

The subsequent [worker diagnostic hardening](media-worker-errors.md) closes the
constructor detail and explicit local-path boundary with separate paired evidence.
The exact structural snapshots remain immutable; upstream failure-message overrides
and original process-output allocation remain separate boundaries.

S06 retains its implementation and original cursor. The GWP-20260908-06 source/docs
freeze and shared release-build transfer still require an explicit receipt.
Scoped Cargo validation does not establish a release or real-provider acceptance.
