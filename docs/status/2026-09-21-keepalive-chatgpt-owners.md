# ChatGPT keepalive extraction

Date: 2026-09-21. Status: structural_green; not a full keepalive
or repository-refactor completion, and not a released version.

## Recovered task and scope

The ongoing task is the effective-code-line refactor recorded in
`docs/plan/2026-09-03-gateway-effective-line-refactor.md` and the parallel board.
The latest coordinator checkpoints cover folder-sync and local object atomic
writes. A fresh strict scan found 29 files above 700 effective lines, including
12 browser-profile legacy entries. The prior immutable package found in the
requested release root is `20260908-producer-mailbox-s06-123700`.

This increment extracts the ChatGPT refresh family from `src/keepalive.rs`.
The parent retains multi-provider dispatch and shared readers/header handling.
No other provider implementation, dependency, line policy, baseline, exception,
runtime profile or existing release was changed by this increment.

## Structure and measurements

Measurements use the repository's language-aware lexer after official rustfmt.
All new files are below 500 effective lines and UTF-8 without BOM.

| Path under `src/keepalive/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `chatgpt_web/mod.rs` | 178 | Refresh/relay orchestration and existing provider entry points |
| `chatgpt_web/types.rs` | 179 | Worker wire messages and refreshed material |
| `chatgpt_web/input.rs` | 248 | Sign-in seed and worker input construction |
| `chatgpt_web/policy.rs` | 92 | Session freshness and browser-fallback admission |
| `chatgpt_web/oauth.rs` | 191 | OAuth timing, HTTP refresh and token decoding |
| `chatgpt_web/runtime.rs` | 305 | Runtime projection and ordered persistence |
| `chatgpt_web/worker.rs` | 289 | Browser worker launch and result decoding |

`src/keepalive.rs` decreases from **4568 to 3190** effective lines, a reduction
of **1378**. Its remaining Qwen/Suno/shared dispatch and tests are unfinished
debt. This checkpoint does not count the root as cleared.

Strict before/after: 2192 -> 2199 scanned; 11 hard, 18 mandatory and 39 soft in
both scans. Strict exits 1 in both snapshots. Among oversized source entries,
only `src/keepalive.rs` changes its source hash/count. The 29-file debt count is
unchanged. No exception or baseline regeneration was used.

## Preservation and verification

Evidence directory:
`target/effective-line-evidence/20260921-keepalive-chatgpt/`.

The saved pre-change `before.rs` has SHA-256
`4af688fe697e9fde84b19a02b29f541ff0cf54487299849cd33222a876b1e827`.
Exact extraction projection was checked before formatting. After formatting,
the seven expected owners and retained parent were independently formatted via
`rustfmt --edition 2021 --emit stdout --config skip_children=true` and compared
byte-for-byte with the actual files. This preserves all original test bodies,
function bodies, constants, serde attributes and field order. Only ownership
wiring, imports, module comments and keepalive-local visibility change.

Existing `crate::keepalive` public entry paths and the crate-visible OAuth due
predicate remain re-exported. The provider credential steward and pipeline
callers need no edits. Shared browser-policy errors remain parent-owned so
Qwen behavior is unchanged.

| Verification | Result |
| --- | --- |
| Before: `cargo test --locked --lib keepalive::tests -- --test-threads=1` | 24 passed |
| After: same keepalive command | 24 passed |
| `cargo test --locked --lib provider_credential_refresh::tests -- --test-threads=1` | 3 passed |
| `cargo check --locked --all-targets` | Passed; three existing Gemini warnings |
| `npm run test:effective-lines --prefix scripts` | 19 passed |
| `npm run check:effective-lines --prefix scripts` | Passed |
| `rustfmt --edition 2021 --check src/keepalive.rs` | Passed, including new children |
| `git diff --check` in Gateway | Passed |
| UTF-8/BOM and exact source projection | Passed |

Gateway scoped Git status contains the modified parent and new ChatGPT subtree
plus this checkpoint's documents. The inherited dirty checkout was preserved;
no staging, commit or push was performed. The Neuro parent reports its existing
modified Gateway submodule entry; its separate `git diff --check` also passes.
No parent source or sibling-project file was edited by this increment.

The first compile found a missing keepalive-local visibility annotation on
`ChatGptWebSessionWorkerAuthSeed.password_sha256`. It was corrected without
changing the field type or wire representation; `check.log` retains that
failure and `check-retry.log` records the passing retry. A read-only independent
review found no additional extraction regression; compiler and source proofs
remain authoritative.

Global `cargo fmt --all -- --check` still exits 1 on untouched files:
`src/provider_credential_folder_sync/paths.rs`,
`src/upstream/gemini_canvas_runtime_mirror.rs`, and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`.
The latter two remain within the reserved S06 scope. No global-format success
is claimed and unrelated files were not reformatted.

## Scoped safety and resource review

- Types/input preserve serde aliases/defaults, field omission, filename
  normalization and environment lookup timing. No new secret copy or logging
  path is introduced beyond the original code.
- Policy preserves global browser admission before worker launch, provider
  fallback precedence and expiry behavior. No new queue, task or retry exists.
- OAuth preserves the form, endpoint/client selection, 30-second timeout,
  token rotation and existing error codes. Existing unbounded response reads,
  diagnostic disclosure and unchecked extreme expiry arithmetic still need a
  separately tested hardening change.
- Runtime preserves Redis then PostgreSQL ordering, best-effort failures,
  internal-header filtering, metadata merge and account attribution. Existing
  persistence semantics are not strengthened by a source move.
- Worker preserves process ownership, input/output protocol, 150-second wait
  and response precedence. Existing stdin/wait cancellation paths do not
  explicitly kill/reap the child, and stdout/stderr collection is unbounded.
  These are open lifecycle/resource risks, not repaired or hidden by this split.
- Orchestration retains persist-before-return and OAuth/browser fallback order.
  No added dynamic dispatch, cloning, allocation, blocking path or complexity
  increase was introduced by extraction.

## Release and next boundary

No new package has been built or published by this checkpoint. The historical
S06 source/build reservation remains in the handoff under `GWP-20260912-01`.
A current transfer confirmation was requested; an absence of a Gateway Cargo
process is not treated as an ownership transfer. The observed unrelated Beaver
builds were left alone.

After the shared build window is confirmed, freeze the integrated worktree,
run the documented builder, package a fresh immutable version under
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`, and execute
package/UI integrity and isolated runtime checks. Do not reuse the old package
or claim live-provider, Docker, UI or full product acceptance from unit tests.

Next structural boundary: Qwen authentication/worker/persistence, then the
remaining provider probes and shared dispatch/test owners in `keepalive.rs`.
The full refactor, strict closure, noted resource hardening and release remain
open.
