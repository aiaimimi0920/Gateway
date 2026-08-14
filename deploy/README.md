# Gateway Docker Deployment

This directory contains the official server-side Docker Compose deployment
stack for Gateway.

## Deployment Variants

| File | Storage mode | Best for |
| --- | --- | --- |
| `docker-compose.yml` | Named volumes | Long-running servers where Docker manages persistent volumes |
| `docker-compose.local.yml` | Local directories | Easier backup, inspection, and migration of `gateway_data/` and `redis_data/` |
| `docker-compose.dev.yml` | Source bind mount + build caches | Source checkout only; local edits rebuild the running Gateway automatically |
| `docker-deploy.sh` | One-click local-directory preparation | Linux/macOS server deployment aligned with the recommended Sub2API Docker workflow |

The production variants deploy the current minimum official runtime stack:

- `gateway`
- `redis`

PostgreSQL remains optional in the current Rust runtime. If you need DB-backed
features later, point `GATEWAY_DATABASE_URL` at an external PostgreSQL
instance.

## Quick Start

### Method 1: One-Click Preparation (Recommended)

```bash
cd deploy
chmod +x docker-deploy.sh
./docker-deploy.sh
docker compose -f docker-compose.local.yml up -d
docker compose -f docker-compose.local.yml logs -f gateway
```

The script:

- creates `.env` from `.env.example` when needed
- generates `GATEWAY_API_KEY`, `GATEWAY_API_KEY_SECRET`, and `GATEWAY_MANAGEMENT_TOKEN`
- enables remote console access for the prepared stack
- creates `gateway_data/` and `redis_data/`

### Method 2: Manual Preparation

```bash
cd deploy
cp .env.example .env
mkdir -p gateway_data redis_data
docker compose -f docker-compose.local.yml up -d
docker compose -f docker-compose.local.yml logs -f gateway
```

Gateway is then available at:

- `http://localhost:4200`

## First Boot Behavior

The container image defaults to:

- `GATEWAY_RUNTIME_ROLE=standalone`
- `GATEWAY_ROUTES_FILE=/data/routes.yaml`
- `GATEWAY_STATE_DIR=/data/state`

On the first boot, the image entrypoint automatically copies the bundled
`/app/routes.example.yaml` into the configured `routes.yaml` location if the
file does not exist yet. That means:

- named-volume deployments get a persistent `/data/routes.yaml`
- local-directory deployments get `deploy/gateway_data/routes.yaml`

Edit that file after the first boot to replace the example route configuration
with your real provider routes.

## Recommended Local-Directory Workflow

```bash
cd deploy
cp .env.example .env
mkdir -p gateway_data redis_data
docker compose -f docker-compose.local.yml up -d
```

Useful commands:

```bash
# Follow runtime logs
docker compose -f docker-compose.local.yml logs -f gateway

# Stop the stack
docker compose -f docker-compose.local.yml down

# Restart after editing gateway_data/routes.yaml
docker compose -f docker-compose.local.yml restart gateway
```

## Source-Mounted Development Workflow

This workflow is available only in a full Gateway source checkout.
`docker-compose.dev.yml` and `docker-dev-entrypoint.sh` are intentionally not
included in portable or Docker deployment release bundles. For fast local
iteration against the repository source tree:

```bash
cd deploy
cp .env.example .env
mkdir -p gateway_data redis_data
docker compose -f docker-compose.dev.yml up -d
docker compose -f docker-compose.dev.yml logs -f gateway
```

What this development stack does:

- bind-mounts the repository root into `/workspace`
- keeps Cargo, npm, target, desktop `node_modules`, and browser-worker
  `node_modules` caches in separate Docker volumes
- requires Node.js `>=22.22.0` at development-image build time and checks the
  runtime again whenever the development entrypoint starts
- installs each Node dependency tree when its `package-lock.json` hash changes,
  then audits both production dependency trees before starting either watcher
- defaults to `GATEWAY_DEV_CARGO_BUILD_JOBS=4` and
  `GATEWAY_DEV_CARGO_INCREMENTAL=1` so the dev stack rebuilds faster than the
  deterministic release path; override those two env values only if your
  workstation needs a different CPU/latency tradeoff
- runs `npm run build:web -- --watch`
- runs `cargo watch --poll -x 'run --locked --bin gateway'`
- gives the first cold boot a longer health-check grace period because the
  initial Rust compile can legitimately take several minutes

That means edits under the local `Gateway/` repository are picked up inside the
container and the 4200 service is rebuilt/restarted automatically.

Notes for the first cold boot:

- `docker compose ps` can stay in `health: starting` for a few minutes
- once the first compile finishes, later source changes usually rebuild much
  faster inside the same running container

## Named-Volume Workflow

```bash
cd deploy
cp .env.example .env
docker compose up -d
docker compose logs -f gateway
```

Useful commands:

```bash
# Stop the stack
docker compose down

# Remove all persistent data
docker compose down -v
```

## Repository Helper Script

From the repository root you can drive the same deployment stack with:

```powershell
.\tools\deploy-gateway-docker.ps1 -Action up -Mode local
.\tools\deploy-gateway-docker.ps1 -Action logs -Mode local -Follow
.\tools\deploy-gateway-docker.ps1 -Action down -Mode local
```

For source-mounted development mode in a full source checkout (not a release
bundle):

```powershell
.\tools\deploy-gateway-docker.ps1 -Action up -Mode dev -ComposeProjectName gatewaydev
.\tools\deploy-gateway-docker.ps1 -Action logs -Mode dev -ComposeProjectName gatewaydev -Follow
.\tools\deploy-gateway-docker.ps1 -Action down -Mode dev -ComposeProjectName gatewaydev
```

For fresh local users, that PowerShell helper defaults to
`GATEWAY_BIND_HOST=127.0.0.1`. When the stack is loopback-bound, it also seeds
missing console login values in `deploy/.env` without overwriting explicit
user choices:

- `GATEWAY_MANAGEMENT_TOKEN=123456`
- `GATEWAY_CONSOLE_REMOTE_ACCESS=true`

After `-Action up`, open:

- `http://127.0.0.1:4200/ui/`

and sign in with the management token `123456`. If you are preparing a public
server instead, change `GATEWAY_BIND_HOST` to `0.0.0.0` and replace the
default management token before exposing the service.

For a disposable end-to-end validation using a locally built image:

```powershell
.\tools\verify-gateway-docker-stack.ps1 -BuildImage
```

The official wrapper generates a unique `GATEWAY_AUDIT_NONCE` for the Docker
image build, which forces both in-image production dependency audits to run
without requiring Node.js or host `node_modules`. To perform the same
dependency-fresh image build manually:

```powershell
$auditNonce = [guid]::NewGuid().ToString("N")
docker build --build-arg "GATEWAY_AUDIT_NONCE=$auditNonce" -t gateway:local .
```

A raw `docker build -t gateway:local .` remains useful for cached development
iterations, but it does not prove that the dependency audit was refreshed.

## Environment Variables

Copy `.env.example` to `.env` and change what you need.

Most important variables:

| Variable | Default | Description |
| --- | --- | --- |
| `IMAGE_TAG` | `latest` | Gateway image tag from `ghcr.io/aiaimimi0920/gateway` |
| `GATEWAY_BIND_HOST` | `0.0.0.0` | Host bind address for the published API port |
| `GATEWAY_PORT` | `4200` | Published host port |
| `RUST_LOG` | `info` | Gateway runtime log level |
| `GATEWAY_REDIS_URL` | `redis://redis:6379/0` | Internal Redis URL used by the gateway container |
| `GATEWAY_RUNTIME_ROLE` | `standalone` | Single-instance service mode used by the official compose stack |
| `GATEWAY_ROUTES_FILE` | `/data/routes.yaml` | Persistent route file path inside the container |
| `GATEWAY_STATE_DIR` | `/data/state` | Persistent Gateway state directory |

Optional variables:

- `GATEWAY_API_KEY`
- `GATEWAY_API_KEY_SECRET`
- `GATEWAY_MANAGEMENT_TOKEN`
- `GATEWAY_CONSOLE_REMOTE_ACCESS`
- `GATEWAY_DATABASE_URL`

## Health Checks

The compose stack waits for Redis and checks Gateway via:

- `/healthz`

If the service starts but routing is still incomplete, inspect:

- `docker compose logs gateway`
- `gateway_data/routes.yaml`
- `http://localhost:4200/readyz`

## Notes

- This stack is intentionally **server-first**. It is the recommended product
  deployment path for Gateway.
- The Windows desktop shell remains useful for local management, but it is not
  the canonical production deployment shape.
# Credential pool automation

The account ledger exposes a target size plus automatic refill/prune controls
for every route provider. These controls are backed by the Gateway worker; they
are not UI-only flags.

Automation drivers are loaded from a backend-owned JSON registry. Start from
`credential-pool-drivers.example.json`, store the real registry outside the
public route document, and set:

```dotenv
GATEWAY_CREDENTIAL_POOL_AUTOMATION_DRIVER_CONFIG=/data/credential-pool-drivers.json
GATEWAY_CREDENTIAL_POOL_AUTOMATION_SCRIPT_ROOT=/data/credential-pool-scripts
```

Route providers can optionally select an allowlisted driver with
`credential_automation_driver_id`. If that field is absent, Gateway uses the
single registry driver whose `provider_ids` contains the provider ID. Multiple
matches require an explicit driver ID.

Both script stdin and HTTP POST use a fixed JSON reconcile request. A driver
must return:

```json
{
  "credentials": [
    {
      "id": "provider-account-2",
      "account_name": "Account 2",
      "api_key": "secret returned by the trusted driver",
      "enabled": true
    }
  ],
  "prune": [
    {
      "credential_id": "provider-account-old",
      "classification": "permanent_auth_failure"
    }
  ],
  "message": "reconciled"
}
```

Allowed prune classifications are only `permanent_auth_failure`,
`account_deleted`, and `permanent_upstream_rejection`. Quota exhaustion, rate
limits, browser challenges, and transient network errors are rejected by the
response schema and cannot trigger deletion. Script drivers are restricted to
relative `.ps1`, `.js`/`.mjs`/`.cjs`, `.py`, or `.exe` files inside the
allowlisted script root. HTTP drivers require HTTPS, except for loopback test
endpoints, and may reference a bearer token only through `secret_env`.

## Credential refill task framework

Gateway exposes one reliable task state machine for all three refill flows:

| Trigger | Meaning |
| --- | --- |
| `notification` | Gateway detects a pool deficit and publishes a task automatically. This applies when `auto_refill_enabled=true` and the Provider has no direct trusted driver. |
| `inquiry` | A refill worker asks for work. If no matching pending task exists, Gateway evaluates matching Provider deficits, creates a task, and claims it for that worker. |
| `user_requested` | An operator explicitly requests a refill from the Gateway console or management API. An explicit request defaults to `max(deficit, 1)`, so it can request one account even when the measured deficit is zero. |

All three triggers use these task states:

```text
pending -> claimed -> succeeded
                   -> failed
```

The durable notification stream is:

```text
gw:credential-pool:refill:requests
```

External refill programs should create and use a stable Redis Stream consumer
group. A stream entry is only a wake-up signal: the worker must still call the
Gateway claim endpoint before doing any work. Claiming provides a lease and
prevents multiple workers from refilling the same Provider concurrently. A
long-running worker must renew its lease before it expires.

Every endpoint below requires normal Gateway management authentication. The
`claimToken` returned by the claim endpoint proves ownership of one lease; it
does **not** replace the management token.

```text
GET  /v1/internal/gateway/credential-pool-refill
GET  /v1/internal/gateway/credential-pool-refill/tasks?providerId=...&state=...&limit=...
POST /v1/internal/gateway/credential-pool-refill/providers/:providerId/request
POST /v1/internal/gateway/credential-pool-refill/tasks/claim
POST /v1/internal/gateway/credential-pool-refill/tasks/:taskId/renew
POST /v1/internal/gateway/credential-pool-refill/tasks/:taskId/complete
POST /v1/internal/gateway/credential-pool-refill/tasks/:taskId/fail
```

Example inquiry/claim request:

```json
{
  "workerId": "refill-worker-1",
  "providerIds": ["suno"],
  "leaseSeconds": 300
}
```

An operator request accepts an optional `requestedCount` and caller-provided
`idempotencyKey`. Gateway scopes idempotency to the Provider and permits only
one outstanding task per Provider:

```json
{
  "requestedCount": 2,
  "idempotencyKey": "operator-request-2026-08-14"
}
```

Workers can complete a claimed task with one of three delivery modes:

1. `folder_sync`: the worker writes material below the configured credential
   folder and asks Gateway to run the existing import. `relativePaths` are
   validated as safe relative paths; the current importer still scans its
   configured root rather than limiting import to only those hints.
2. `gateway_pull`: the worker supplies an opaque `artifactReference`. Gateway
   passes it only to the Provider's backend-allowlisted `collect_refill`
   script/HTTP driver and commits the returned credentials itself. Route
   configuration cannot provide an arbitrary pull URL.
3. `direct_callback`: the worker sends credential drafts in the authenticated
   completion request. Gateway appends new credential IDs through the normal
   revisioned route-config commit path. Retrying an already-added credential ID
   is idempotent.

The Redis Stream event is intentionally secret-free and contains only
`taskId`, `providerId`, `trigger`, `requestedCount`, `routeRevision`, and
`createdAt`. Never put API keys, cookies, OAuth tokens, authorization headers,
claim tokens, cloud URLs, or credential payloads in the Stream, task failure
reason, or ordinary task metadata. Secret-bearing credentials may enter
Gateway only through the authenticated `direct_callback` body, the configured
credential folder, or a backend-trusted pull driver.

Configuration defaults:

```dotenv
GATEWAY_CREDENTIAL_REFILL_QUEUE_ENABLED=true
GATEWAY_CREDENTIAL_REFILL_NOTIFICATION_INTERVAL_SECS=30
GATEWAY_CREDENTIAL_REFILL_TASK_TTL_SECS=604800
GATEWAY_CREDENTIAL_REFILL_DEFAULT_LEASE_SECS=300
GATEWAY_CREDENTIAL_REFILL_MAX_LEASE_SECS=3600
GATEWAY_CREDENTIAL_REFILL_STREAM_MAX_LEN=10000
```
