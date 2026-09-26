# Qwen keepalive extraction

Date: 2026-09-21. Status: structural_green. This is a structural
increment, not full keepalive clearance or a released version.

## Scope and measurements

The ongoing effective-line refactor continues with Qwen ownership extracted
from `src/keepalive.rs`. The parent decreases from 3190 to 2329 effective
lines (861 fewer); its remaining providers, shared dispatch/readers and tests
remain outstanding debt. The accepted ChatGPT owners are unchanged.

| New owner under `src/keepalive/qwen_web/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `mod.rs` | 78 | Freshness and HTTP-first/browser fallback orchestration |
| `types.rs` | 79 | Worker wire DTOs and refreshed runtime |
| `input.rs` | 77 | Filename normalization and worker input construction |
| `signin.rs` | 363 | Signin seed, password attempts, headers and HTTP signin |
| `worker.rs` | 204 | Browser child launch, input, wait and output decoding |
| `runtime.rs` | 134 | Runtime projection and ordered material persistence |

All extracted owners are below 500 effective lines, measured with the repository
lexer after official rustfmt, and use UTF-8 without BOM. No baseline, checker,
dependency or exclusion was changed. Strict scans cover 2199 -> 2205 files;
both report 11 hard, 18 mandatory and 39 soft entries and exit 1. Only the
parent changes its hash among files over 700 lines. The 29-file debt count is
unchanged; this increment does not claim the parent debt has been cleared.

## Preservation and evidence

Evidence directory: `target/effective-line-evidence/20260921-keepalive-qwen/`.
The saved `before.rs` captures the parent immediately after ChatGPT extraction.
Its SHA-256 is
`eb9f0ae97d92444d3a090aa271f83fe3d6d2454428d85b2b3981533fdaaf1838`.
`extraction-proof.json` records the measured source projection and counts.
Exact source projections passed before and after rustfmt: expected moved owners
and retained parent were compared byte-for-byte after independent formatting.
Only imports, ownership wiring and keepalive-local visibility changed. All
original function/test bodies, serde attributes, field order, constants and
timeouts are preserved. The ChatGPT source projections also remain identical.

The public challenge-refresh path remains available through `crate::keepalive`.
Normal readiness still attempts direct HTTP signin before browser fallback;
management credential refresh deliberately remains browser-only. Shared readers,
headers, JWT handling and browser admission remain parent-owned. Redis writeback
still precedes PostgreSQL persistence. A read-only independent review found no
extraction regression or unintended public API expansion.

| Verification | Result |
| --- | --- |
| Before: `cargo test --locked --lib keepalive::tests -- --test-threads=1` | 24 passed |
| Before: `cargo test --locked --lib qwen -- --test-threads=1` | 49 passed |
| After: same keepalive command | 24 passed |
| After: same Qwen command | 49 passed |
| `cargo check --locked --all-targets` after import cleanup | Passed; three inherited Gemini warnings |
| `npm run test:effective-lines --prefix scripts` | 19 passed |
| `npm run check:effective-lines --prefix scripts` | Passed |
| `rustfmt --edition 2021 --check src/keepalive.rs` | Passed, including children |
| Exact formatted projections and encoding | Passed |
| Gateway and Neuro independent `git diff --check` | Passed |

The first all-target compile passed with one newly unused root `tracing::warn`
import plus three inherited Gemini warnings. The unused import was removed;
`check-final.log` records the passing final compilation and
`after-tests-final.log` records 24 passing keepalive tests on the final source.
No unrelated formatting is
applied. The prior checkpoint records global formatter debt in folder-sync paths
and two reserved Gemini runtime-mirror files; this increment claims scoped
formatting only.

## Safety and resource review

- Types/input preserve serde omission/defaults, environment lookup timing and
  credential filename normalization. No new secret logging or material storage
  is introduced.
- Signin preserves password attempt order, SHA-256 handling, endpoint, errors
  and expiry semantics. Existing unbounded HTTP bodies, email diagnostics,
  per-call `Client::new()` and case-sensitive Authorization removal remain
  separate hardening concerns.
- Worker preserves admission, process arguments, JSON protocol and 120-second
  wait. Existing stdin/timeout cancellation paths do not explicitly kill/reap
  the child, and stdout/stderr collection remains unbounded.
- Runtime preserves internal-header filtering, material attribution and
  best-effort persistence: Redis write errors are ignored and PostgreSQL errors
  warn. Extraction does not strengthen those semantics.
- Orchestration retains fallback order and persistence before returning. No
  new task, queue, retry, blocking operation or algorithmic complexity is added.

## Release and remaining work

No package was built or published for this increment. The historical S06 shared
build/release reservation under `GWP-20260912-01` remains untransferred; the
previous request for current ownership confirmation has no response. Scoped
compilation/tests do not prove product, UI, Docker or live-provider acceptance.
After ownership transfer, use the official builder/packager and verify a new
immutable version under
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.

The inherited dirty Gateway checkout is preserved without staging, commits or
pushes. Neuro retains its modified Gateway submodule entry; this increment does
not edit parent or sibling sources. Next boundaries are the remaining provider
probes and shared dispatch/test owners. Full root clearance, repository strict
closure, existing resource hardening and the requested release remain open.
