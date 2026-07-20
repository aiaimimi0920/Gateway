# Gateway Productization Design

**Date:** 2026-07-18  
**Scope:** `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway`  
**Release root:** `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`

## Goal

Turn the existing Rust Gateway into a product-grade project through three
ordered milestones:

1. A portable local and Windows desktop delivery.
2. A provider-line capability and live-evidence product surface.
3. An enterprise runtime, observability, and operations baseline.

The result must remain one Gateway product with one canonical request pipeline,
not three unrelated implementations.

## Hard Constraints

- All source implementation for this effort stays inside `Gateway/`.
- Release artifacts are written only below `release/Gateway/`.
- Existing user changes in `src/http/routes/images.rs`,
  `src/http/routes/music.rs`, and `src/http/routes/videos.rs` are preserved.
- Development-time credentials, cookies, tokens, JWTs, session material, and
  the current `routes.yaml` contents are retained exactly as requested. This
  effort does not delete, rotate, redact, variableize, or migrate those values.
- The product must continue to support both local/personal operation and
  Platform-hosted operation through the same Rust runtime.
- Every production behavior change follows a test-first cycle: failing test,
  minimal implementation, passing focused test, then broader regression tests.

## Current Baseline

The current Gateway already provides:

- Rust `neuro-gateway` runtime with `splitter`, `worker`, and `standalone`
  roles.
- Redis-backed runtime state, optional PostgreSQL-owned management state, YAML
  route loading, and background credential maintenance tasks.
- A five-stage auth/filter/route/send/finalize pipeline.
- OpenAI, Anthropic, Gemini, Bedrock, Cohere, search, media, browser-backed,
  and websocket route families.
- Provider line manifests, Cargo feature gates, browser worker scripts,
  management APIs, health/readiness endpoints, and a Tauri desktop shell.
- Existing offline test and release-candidate gates.

The baseline is locally usable, but the desktop archive, provider evidence
model, and enterprise operations contract are not yet closed as one product.

## Product Architecture

### Runtime layers

The implementation keeps these boundaries explicit:

```text
Desktop / Platform / Loom clients
                |
        Gateway HTTP contract
                |
        Auth + access + quota
                |
          Canonical pipeline
                |
       Routing + protocol adapters
                |
   Direct HTTP / browser-backed upstreams
                |
      Redis / PostgreSQL / object storage
```

The desktop application remains a control shell. Provider routing,
credentials, quota, protocol conversion, and management truth stay in the
headless Gateway.

### Configuration model

The runtime continues to read environment variables and profile-provided
environment values. Productization adds explicit validation and diagnostics;
it does not replace the existing configuration model or remove development
values.

The local product has two explicit modes:

- `standalone`: one process serves the configured port.
- `splitter`: a manager owns worker replacement and traffic cutover.

The desktop profile must record the selected mode and all paths needed by the
selected mode instead of silently relying on build-machine defaults.

## Phase 1: Portable Local Delivery

### Objective

Make a Gateway release directory usable on another Windows machine after the
user supplies the required local dependencies and profile values.

### Components

1. **Gateway-owned release packager**
   - Add a Gateway-owned packaging entrypoint under `tools/`.
   - Build both binaries, stage the runtime support files, write a manifest,
     checksums, build information, and a deterministic package layout.
   - Keep the existing root release entrypoint compatible, but make the
     Gateway-owned packager the source of truth for Gateway contents.
   - Never write outside `release/Gateway/<versionId>` when staging Gateway.

2. **Portable process resolution**
   - Resolve the sidecar and default working directory relative to the desktop
     executable or an explicitly selected profile directory.
   - Remove the runtime dependency on compile-time absolute source paths.
   - Resolve relative `GATEWAY_ROUTES_FILE` paths from the profile working
     directory and expose the resolved path in diagnostics.

3. **Profile and onboarding contract**
   - Add an explicit runtime role field to the desktop profile.
   - Validate Redis URL, port, role, working directory, routes path, and
     required runtime files before save/start.
   - Add dependency probes for Redis, optional PostgreSQL, routes, and the
     sidecar binary.
   - Make startup failures actionable and preserve the existing log tail.

4. **Lifecycle contract**
   - Send the configured management credential when requesting drain.
   - Distinguish graceful drain, timeout, forced termination, and startup
     failure in the process snapshot.
   - Keep `/healthz` and `/readyz` as the authoritative startup probes.

5. **Isolated release E2E**
   - Run a packaged headless binary against an isolated Redis and port.
   - Verify health, readiness, models, deterministic malformed-request errors,
     shutdown, and no leaked process.
   - Run the desktop shell from the staged directory and verify it can locate
     the sidecar without the source checkout.

### Phase 1 completion criteria

- A staged package contains every file required by the selected runtime mode.
- A clean machine path does not depend on the build machine's absolute paths.
- Profile validation catches missing dependencies before sidecar launch.
- Graceful drain works with the configured management credential.
- The isolated packaged E2E passes twice consecutively.
- Existing Gateway unit, Python, Node, and desktop checks remain green.

## Phase 2: Provider Product Surface

### Objective

Make every provider line understandable, testable, and operationally
classifiable without pretending that external quota or account state is a code
failure.

### Components

