# Neuro Gateway Integration Contract

Gateway is an independently runnable AI API gateway, similar in role to `newapi`
or `oneapi`.

It supports two deployment/configuration shapes:

- **Personal Gateway**: Gateway runs on a personal computer, owns local provider
  credentials, routes, and relay behavior, and may expose its own management UI.
- **Website/service Gateway**: Gateway runs behind Neuro Platform. Platform owns
  public user accounts, permissions, quotas, billing, and access policy, while
  Gateway remains the provider relay and execution backend.

Loom can also connect to Gateway in local AI-brain/orchestration scenarios. In
that mode, Loom may manage Gateway state through Gateway APIs and hide Gateway's
own UI, while Gateway still owns the provider relay runtime.

Platform, Loom, Hook, account-service pages, and other host implementations
should live outside this project. They integrate with Gateway through the HTTP
APIs, management APIs, route configuration, provider manifests, runtime
environment variables, optional Gateway UI surface, and packaged worker assets
described here.

## Boundary

Gateway owns:

- The `gateway` Rust binary and library in `src/`.
- Public model-serving HTTP and websocket endpoints.
- Internal Gateway management endpoints under `/v1/internal/gateway/**`.
- Gateway runtime health/readiness/metrics endpoints.
- Gateway-owned management UI surface when enabled.
- Provider line manifests in `manifests/`.
- Baseline route configuration in `routes.yaml` and `routes.example.yaml`.
- Browser-backed provider worker scripts in `scripts/`.
- Gateway packaging inputs: `Cargo.toml`, `Cargo.lock`, `Dockerfile`, and
  release workflow assumptions.

Gateway does not own:

- Platform website pages or Platform-specific React/Next.js components.
- Platform account, quota, billing, entitlement, or public access policy.
- Loom AI-brain orchestration internals.
- Hook screenshot, voice, or foreground interaction internals.
- Host-project routing, navigation, session state, or visual design.
- Deployment environments that are specific to a separate website application.

The intended relationship is explicit integration through Gateway-owned
contracts. Host projects may display or edit Gateway state, but they should do so
through Gateway APIs or shared contracts rather than by embedding host-project
implementation code inside Gateway.

## Public serving endpoints

The public serving API is mounted by `src/http/router.rs`. It includes:

- OpenAI-compatible chat/completions/responses/messages/model endpoints:
  `/v1/chat/completions`, `/v1/completions`, `/v1/responses`,
  `/v1/messages`, `/v1/models`.
- New API compatibility aliases under `/v1/new-api/**`.
- Gemini-compatible model actions under `/v1/models/*action` and
  `/v1beta/models/*action`.
- Bedrock Converse routes under `/model/:model/converse` and
  `/model/:model/converse-stream`.
- Cohere chat under `/v2/chat`.
- Embeddings, audio, images, search, fetch, research, music, video, realtime,
  and Gemini Live websocket routes.
- Credential checkout and ensure endpoints:
  `/v1/credentials/checkout`, `/v1/credentials/ensure`.

Consumers should treat `src/http/router.rs` as the source of truth for concrete
route paths and methods.

## Internal management endpoints

Gateway exposes internal management APIs under `/v1/internal/gateway/**`.
These are Gateway-owned APIs intended for Gateway's own UI and for host projects
such as Platform or Loom to consume. Host implementation code should not live
inside Gateway.

Major internal API groups include:

- API access and user credential issuance, verification, rotation, revocation.
- Provider accounts, provider credentials, quotas, folder sync, source profiles,
  model pricing, and model tiering.
- Access catalog, access bundles, access keys, balances, route previews, and
  affinity management.
- Runtime readiness, pressure, drain, request audits, usage aggregates, prompt
  cache summaries, analysis exports, anomaly incidents, remediation runs, and
  credential stock monitoring.
- Browser executor node, slot, lease, and health management.

