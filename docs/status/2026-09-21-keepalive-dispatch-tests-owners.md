# Keepalive dispatch and test extraction

Date: 2026-09-21. Status: structural_green; release_pending.

## Scope and structure

This increment continues the effective-line refactor after the ChatGPT and
Qwen owner extractions. It moves the keepalive wire contracts and request-time
steward dispatch into dedicated production owners, and groups existing tests
by provider or header/browser-policy contract. Shared readers, management
runtime dispatch and provider probes remain in `src/keepalive.rs`.

The parent decreases from **2329 to 1338 effective lines**, a reduction of 991.
It leaves the >1500 hard category but remains above the 700-line ceiling. This
checkpoint does not claim complete keepalive migration.

| New path under `src/keepalive/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `types.rs` | 81 | Request/response serde contracts |
| `ensure.rs` | 155 | Request-time provider/steward dispatch and material merge |
| `tests/mod.rs` | 5 | Test module declarations |
| `tests/chatgpt.rs` | 163 | Browser admission, OAuth exchange and header projection |
| `tests/qwen.rs` | 126 | Freshness, signin inputs and header projection |
| `tests/headers.rs` | 89 | Shared browser-policy errors and internal-header filtering |
| `tests/suno.rs` | 317 | Cookie selection and local HTTP runtime probes |
| `tests/gemini.rs` | 80 | Existing modular protocol and runtime-material contracts |

Counts use the repository lexer after official rustfmt. Every new owner is
below 500 effective lines. No dependency, exception, baseline or policy changed.
Strict before/after scans cover 2205 -> 2213 files, with hard entries 11 -> 10,
mandatory entries 18 -> 19 and soft entries 39 -> 39. Both strict scans exit 1;
the total above 700 stays 29. Only the parent changes among oversized files.

## Preservation and validation

Evidence: `target/effective-line-evidence/20260921-keepalive-dispatch-tests/`.
`before.rs` is the saved pre-edit parent. `extraction-proof.json` contains its
SHA-256, exact test-name inventory, source-projection results and line counts.

The expected extracted files and retained parent were independently formatted
and compared byte-for-byte with actual files. Function/test bodies, constants,
serde attributes, aliases, omission/default behavior and declaration order are
preserved. Allowed differences are module/import wiring and field visibility
spelled `pub(in crate::keepalive)` to preserve the original owning scope after
the types move. Public entry paths remain re-exported from `crate::keepalive`.

All 24 original test names and assertions remain; their full paths acquire a
provider/header test submodule. The existing `keepalive::tests` filter still
selects the complete group. The ChatGPT and Qwen production subtrees also pass
their previous exact formatted source projections. No reserved S06 source is
modified. A read-only independent review found no extraction regression;
mechanical source proofs and executed tests provide the definitive checks.

| Check | Result |
| --- | --- |
| Before: `cargo test --locked --lib keepalive::tests -- --test-threads=1` | 24 passed |
| After: same command, including final import cleanup | 24 passed |
| Final `cargo check --locked --all-targets` | Passed; three inherited Gemini warnings |
| `npm run test:effective-lines --prefix scripts` | 19 passed |
| `npm run check:effective-lines --prefix scripts` | Passed, including final import cleanup |
| `rustfmt --edition 2021 --check src/keepalive.rs` | Passed, including child owners |
| Exact formatted source and test inventory | Passed |
| UTF-8 without BOM and final Gateway diff check | Passed |
| Separate Neuro `git diff --check` | Passed |

The initial all-target compile passed with three inherited Gemini warnings and
newly unused root error-classifier imports. Those imports were removed and the
passing final compile/test evidence is recorded in `check-final.log` and
`after-tests-final.log`. The prior global-format
debt in folder-sync paths and two reserved Gemini runtime-mirror files is not
reformatted; this checkpoint claims scoped formatting only.

## Safety and resource review

- Wire types retain the same secret-bearing fields and serde representation;
  no new logging, copies or public field access outside keepalive are added.
- Dispatch preserves adapter admission and the existing ChatGPT/Qwen early
  branches, keepalive eligibility, steward URL/auth token and timeout.
- Network errors, unsuccessful status, invalid JSON and not-ready responses
  retain their original provider/error codes and ordering. Unbounded response
  bodies and upstream diagnostic disclosure remain inherited hardening work.
- Runtime projection preserves internal-header filtering, auth/expiry merge,
  body/header patch order and best-effort Redis writeback. The Suno exception
  still omits the returned API key from writeback. No new task, retry, queue,
  blocking operation or algorithmic work is introduced.
- Tests retain loopback ephemeral ports, mock payloads and explicit server
  aborts. Existing assertion-panic cleanup behavior is not strengthened by
  moving the tests. The refactor introduces no new production resource owner.

## Release and remaining work

The inherited dirty Gateway worktree is preserved; no staging, commit, push,
parent-source edit or sibling-project edit occurs. The parent Neuro repository
continues to report its modified Gateway submodule entry.

No release package is built in this increment. The historical S06 shared-build
reservation under `GWP-20260912-01` still awaits current ownership confirmation.
The requested destination remains
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
Once the shared window transfers, use the official builder/packager and run
package/UI integrity and isolated runtime checks on a new immutable version.
Unit tests and compilation do not substitute for release or live-provider proof.

Next ownership boundaries are shared auth/header readers, provider-specific
runtime probes and the remaining management orchestration. Full root clearance,
repository strict closure, resource hardening and release remain open.
