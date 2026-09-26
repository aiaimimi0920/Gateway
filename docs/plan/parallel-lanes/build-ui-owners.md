# Build-time web UI ownership

Owner: resumed parallel coordinator. State: accepted structural checkpoint.

Accepted at 2026-09-13 18:21:18.877 UTC. Entry 816 -> 153 effective lines;
all five owners are at most 294. Paired prebuilt/UI contracts pass 17/6 with
identical test identities and warning sets. Fresh all-targets, scoped formatting,
checker 19/19, ratchet, source/encoding/idle proof and both Git checks pass.
Two newly generated OUT_DIR snapshots each preserve all eleven published assets;
generated embed source is exact and no new contract fixtures remain.

Immutable evidence: target/effective-line-evidence/20260914-build-ui-owners/scope.json.
SHA-256: 58816f588a732cbf7278ccd511429baf4b2039fe428ec112912393e4ce5fc3b5.
Accepted input union: 598. Strict: 1,784 scanned, 19 hard, 25 mandatory,
40 soft; 44 above 700, clearance 101/145 (69.7%). Global formatter still finds
only the unchanged S06 runtime-mirror files. No release or deployment occurred.
See ../../status/2026-09-14-build-ui-owners.md for acceptance details.

Before snapshot contains 594 inputs and pins database-root acceptance
3601f1b986685eb408e17bfae4f57b519cb65de036edf60fcd592483fa6efd4d.
Original build.rs SHA-256:
75cf56bafee3d2dd9676539eb23625825f3972c5fdeb5497153146a7c71d8046.
The reviewed projection is entry 153, lock 294, manifest 152, snapshot 202 and
embed source 43 effective lines. Baseline prebuilt/UI contracts pass 17/6. An
isolated rustc compile and one running test disproved the reviewer's include/path
concern, so the projection and test seam remain unchanged. Published web assets
are pinned at eleven files (940,107 bytes), with their ready-marker digest saved.

Exclusive source scope: build.rs and new build_support/publish_lock.rs,
manifest.rs, snapshot.rs and embed_source.rs. Original build.rs is 816 effective
lines. The coordinator read all 903 physical lines, all 17 existing prebuilt
build tests, six UI asset tests, the runtime embedding boundary, current README
prebuilt-build instructions and the owning historical embedding design section.

Keep main orchestration, constants and shared private validation records at the
entry. Extract lock lifetime/recovery/platform probes, manifest validation,
snapshot copy/publication and generated embed source as whole responsibilities.
All owners must remain below 500. Preserve complete bodies including cfg variants,
impls, nested traversal and error/cleanup order. Keep the include-based existing
test module and its assertions unchanged; compilation must verify relative module
path resolution. No compatibility facade or public API is added.

After database-root gates are terminal, record the prior accepted input union.
Pair prebuilt_web_ui_build_contract (17) and console_ui_assets_contract (6), plus
the existing asset publication contract when applicable. Use a per-command
GATEWAY_PREBUILT_WEB_UI=1 environment for both Cargo phases to avoid a second
implicit frontend source build. Capture and preserve the current ready marker
and all declared web-asset hashes, and verify generated OUT_DIR snapshots.
Run serialized locked offline all-targets and scoped closing gates afterward.
Build-script checks belong to the coordinator; no source/format/build overlap.

The shared-build rules at board lines 1520-1533 still apply. This is a bounded
source refactor and build-script contract check, with no final source freeze,
release package, persistent runtime mutation or S06 takeover. GWP-20260912-01
and the original executor's remaining Rust/Gemini cursor remain unresolved.
