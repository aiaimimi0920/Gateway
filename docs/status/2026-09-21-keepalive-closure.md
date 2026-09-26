# Keepalive structural closure

Date: 2026-09-21. Status: structural_green; full-goal and release work remain active.

## Result and ownership

The remaining keepalive source is split by authentication material, metadata,
HTTP headers, local-browser admission, provider admission probes and Suno runtime
material. `src/keepalive.rs` decreases from **1338 to 481 effective lines**.
It retains management orchestration and runtime-session mapping/persistence.
Existing Gemini mapping/material behavior remains in that parent; no reserved
S06 implementation is edited.

All **29 Rust files** in the complete keepalive owner tree, including the parent
and tests, are now <=481 effective lines. No soft-limit exception is required.
This completes the structural size target for keepalive with the passing
regression gate below; it does not complete whole-repository refactoring or
unresolved resource hardening.

| New owner under `src/keepalive/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `auth_material.rs` | 54 | Session transport normalization and JWT expiry decoding |
| `metadata.rs` | 78 | Metadata readers and diagnostic summaries |
| `headers.rs` | 143 | External header projection and authenticated probe construction |
| `browser_policy.rs` | 25 | Local-browser admission errors shared by refresh workers |
| `probes.rs` | 354 | Chataibot/LumaLabs/Producer admission and preserved fallthrough material |
| `suno.rs` | 316 | Browser-cookie selection and ordered Suno runtime probes |
| `tests/probes.rs` | 213 | Loopback admission, failure and early-return regressions |

Strict before/after: 2213 -> 2220 scanned; hard 10 -> 10, mandatory 19 -> 18,
soft 39 -> 39. Strict remains exit 1, with **28 files above 700**, down from 29.
Only keepalive changes among the oversized entries. No checker, baseline,
exception, dependency, runtime profile or exclusion change is involved.

## Preservation and verification

Evidence directory: `target/effective-line-evidence/20260921-keepalive-closure/`.
The saved parent baseline SHA-256 is
`443fc3b3b3699893d342f9241f36b72930fd12f7b56b39972ef7c3a243dc6c37`.
`extraction-proof.json` records the baseline identity, all 29 owner counts,
strict snapshots and exact formatted projection results.

Complete reader/header/policy functions retain their original bodies. The
admission block retains branch order, HTTP methods/paths, expiry guards,
response decoding and errors; its 12 early responses are wrapped in
`ControlFlow::Break`. Success returns the same owned `ProbeMaterial` in
`ControlFlow::Continue`, preserving generic DB/Redis fallthrough without adding
material clones. The root handles that distinction before the Suno branch.
Suno retains its terminal response and never enters generic persistence.

Expected owners and retained parent compare byte-for-byte after independent
rustfmt, allowing only the documented ownership wiring, visibility and
ControlFlow adaptation. Prior ChatGPT/Qwen owners, request dispatcher, serde
types and all existing test bodies also remain identical to their preceding
formatted projections. A read-only independent review found no extraction
regression.

Four new tests cover eight provider response cases, two expired-token cases,
missing Luma realm and missing Chataibot quota. They verify the public management
entry's readiness/error result, material preservation, request path, Bearer auth,
single request and network-free early returns. Fixture server tasks abort on
drop, including panic unwinding; tests use ephemeral loopback ports and no
external Redis/PostgreSQL service.

| Check | Result |
| --- | --- |
| Before: `cargo test --locked --lib keepalive::tests -- --test-threads=1` | 24 passed |
| After: same command, including four new tests | 28 passed |
| `cargo test --locked --lib provider_credential_refresh::tests -- --test-threads=1` | 3 passed |
| `cargo check --locked --all-targets` | Passed; three inherited Gemini warnings |
| `npm run test:effective-lines --prefix scripts` | 19 passed |
| `npm run check:effective-lines --prefix scripts` | Passed |
| `rustfmt --edition 2021 --check src/keepalive.rs` | Passed, including children |
| Exact projections and UTF-8/no-BOM source audit | Passed |
| Separate Gateway and Neuro `git diff --check` | Passed |

Global `cargo fmt --all -- --check` still exits 1 on unchanged
`src/provider_credential_folder_sync/paths.rs`,
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. The last two remain reserved
S06 files. `global-format.log` preserves the current evidence; no global-format
success is claimed.

## Safety, resources and remaining hardening

- Auth/metadata retain normalization, alias precedence, JWT decoding, expiry
  behavior and diagnostics. Token/body bounds and diagnostic disclosure are
  inherited review items; no new secret log or storage path is introduced.
- Headers retain internal-selector filtering and case-insensitive updates.
  Invalid names/values are still skipped. Cookie deduplication complexity and
  unbounded upstream material remain unchanged.
- Admission probes preserve remote error vs not-ready response semantics, Luma
  redirect refusal, Chataibot quota interpretation and Producer expiry checks.
  Existing body reads and per-probe timeout policy need separate hardening.
- Suno preserves storage-state precedence, cookie domain/expiry/path selection,
  user_config status handling, challenge JSON rules, derived bearer/expiry and
  terminal return. Cookie secure/same-site modeling is unchanged.
- Ownership extraction adds no queue, task, retry, material clone or production
  heap allocation. Shared policy admission still precedes browser worker launch.
  The existing ChatGPT/Qwen process cancellation/kill-reap and unbounded-output
  issues recorded in their extraction reports remain open.
- Parent session persistence and cache writeback order remain unchanged. No
  new transaction, lock or cross-await critical section is introduced.

## Whole-goal continuation and release

The active objective remains completion of the entire splitting/optimization
plan. See `2026-09-21-refactor-completion-audit.md` for the full acceptance matrix:
16 remaining >700 Rust files, 12 retained runtime-profile artifacts, 39 soft
entries, integrated gates, hardening, release and runtime proof remain subject
to their actual completion criteria.

An explicit current transfer of the reserved S06 source and shared build window
was requested. No transfer or package is inferred from an old handoff or absence
of a process. Unreserved soft-limit/hardening work remains available meanwhile.
Release destination stays `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
No release build/package/deployment, live profile edit or service restart occurred.

Inherited dirty worktrees are preserved. Scoped Gateway status contains this
parent, new owners/tests and checkpoint documents; Neuro retains its modified
Gateway submodule entry. No staging, commit, push or sibling source edit occurred.