Internal management routes must be protected by `GATEWAY_MANAGEMENT_TOKEN`.
If the token is missing, Gateway fails closed with
`gateway_management_token_not_configured`. Local development may explicitly
override this by setting `GATEWAY_ALLOW_UNAUTHENTICATED_INTERNAL_ROUTES` to
`1`, `true`, `yes`, or `on`; do not enable that override in shared or
production environments.

## Browser executor endpoints

Gateway owns browser executor coordination and browser-backed provider worker
assets:

- Direct executor API:
  `/v1/internal/browser-executor/health`,
  `/v1/internal/browser-executor/execute`.
- Management API:
  `/v1/internal/gateway/browser-executor/**`.
- Worker scripts in `scripts/`.

Direct executor routes must be protected by
`GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN`. If the token is missing, Gateway fails
closed with `browser_executor_token_not_configured`. Local development may
explicitly override this by setting
`GATEWAY_ALLOW_UNAUTHENTICATED_BROWSER_EXECUTOR` to `1`, `true`, `yes`, or
`on`; do not enable that override in shared or production environments.

## Configuration contract

Gateway runtime configuration is environment-variable based. Required and common
variables include:

- `GATEWAY_REDIS_URL` - required by `Config::from_env`.
- `GATEWAY_DATABASE_URL` or `DATABASE_URL` - optional PostgreSQL connection.
- `GATEWAY_API_KEY` - optional public serving shared secret.
- `GATEWAY_API_KEY_SECRET` - optional project-scoped key signing secret.
- `GATEWAY_MANAGEMENT_TOKEN` - internal management API token.
- `GATEWAY_ALLOW_UNAUTHENTICATED_INTERNAL_ROUTES` - local-development-only
  override for missing internal management token.
- `GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN` - direct browser executor API token.
- `GATEWAY_ALLOW_UNAUTHENTICATED_BROWSER_EXECUTOR` - local-development-only
  override for missing browser executor token.
- `GATEWAY_REQUEST_TIME_BROWSER_POLICY` - browser-backed request policy:
  `local_allowed` uses a remote executor when configured and otherwise allows
  local fallback; `remote_only` fails closed when the remote executor is missing
  or unreachable; `disabled` keeps request-time browser policy disabled where
  supported.
- `GATEWAY_RUNTIME_ROLE` - `splitter`, `worker`, or `standalone`.
- `GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_*` - provider credential folder sync.
- `GATEWAY_PROVIDER_CREDENTIAL_REFRESH_*` - provider credential refresh loop.
- `GATEWAY_CREDENTIAL_STOCK_MONITOR_*` - credential stock monitor loop.

The authoritative list of parsed environment variables is in `src/config.rs`.

## Manifest and route contract

Provider line manifests are Gateway-owned metadata under `manifests/`.

Validate them with:

```powershell
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
```

Route defaults are Gateway-owned and live in:

- `routes.yaml`
- `routes.example.yaml`
- `routes.gemini-canvas-e2e.yaml`
- `routes.gemini-platform-fixture.yaml`

Gateway's own management UI and host applications such as Platform or Loom may
display or edit Gateway state, but they should do so by calling Gateway APIs or
shared Gateway contracts rather than by embedding host-project implementation
code in this project.

## Packaging contract

Gateway packages:

- The release binary.
- `routes.yaml` and `routes.example.yaml`.
- `manifests/`.
- `scripts/`, excluding `node_modules`, `.runtime`, and `output`.
- Gateway-owned `docs/` including provider inventory/evidence and operations
  material.
- Gateway-owned `tools/` used for package smoke, provider evidence, and
  recovery verification.
- Gateway-owned management UI assets, when such assets are present in this
  project.

Gateway does not package Platform/Loom/Hook host-project assets.

## Validation contract

Before treating Gateway as ready, run:

```powershell
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
node --test scripts/tests/*.test.mjs
cargo test --locked --no-run
cargo build --locked --release --bin gateway
```
