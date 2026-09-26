# Console runtime and HTTP ownership acceptance - 2026-09-11

This production-source batch is structurally verified. Runtime decreased from
1,087 to 334 effective lines and Console HTTP handlers from 822 to 314. The two
clearances advance original structural progress to 54/145 (37.2%). The full
optimization and integrated release plan remains incomplete, with 91 oversized
files still recorded by strict mode.

## Implemented boundaries

Public runtime types, backend contracts, coordinator/replica fields and runtime
role selection remain in `src/console/runtime.rs`. Complete existing methods
now belong to commit, activation, startup recovery, revision-state and snapshot
owners. No existing async method was split internally, so guard lifetimes,
await order, rollback decisions and publication sequence remain intact. Relative
type paths in moved code were qualified to the same existing Console types.

The HTTP root retains wire types, shared authentication/context/header/error
helpers and Gemini session handlers. Session, probe, route-config and revision
handlers have private owners and are re-exported at their existing public paths.
Router registration and the three Gemini handler bodies are unchanged. The
underlying Gemini service and S06 source remain outside this batch.

| Source file | Effective lines | Responsibility |
| --- | ---: | --- |
| `src/console/runtime.rs` | 334 | Public runtime roles, backend contracts and shared state |
| `src/console/runtime/activation.rs` | 128 | Activation outcomes, final publication and rollback |
| `src/console/runtime/commit.rs` | 249 | Complete guarded commit future |
| `src/console/runtime/recovery.rs` | 235 | Startup reconciliation against durable and Redis state |
| `src/console/runtime/revision_state.rs` | 95 | Receipt, digest and revision-state projections |
| `src/console/runtime/snapshots.rs` | 112 | Verified snapshot and YAML installation |
| `src/http/routes/internal_console.rs` | 314 | Wire types, common boundaries and existing Gemini routes |
| `src/http/routes/internal_console/probes.rs` | 173 | Credential/provider probe authorization and results |
| `src/http/routes/internal_console/revisions.rs` | 146 | Active/archived revision history and payloads |
| `src/http/routes/internal_console/route_config.rs` | 155 | Route reads, validation, revision and secret admission |
| `src/http/routes/internal_console/sessions.rs` | 104 | Bootstrap, verification, grants, rotation and logout |

All 11 owned files remain below 500 effective lines. No dependency, policy,
baseline, exception or route-configuration change was required.

## Preservation and review

Controlled-source comparison reconstructs both complete roots and every child
from the captured originals. Allowed transformations are imports, module/handler
registration, necessary private visibility, relocated type qualification and
purpose comments. Function bodies and operation ordering are otherwise retained.
The HTTP verifier separately checks exact equality of the three Gemini handlers.
The 51 earlier test, document/secret and persistence/journal sources still match
their recorded hashes; no test assertion was changed or removed.

The coordinator reviewed the complete moved code for auth/grant ordering,
public paths, response shapes, errors, state and resource ownership. Existing
handler extraction types, no-store/ETag behavior and error status mapping remain
unchanged. Runtime writer guards still span the same awaited operations. No new
I/O operation, task, queue, allocation site or algorithm was introduced. This
structural checkpoint makes no additional resource-hardening claim. Independent
review is not claimed because the required default-agent model was unavailable
in the preceding attempts.

## Fresh verification

Paired baseline and final lock/failure tests pass 2/2:

```powershell
cargo test --offline --locked --lib console::persistence_lock_contract -- --test-threads=1
```

The same nine integration targets pass 152/152 before and after extraction:

```powershell
cargo test --offline --locked `
  --test console_document_contract `
  --test console_management_contract `
  --test console_management_sensitive_contract `
  --test console_credential_probe_contract `
  --test console_transaction_contract `
  --test console_transaction_runtime_contract `
  --test console_persistence_contract `
  --test console_hot_reload_contract `
  --test console_session_contract -- --test-threads=1 --nocapture
```

Counts are document 58, management 12, sensitive input 2, probe 6, transaction 4,
transaction runtime 12, persistence 40, hot reload 13 and session 5. The hot-reload
cases cover complete-snapshot visibility, concurrent-writer lineage and provider
runtime-state reuse. Session cases exercise real isolated bootstrap, Argon hash
persistence, remote rejection, token rotation, secret confirmation and logout.

Cargo reports 154 passed across the unit and integration gates, but one existing
persistence symlink case returns early on Windows error 1314. Its link-rejection
assertion is not executed platform proof. The guard remains unchanged.

Other fresh gates:

- Intermediate `cargo check --offline --locked --lib`: exit 0.
- Final `cargo check --offline --locked --all-targets`: exit 0.
- Scoped Rust formatter for both roots and children: exit 0.
- Complete runtime and HTTP source-equivalence checks: passed.
- Effective-line checker: 19 passed, zero failures.
- Effective-line ratchet: exit 0, zero new violations.
- Strict: exit 1; 1,345 scanned, 33 hard, 58 mandatory and 40 soft;
  91 files remain above 700.
- Gateway and Neuro `git diff --check`: exit 0, evaluated independently.
- Scoped sources: strict UTF-8 without BOM or NUL.
- Neuro development-standard contract: passed.

No new compiler warning was introduced. The normal library retains three
production warnings and the unit-library gate one. Full `cargo fmt --all --
--check` still exits 1 only for S06's
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`; it is not reported as green.

## Evidence and release state

Evidence root: `target/effective-line-evidence/20260911-console-runtime-http/`.

- `runtime.before.rs`, `internal_console.before.rs`: original source snapshots.
- `split-runtime.mjs`, `split-http.mjs`: extraction and controlled-source verifiers.
- `baseline-locks.log`, `baseline-contracts.log`: paired pre-change gates.
- `runtime-check.log`: intermediate compilation.
- `after-locks.log`, `after-contracts.log`, `all-targets-check.log`: final Cargo gates.
- `runtime-equivalence.log`, `http-equivalence.log`: final source comparisons.
- `scoped-format.log`, `global-format.log`, `checker-tests.log`, `ratchet.log`,
  `strict-final.json`, `gateway-diff-check.log`, `neuro-diff-check.log`,
  `development-standard.log`: remaining gates.
- `scope.json`: before-owner measurements and 11 final source hashes, not a
  whole-product provenance manifest.

Changes remain inside Gateway's two roots, nine new child modules and progress
documents. Existing dirty worktrees and the S06 cursor were preserved; no staging,
commit, reset or sibling-project edit was performed. S10, S14, S21 and overall
optimization are not marked complete.

The latest existing release was refreshed on disk and remains
`C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\20260908-producer-mailbox-s06-123700`,
with its manifest present. It predates this source checkpoint. No new package was
built because GWP-20260908-06 still lacks an explicit source/docs freeze and
shared-build transfer receipt. The next integrated release must include all
required untracked owners and pass the official packaging and runtime gates.
