# Build-time web UI ownership acceptance

Accepted at 2026-09-13 18:21:18.877 UTC (2026-09-14 local time). The overall
Gateway optimization and release plan remains open.

## Scope and preserved behavior

The build.rs entry decreases from 816 to 153 effective lines. Four private
owners separate the existing publish lock, ready-manifest validation, immutable
snapshot publication and generated Rust embedding source.

| File | Effective lines | Responsibility |
| --- | ---: | --- |
| build.rs | 153 | Build orchestration, constants and private validation records |
| build_support/publish_lock.rs | 294 | Lock acquisition, recovery, Drop and platform process probes |
| build_support/manifest.rs | 152 | Ready marker, path containment and asset validation |
| build_support/snapshot.rs | 202 | Validated copy, snapshot publication and cleanup |
| build_support/embed_source.rs | 43 | Generated embedding source and Cargo environment directive |

Complete-block round-trip proof preserves cfg variants, impl/Drop bodies,
nested traversal, all operation/error/cleanup ordering and unchanged parent
behavior. Eleven helpers gain only pub(super); shared records and fields remain
private. The 593 neighboring inputs, including both original integration test
files, are unchanged. Recursive checker policy already covers build_support;
no policy, baseline, dependency or exception change was made.

Review covered lock ownership/recovery, process liveness, path traversal and
symlink rejection, manifest/digest validation, copy retries, failure cleanup,
snapshot replacement and generated-path escaping. Extraction introduces no new
resource owner or change in allocation, locking, timeout or I/O policy.

## Verification

Paired prebuilt_web_ui_build_contract tests pass 17/17 and
console_ui_assets_contract tests pass 6/6. Names, outcomes and warning sets are
identical. Every Cargo command uses recorded per-process
GATEWAY_PREBUILT_WEB_UI=1; no persistent environment setting or implicit frontend
source build was introduced.

The independent reviewer raised an include/path resolution concern. An isolated
rustc reproduction compiled and passed its one test; the actual 17-test Cargo
contract also passes after extraction. No speculative compatibility branch or
test-path adjustment was added. The historical separate Python asset-publication
test is absent in this checkout and is not reported as a passing gate.

Locked offline all-targets compilation, scoped rustfmt, source proof, checker
19/19, ratchet, UTF-8 without BOM, idle process proof and separate Gateway/Neuro
diff checks pass. Every compiler/formatter gate ran serially and all owned native
handles are terminal. No external process was terminated.

The published ready marker and eleven web files (940,107 bytes) are unchanged.
Ready-marker SHA-256:
ca850709587a0b05dddc389db9938e8fa2317d6f62d0585753e9244b042a6d2e.
Two fresh Cargo OUT_DIR snapshots contain exactly the same eleven paths and
digests; generated gateway_embedded_ui.rs is exact. No newly created prebuilt
contract fixture directory remains. This verifies debug build output, not a
new product release.

Strict scans 1,784 files: 19 hard, 25 mandatory, 40 soft; 44 remain above 700.
Clearance is 101/145 (69.7%). Strict exits 1. Global formatting exits 1 only for
src/upstream/gemini_canvas_runtime_mirror.rs and its unchanged test sibling;
both remain within the reserved S06 scope.

Immutable evidence: target/effective-line-evidence/20260914-build-ui-owners/scope.json.
SHA-256: 58816f588a732cbf7278ccd511429baf4b2039fe428ec112912393e4ce5fc3b5.
Acceptance verifies the 598-input union, saved/current hashes, projection chain,
exact tests/warnings, prebuilt environments, serialized timing, generated assets,
inventory, reviewer record and separate Git state.

At acceptance Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d:
1,992 status entries (182 modified, one unstaged deletion, two staged deletions,
1,807 untracked). Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a:
192 entries (ten modified, 182 untracked). Documentation publication follows
this census; inherited edits remain preserved.

## Continuation

Routing configuration ownership is the next independent S16 boundary. Read-only
scouts have located its contracts, callers and tests; exact source reading,
ownership design and baseline remain required before production changes.
S06 ownership/cursor and final shared-build transfer still require an actual
receipt under GWP-20260912-01. Runtime-profile governance, residual strict debt,
full language/provider/release gates, immutable packaging and packaged
runtime/UI/Docker acceptance remain open. No persistent service changed; the
final runtime target remains persistent 4200 and no persistent 4226.
