# HTTP router registration extraction

Date: 2026-09-11. Owner: parallel coordinator. State: `structurally_verified`.
Scope: [HTTP router lane](../plan/parallel-lanes/http-router.md).

## Source result

`src/http/router.rs` decreased from 1,011 to 40 effective lines. It retains UI and
observability routes, ordered domain mounting, the global body limit, request
lifecycle/logging, CORS and final state attachment. All eight owned source/test
files are at most 235 effective lines.

| Private registration owner | Effective lines |
| --- | ---: |
| `router/inference.rs` | 235 |
| `router/management_access.rs` | 197 |
| `router/management_config.rs` | 200 |
| `router/request_audits.rs` | 83 |
| `router/credential_lifecycle.rs` | 94 |
| `router/analysis_reports.rs` | 235 |

Each module accepts and returns the existing typed router. The inference owner
also borrows the existing application state for configured body limits. No new
state owner, handler abstraction, dynamic registration table or dependency was
introduced. The new layer contract contains 156 effective lines. No exception or
baseline regeneration was needed.

Per-file counts, SHA-256 hashes, the exact original source hash, source proof and
independent repository states are in
[`scope.json`](../../target/effective-line-evidence/20260911-http-router/scope.json).
`router.before.rs` is the immutable original source. `extract-router.mjs --verify`
reproduces the complete comparison against that snapshot.

## Preservation and boundary review

The comparison preserves 233 route registrations and 31 separately limited
routers. All original path strings, path-parameter names, HTTP methods, handler
expressions, configuration fields and compatibility aliases are retained. The
public subrouter definitions and merge order are unchanged; domain registration
continues in the original destination-router order. All six complete module
bodies and the entry's final wiring match the controlled expected source after
official formatting.

Review of each owner confirms that registration still happens only at startup.
The code adds no request-body buffering, parsing, task, process, lock, await,
cache or cancellation boundary. Handlers retain admission and resource ownership.
The entry still attaches the global body limit inside request logging/lifecycle
and CORS outermost, then supplies the same `Arc<AppState>` to the final router.

## Fresh verification

The same seven targets passed before and after extraction:

| Target | Passed |
| --- | ---: |
| `image_edit_ingress_limits` | 3 |
| `ingress_extractor_limits` | 3 |
| `internal_management_access_contract` | 4 |
| `internal_management_routes_contract` | 4 |
| `operator_summary_contract` | 10 |
| `router_layer_contract` | 3 |
| `smoke` | 17 |
| Total | 44 |

The new tests characterize 33 public/native/compatibility endpoint boundaries at
the exact configured byte limit and one byte beyond it. Limits differ by family
to detect a wrong configuration field. They also verify the global limit across
six route groups, CORS preflight before drain rejection, drain rejection before
the body limit, request-ID propagation, and health access while draining. They
use synthetic state and no live provider, PostgreSQL or credential storage.

All-targets compilation, scoped official rustfmt, checker tests 19/19, ratchet,
UTF-8 without BOM, whitespace and independent Gateway/Neuro Git checks passed.
Logs are under `target/effective-line-evidence/20260911-http-router/`:
`baseline-tests.log`, `final-tests.log`, `final-all-targets.log`, `scoped-fmt.log`,
`global-fmt.log`, `checker-tests.log`, `ratchet.log`, `gateway-diff-check.log`,
`neuro-diff-check.log` and `strict-inventory.json`.

Global formatting still reports only the reserved S06 files
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. Those files were not edited.

## Remaining plan and release boundary

Strict scans 1,399 files: 31 hard, 54 mandatory and 40 soft. There are still 85
files above 700; structural clearance is 60/145 (41.4%). Strict remains red for
that debt. This checkpoint completes router registration extraction only.

No release was built or modified. The GWP-20260908-06 source/docs freeze and
shared-build receipt remain outstanding. S06 retains its source and original
plan cursor. Remaining structural work and integrated runtime/UI/Docker release
acceptance remain open.
