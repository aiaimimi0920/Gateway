# Gateway Module Inventory

## High-Level Inventory

| Area | Representative files | Responsibility | Productization phase |
| --- | --- | --- | --- |
| Runtime | `src/main.rs`, `src/runtime.rs`, `src/config.rs`, `src/state.rs`, `src/splitter.rs` | Process roles, dependency wiring, lifecycle, worker replacement | Phase 1 and 3 |
| HTTP | `src/http/router.rs`, `src/http/routes/*.rs`, `src/http/request_headers.rs` | Public and internal route contracts, extraction, auth header propagation | Phase 1 lifecycle; Phase 3 controls |
| Pipeline | `src/pipeline/*.rs` | Canonical request execution and quota/audit finalization | Cross-phase invariant |
| Routing | `src/routing/*.rs` | Route configuration, candidate scoring, protocol resolution, credential affinity | Phase 2 evidence; Phase 3 reliability |
| Protocol | `src/protocol/**` | Provider request normalization and response translation | Phase 2 provider closure |
| Upstream | `src/upstream/**` | Direct HTTP, browser-backed, websocket, and media execution | Phase 2 provider closure; later modularization |
| Credentials/DB | `src/auth/**`, `src/credential_*.rs`, `src/db/**` | Key issuance, provider credentials, quota, audits, inventory, remediation | Phase 2 and 3 |
| Metrics/health | `src/metrics/**`, `src/http/routes/health.rs` | Runtime probes and sliding-window measurements | Phase 3 |
| Manifests | `manifests/lines/**`, `manifests/schema/**` | Provider-line identity, feature and validation metadata | Phase 2 |
| Browser workers | `scripts/*.mjs`, `scripts/tests/**` | Selected browser-backed provider execution and offline contracts | Phase 1 packaging; Phase 2 provider closure |
| Desktop backend | `apps/desktop/src-tauri/src/{process,profile,paths,state,logs}.rs` | Profile persistence, sidecar process, path checks, logs | Phase 1 |
| Desktop frontend | `apps/desktop/src/{App,features,lib,state}` | Configuration UI, onboarding, probes, API tests, diagnostics | Phase 1 |
| Build/release | `tools/*.ps1`, root release delegation | Build, artifact checksums, UI smoke, package staging | Phase 1 |
| Tests | `tests/*.rs`, `tests/python/*.py`, `scripts/tests/*.mjs` | Unit, contract, manifest, worker, and release-candidate tests | All phases |

## Complexity Hotspots

| File/area | Current concern | Planned handling |
| --- | --- | --- |
| `src/upstream/client.rs` | Concentrates many provider and browser execution paths | Do not rewrite in Phase 1; split one provider family at a time after evidence coverage exists. |
| `src/protocol/gemini_canvas.rs` and related modules | Large protocol and media surface with mixed execution modes | Keep behavior stable; add Phase 2 capability/evidence boundaries before extraction. |
| `src/db/remediation.rs` | Large operator and remediation state model | Phase 3 observability/recovery work may extract focused repositories. |
| `apps/desktop/src/state/useGatewayDesktopState.ts` | Coordinates most UI state and probes | Phase 1 adds narrow helpers and contract tests before extracting state slices. |
| `tests/python/test_gateway_desktop_ui_contract.py` | Large source-contract test file | Add focused test modules for packaging and profile behavior rather than growing one file indefinitely. |

## Phase 1 File Map

### Create

- `tools/package-gateway-release.ps1` — Gateway-owned staged package builder.
- `tools/smoke-gateway-packaged-runtime.ps1` — isolated packaged runtime E2E.
- `tests/python/test_gateway_package_contract.py` — package layout contract.
- `tests/python/test_gateway_packaged_runtime_contract.py` — E2E script contract.

### Modify

- `tools/build-gateway-release.ps1` — expose reusable build/stage primitives.
- `tools/smoke-gateway-ui-release.ps1` — validate support files and package schema.
- `apps/desktop/src-tauri/src/process.rs` — portable path resolution and drain auth.
- `apps/desktop/src-tauri/src/profile.rs` — runtime role and dependency validation.
- `apps/desktop/src-tauri/src/state.rs` — lifecycle metadata if required.
- `apps/desktop/src/lib/types.ts` — profile/runtime role and probe fields.
- `apps/desktop/src/lib/profileTemplates.ts` — explicit local mode templates.
- `apps/desktop/src/lib/profileValidation.ts` — client-side role/path checks.
- `apps/desktop/src/state/useGatewayDesktopState.ts` — probe/start/stop state.
- `tests/python/test_gateway_desktop_ui_contract.py` — only focused assertions
  where existing contracts must change.

## Dependency Rule

Desktop code may call Gateway HTTP and Tauri commands. It must not import or
duplicate `pipeline`, `routing`, `protocol`, or provider credential logic.
Provider-line code must continue to enter through the canonical pipeline.
Release tools may copy Gateway support files but must not rewrite runtime
configuration semantics.
