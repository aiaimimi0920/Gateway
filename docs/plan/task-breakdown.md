# Gateway Productization Task Breakdown

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-development` or `executing-plans` to implement this plan task-by-task. Every production behavior task follows a red-green-refactor cycle.

**Goal:** Deliver one portable, provider-evidenced, and operationally complete Gateway product from the existing Rust runtime and desktop shell.

**Architecture:** Keep the headless Rust runtime and canonical five-stage pipeline as the only request execution path. Add product boundaries around release packaging, desktop lifecycle, provider inventory/evidence, and enterprise operations without copying provider logic into the desktop shell.

**Tech Stack:** Rust/Axum/Tokio/Redis/optional PostgreSQL, Tauri 2 + React/TypeScript, PowerShell release tooling, Python contract tests, Node worker tests, Prometheus text metrics.

---

## Scope Rules

- Source edits are limited to `Gateway/`.
- Release output is limited to `release/Gateway/`.
- Preserve the existing uncommitted edits in `src/http/routes/images.rs`, `music.rs`, and `videos.rs`.
- Keep development credentials, cookies, tokens, JWTs, session material, and the current `routes.yaml` unchanged.
- Do not make real upstream calls part of ordinary CI. Live canaries are explicit operator actions.

## Task Sequence

### Phase 1A: Portable package and path contract

1. Add `tests/python/test_gateway_package_contract.py` with a failing contract for a deterministic package layout, `manifest.json`, `checksums.sha256`, both executables, routes, manifests, scripts, and an explicit support-file list.
2. Add `tools/package-gateway-release.ps1` with parameters for source root, version id, output root, build toggle, and clean staging. Stage only Gateway-owned files and emit the manifest/checksum files.
3. Extend `tools/build-gateway-release.ps1` with a stable output contract consumed by the packager; retain its existing build and retry behavior.
4. Add `tests/python/test_gateway_packaged_runtime_contract.py` for the isolated runtime runner and its required assertions.
5. Add `tools/smoke-gateway-packaged-runtime.ps1` to launch the staged headless binary with an isolated port/Redis, verify `/healthz`, `/readyz`, `/v1/models`, malformed request behavior, drain, and process cleanup.
6. Run the package contract, runtime contract, manifest validator, and existing release tests before moving to desktop changes.

### Phase 1B: Desktop profile and lifecycle

1. Add a failing Rust contract test for `runtimeRole`, `gatewayManagementToken`, and relative path resolution from the executable/package directory.
2. Add explicit role and management-token fields to `GatewayProfile`, reserve the corresponding environment keys, and serialize them with the existing camelCase schema.
3. Replace compile-time source-path fallbacks with a portable resolver: configured working directory, then executable directory, then a clear error. Resolve relative routes against the resolved working directory.
4. Add dependency probes for the sidecar executable, Redis, optional PostgreSQL, and routes file. Return actionable probe messages without logging secret values.
5. Pass `GATEWAY_RUNTIME_ROLE`, `GATEWAY_MANAGEMENT_TOKEN`, and resolved `GATEWAY_ROUTES_FILE` to the child process. Send the management token as an `Authorization: Bearer` header during drain.
6. Extend `GatewayProcessSnapshot` with explicit startup/drain/termination states and preserve the recent log tail.
7. Update TypeScript profile types, templates, validation, onboarding, and diagnostics to display the new states and probe results.
8. Run focused Rust and TypeScript tests, then the complete desktop typecheck and Python UI contract suite.

### Phase 1C: Isolated packaged E2E

1. Stage a package outside the source checkout under `release/Gateway/<versionId>`.
2. Run `smoke-gateway-packaged-runtime.ps1` twice consecutively with a temporary Redis and unique port.
3. Run `smoke-gateway-ui-release.ps1` against the same package and verify that the UI does not auto-start a sidecar before an explicit start command.
4. Record package manifest, checksums, runtime logs, and E2E result under the release directory; do not include secrets in generated evidence.

### Phase 2A: Inventory and evidence

1. Add `tests/python/test_gateway_provider_inventory_contract.py` with a failing schema and manifest-coverage test.
2. Add `tools/generate-gateway-provider-inventory.py` to join line manifests, Cargo features, implementation-line constants, route/protocol metadata, and verification references.
3. Emit `docs/provider-inventory.json` with schema `gateway-product-inventory/v1`, deterministic ordering, source revision, capability matrix, and evidence summary.
4. Add an evidence writer/validator with states `compiled`, `fixture_passed`, `live_passed`, `external_gate`, `credential_missing`, and `known_unsupported`; require timestamps and classified failure data for non-compiled states.
5. Add a Python test that every manifest appears exactly once and every declared feature/path exists.

### Phase 2B: Verification matrix and canary evidence

1. Add `tools/run-gateway-line-evidence.ps1` with default offline fixture mode and explicit `-AllowLiveProviderCalls` mode.
2. Reuse `tools/verify-gateway-line.ps1` focused filters and `scripts/invoke-gateway-live-provider-canary.ps1` classification rather than duplicating provider execution.
3. Persist one JSON evidence record per run under `docs/evidence/` with provider line, execution mode, endpoint family, credential source label, status, failure class, and artifact paths.
4. Add contract coverage for official HTTP, browser-backed, search, media, and websocket representative lines.
5. Add operator documentation explaining external gates for quota, challenge, account, region, and upstream protocol failures.

### Phase 2C: Provider/browser operational closure

1. Add readiness diagnostics for remote executor, local browser fallback, session material, credential refresh, lease/cooling, and model health.
2. Add tests proving each execution mode is reported explicitly and does not silently fall back without an evidence record.
3. Run the full offline line matrix and one opt-in canary per available credential family; classify unavailable credentials as `credential_missing` instead of failing the Gateway build.

### Phase 3A: Observability and failure contracts

1. Add Rust tests for request-id propagation, structured request/provider dimensions, latency/TTFT/error fields, and drain rejection logging.
2. Add a Gateway-owned metrics registry for request, provider, credential, model, endpoint, runtime, quota settlement, and browser executor counters/histograms.
3. Extend `/metrics` and operator summary endpoints with stable names, labels, and build/runtime identity.
4. Add trace context extraction/injection for supported inbound and upstream headers while preserving sensitive-header masking.
5. Add an SLI/SLO document and alert threshold configuration consumed by an operator summary command.

### Phase 3B: Reliability and access isolation

1. Add fault-injection tests for Redis unavailable/slow/recovering, PostgreSQL unavailable/recovering, browser executor unavailable, and upstream timeout/retry exhaustion.
2. Add splitter/worker drain, replacement, timeout, forced termination, and rollback contract tests that assert audit and quota settlement invariants.
3. Add tenant/project/access-key checks and rate-limit contract tests at project, key, model, endpoint, and provider scopes.
4. Verify internal management and browser-executor routes fail closed when credentials are absent or invalid.

### Phase 3C: Recovery and final release

1. Add `tools/backup-gateway-state.ps1`, `tools/restore-gateway-state.ps1`, and `tools/verify-gateway-recovery.ps1` for isolated Redis/PostgreSQL/object-storage state.
2. Write `docs/operations-manual.md` with actual commands, endpoints, failure classes, drain/replacement procedure, backup/restore, and rollback.
3. Run the full Gateway validation gate, package gate, isolated E2E twice, offline provider matrix, and reliability fault matrix.
4. Generate the final package only under `release/Gateway/<versionId>` and record checksums and evidence.

## Verification Commands

```powershell
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
node --test scripts/tests/*.test.mjs
cargo test --manifest-path Cargo.toml --locked
Push-Location apps/desktop; npm run typecheck; Pop-Location
```