1. **Canonical line inventory**
   - Derive one machine-readable inventory from manifests, Cargo features,
     protocol profiles, route capabilities, and operator-facing metadata.
   - Record provider, line, protocol, execution mode, supported endpoint
     families, credential requirements, and current evidence state.

2. **Evidence states**
   - Use explicit states: `compiled`, `fixture_passed`, `live_passed`,
     `external_gate`, `credential_missing`, and `known_unsupported`.
   - Require an evidence record for every state transition.
   - Keep provider failures classified by code, credential, quota, region,
     challenge, network, and upstream protocol categories.

3. **Verification matrix**
   - Keep focused line tests isolated through feature and manifest selection.
   - Add representative live canary targets for official API, search,
     reverse-web/browser, and non-OpenAI transport families.
   - Make live calls explicit and opt-in; default CI remains credential-free.

4. **Credential and browser operations**
   - Exercise import, refresh, model health, cooling, lease, and recovery paths
     using the existing credential material contract.
   - Add provider-specific readiness diagnostics for browser-backed lines.
   - Preserve pure HTTP, browser-backed, and program-owned execution modes as
     distinct states.

### Phase 2 completion criteria

- Every manifest-defined line appears in the inventory.
- Every line has a declared capability matrix and evidence state.
- The full no-default-feature/feature matrix is repeatable in CI.
- Representative live canaries produce durable, classified evidence.
- Provider failures do not get mislabeled as Gateway implementation failures.
- No provider route bypasses the canonical auth, routing, send, and finalize
  pipeline.

## Phase 3: Enterprise Runtime

### Objective

Make the Gateway operable as a long-running hosted service with observable
behavior, controlled failure, safe traffic replacement, and recoverable state.

### Components

1. **Observability**
   - Standardize request, provider, credential, model, endpoint, and runtime
     dimensions in structured logs and metrics.
   - Add trace propagation, latency/TTFT/error metrics, quota settlement
     metrics, and browser executor health metrics.
   - Provide operator-readable summaries through Gateway-owned APIs.

2. **Reliability and cutover**
   - Define readiness, draining, worker replacement, timeout, retry, and
     rollback contracts for splitter/worker deployments.
   - Test Redis/PostgreSQL unavailable, slow, and recovering states.
   - Preserve request audit and quota settlement invariants during failures.

3. **Access isolation and controls**
   - Complete tenant/project/access-key isolation checks.
   - Apply rate limits at project, key, model, endpoint, and provider scopes.
   - Keep internal management and browser executor routes fail-closed unless an
     explicit local development override is set.

4. **Operations and recovery**
   - Provide backup/restore verification scripts for Gateway-owned state.
   - Add alert thresholds, incident classification, remediation controls, and
     an operations manual tied to actual endpoints and commands.

### Phase 3 completion criteria

- Operators can explain a failed request from logs, metrics, audit, and route
  evidence without reading source code.
- A worker can drain and be replaced without losing quota settlement or audit
  state.
- Dependency failure behavior is deterministic and tested.
- Tenant, key, quota, and rate-limit boundaries have contract coverage.
- Recovery procedures are executable in an isolated environment.

## Cross-Phase Testing and Delivery

Each implementation batch follows:

1. Add a focused failing test or contract fixture.
2. Run the focused test and confirm the expected failure.
3. Implement the smallest behavior change.
4. Run the focused test to green.
5. Run the affected subsystem suite.
6. Run the full Gateway validation gate before closing the batch.

The final gate includes:

```powershell
python Gateway/tools/validate-gateway-line-manifests.py
python -m unittest discover -s Gateway/tests/python -p "test_*.py" -v
node --test Gateway/scripts/tests/*.test.mjs
cargo test --manifest-path Gateway/Cargo.toml --locked
cd Gateway/apps/desktop; npm run typecheck
```

The release gate additionally runs the isolated packaged E2E and the
Gateway-owned artifact smoke. No real upstream call is made by default; live
canaries remain explicit operator actions.

## Delivery Order

The implementation order is intentionally strict:

```text
Phase 1A package layout and path resolution
        |
Phase 1B profile/onboarding and lifecycle
        |
Phase 1C isolated packaged E2E
        |
Phase 2A inventory and evidence model
        |
Phase 2B line matrix and canary coverage
        |
Phase 2C provider/browser operational closure
        |
Phase 3A observability and failure contracts
        |
Phase 3B access controls and reliability
        |
Phase 3C recovery, operations, and final release gate
```

Provider and enterprise work may use parallel lanes only after the Phase 1
package/runtime contract is stable. Changes touching the canonical pipeline,
runtime state, or release contract remain sequential and require a full gate.

## Explicit Non-Goals

- Rewriting the Gateway in another language.
- Copying Platform, Loom, or Hook implementation code into Gateway.
- Replacing the canonical pipeline with provider-specific shortcut paths.
- Removing or sanitizing development-time sensitive values in this phase.
- Claiming every third-party provider is permanently live regardless of external
  account, quota, challenge, or regional conditions.

## Design Decision

Adopt the three-phase productization architecture above, starting with Phase 1
portable local delivery. A phase is complete only when its completion criteria
are demonstrated by code, tests, package contents, and isolated runtime
evidence; documentation alone is insufficient.
