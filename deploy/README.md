# Gateway Docker Deployment

This directory contains the official server-side Docker Compose deployment
stack for Gateway.

## Deployment Variants

| File | Storage mode | Best for |
| --- | --- | --- |
| `docker-compose.yml` | Named volumes | Long-running servers where Docker manages persistent volumes |
| `docker-compose.local.yml` | Local directories | Easier backup, inspection, and migration of `gateway_data/` and `redis_data/` (the database still uses a named volume) |
| `docker-compose.dev.yml` | Source bind mount + build caches | Source checkout only; local edits rebuild the running Gateway automatically |
| `docker-deploy.sh` | Local-directory preparation | Initializes missing files and settings before an operator reviews `.env` |

The production variants deploy the current minimum official runtime stack:

- `gateway`
- `postgres`
- `redis`

## PostgreSQL and the Gateway Schema

PostgreSQL is required for the official Compose stack. The gateway keeps its request audits,
usage aggregates and per-credential model states there, so without a database
the console's live concurrency, cost and success-rate panels answer
`503 PostgreSQL 尚未配置` and every affected number renders as `—`.

The production Compose files default to the bundled PostgreSQL service and require
it to be healthy before starting Gateway. Changing `GATEWAY_DATABASE_URL` does not
remove that dependency. To run without the bundled service, use a reviewed custom
Compose configuration and provision the external schema first.

The Rust runtime does not create tables; it expects the schema to already
exist. `postgres/initdb/001-gateway-schema.sql` supplies it and is applied by
the official `postgres` image the first time a data directory is initialised.
That file is a `pg_dump --schema-only` of the `gateway_*` tables produced by the
33 migrations in the Platform repository
(`Platform/packages/ai-gateway-domain/migrations/`, run by
`src/scripts/migrate.ts`), plus the matching `gateway_schema_migrations` ledger
rows so a fresh database reports the same migration state as a migrated one.
Platform remains the owner of the schema: when a migration is added there,
re-dump this file rather than hand-editing it.

Two standalone-only files follow the baseline. The additive extensions after them
are listed below.

- `postgres/initdb/002-gateway-bootstrap.sql` seeds one tenant, the project named
  by `GATEWAY_DEFAULT_PROJECT_ID` (default `platform-default-project`), that
  project's default route policy, and the two `gateway_api_keys` identity rows
  the gateway attributes shared-secret (`gateway-key`) and dev-mode (`dev-mode`)
  traffic to. Platform inserts equivalent rows itself, so a deployment sharing
  Platform's database does not need this file. Without the project rows every
  relay request fails with `404 AI gateway project 不存在`; without the identity
  rows every request audit fails with `引用的资源不存在` and the console's
  concurrency and success-rate panels stay empty. The route-policy config is a
  capture of the Rust defaults, so re-capture it if those change.
- `postgres/initdb/003-gateway-standalone-constraints.sql` drops
  `gateway_request_audits_provider_account_id_fkey`. A standalone gateway keeps
  its providers in the route document (`GATEWAY_ROUTES_FILE`) rather than in
  `gateway_provider_accounts`, so the provider id recorded on each audit has no
  row to reference and every audit finalise would fail — leaving rows at
  `status='running'` for ever and inflating `/v1/internal/gateway/pressure`. A
  deployment sharing Platform's database writes real `gateway_provider_accounts`
  ids and should keep the foreign key.

New volumes run `postgres/initdb/` in filename order. Existing volumes do not rerun
initdb: back up the database, review schema and grants with the owner, and apply
only the required migrations before the binary upgrade. Use the Platform migration
process for a shared database; do not apply standalone-only seed or constraint
changes there. Do not remove the volume or reapply the baseline dump to upgrade.

### Additive extensions for existing databases

`postgres/initdb/004-gateway-key-cash.sql` adds cash billing storage; see the
[cash billing contract](../docs/key-cash-billing.md).

The additive Accio DB refresh extension is
`postgres/initdb/005-gateway-refresh-authority.sql`. It adds authorization
incarnations/generations and permanent grant evidence; it does not enable the
Accio background scheduler by itself. Existing databases are not upgraded by
initdb. Platform-shared deployments require schema-owner migration/ledger review;
standalone upgrades also require backup and transactional application. Do not
hand-edit `001`, drop grant history, or treat this extension as runtime auto-DDL.
See [Accio DB refresh contract](../docs/provider-accio-db-refresh.md) for the
rollback, secret-access, table-lock and object-publication boundaries.

Server-mode Luma MCP console creation uses the separate
`postgres/initdb/006-gateway-luma-mcp-creation.sql` journal. New standalone
volumes initialize it from `initdb`; back up existing deployments and apply the
additive DDL transactionally after schema-owner review (including Platform-shared
installations). Neither the runtime nor the existing `001` dump creates this
extension on an already initialized deployment. If the table or
`GATEWAY_API_KEY_SECRET` is unavailable, only this Server creation flow rejects
work before sending to Luma. Ensure participating replicas use the same stable
signing secret and authorized management token. A new token, or a new signing
secret, cannot reopen existing tasks; no automatic owner migration is provided.
For binary rollback, leave the additive table in place and retain the data and
signing configuration. Do not discard a volume to install or roll back this
extension. Journal payloads can contain signed media URLs and upstream review or
session content: restrict table, backup and log access as for sensitive console
state. These rows are not public request or cash-billing audits.

Command Code Server model probes require the additive
`postgres/initdb/007-gateway-internal-model-probe-audits.sql` extension. Apply it
before a binary upgrade through the deployment owner's reviewed migration;
startup does not perform DDL. Without it, Command Code Server generation probes
reject before sending; public relay and Local probes are unchanged. Keep the
extra table on rollback. See the [Server probe audit contract](../docs/providers/commandcode-server-probe-audit-contract.md).

SAP Server 模型探测另需在 `007` 之后应用
`postgres/initdb/012-gateway-sap-probe-evidence.sql`。此为可重复执行的加法迁移，
由部署 schema owner 审核并应用；运行时不执行 DDL，不自动迁移线上数据库。
新列 `sap_evidence` 仅记录 SAP 探测闭合后的发送次数、实际 HTTP 状态及固定脱敏错误字段；
OAuth 状态与 inference 状态分开。旧行和未完成的 `running` 行保持 SQL `NULL`，表示未知，
不可解读为零发送或退款依据。缺少扩展时 SAP 探测在发送前拒绝，其他产品沿用原 SQL。
回退二进制时保留列和数据；混合版本的旧写者仍可忽略此可空列。Platform 管理的库必须由
该库 owner 一并确认前置 `007`，不能只对缺少内部探测表的基线执行 `012`。

The SAP Responses receipt schema (`postgres/initdb/008-gateway-sap-response-receipts.sql`)
is additive; it does not enable public stored Responses by itself. On existing
PostgreSQL deployments, review the DDL and grants with the schema owner and
apply it transactionally before enabling a future lifecycle rollout. Runtime
startup does not apply PostgreSQL DDL. Installations that applied the
receipt-only revision must reapply `008` to add the balance-hold table and its
index. Keep both tables on rollback. They store routing references and accounting
evidence, not SAP Service Keys or inference content; restrict access to the
runtime and authorized operators. Do not roll back to a binary that ignores the
holds while a funded response is unsettled: it may rotate or remove the key
before the settlement can adjust its balance.

Apply `postgres/initdb/009-gateway-sap-response-mutations.sql` after `008` for
the SAP lifecycle mutation journal. Preserve it on rollback. An unconfirmed
claim does not prove the upstream operation was rejected and must not be
cleared simply to retry the request.

Apply `postgres/initdb/010-gateway-sap-response-route-pins.sql` after `009`.
Preserve the route pins on rollback. Existing receipts are not backfilled from
current routing data; an unpinned receipt is not eligible for upstream routing.

Apply `postgres/initdb/011-gateway-sap-response-recovery.sql` after `010` before
deploying the local-ledger recovery worker. Review the additive table/index and
runtime grants with the schema owner; PostgreSQL startup still performs no DDL.
Preserve unsent proofs and open holds on rollback. Never backfill proofs from a
`submitting` phase: that phase cannot distinguish an unsent request from an
unknown delivery. The worker only refunds matching durable proofs or settles
already recorded terminal usage; it does not call SAP or release unconfirmed
cancel/delete claims. A missing migration is reported as recovery unavailable,
not as an empty queue. Existing live-owner refunds can still use direct unsent
knowledge, but a failed proof write cannot authorize future automatic refunds.

## Quick Start

For existing PostgreSQL data, review the [schema and migration instructions](#postgresql-and-the-gateway-schema) first.

### Method 1: Prepare a local-directory stack

```bash
cd deploy
chmod +x docker-deploy.sh
./docker-deploy.sh
# Review and edit .env before starting.
docker compose -f docker-compose.local.yml up -d
docker compose -f docker-compose.local.yml logs -f gateway
```

The helper prepares `.env`, `gateway_data/`, and `redis_data/` and fills missing
secrets. Its fallback management token is a fixed development value, not a
generated secret. It does not override the template's explicit
`GATEWAY_CONSOLE_REMOTE_ACCESS=false`. For local use, set
`GATEWAY_BIND_HOST=127.0.0.1` before starting; the Compose template otherwise
publishes to `0.0.0.0`. Replace the development management token and database
password before any shared deployment. Enable remote console access only if
intended and keep it behind your deployment's access controls.

### Method 2: Manual Preparation

```bash
cd deploy
test -f .env || cp .env.example .env
mkdir -p gateway_data redis_data
# Set bind address and deployment secrets in .env before starting.
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
# Prepare .env as in Quick Start; do not overwrite an existing configuration.
docker compose up -d
docker compose logs -f gateway
```

Useful commands:

```bash
# Stop the stack
docker compose down
```

Do not add `-v` for a restart or upgrade: it removes named volumes, including
PostgreSQL data, but does not remove host bind mounts.

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

- `GATEWAY_MANAGEMENT_TOKEN=11011101`
- `GATEWAY_CONSOLE_REMOTE_ACCESS=true`

After `-Action up`, open:

- `http://127.0.0.1:4200/ui/`

and sign in with the management token `11011101`. If you are preparing a public
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
| `GATEWAY_DATABASE_URL` | `postgres://gateway:gateway@postgres:5432/gateway` | Internal PostgreSQL URL used by the gateway container |
| `GATEWAY_POSTGRES_USER` | `gateway` | Bundled PostgreSQL role |
| `GATEWAY_POSTGRES_PASSWORD` | `gateway` | Bundled PostgreSQL password |
| `GATEWAY_POSTGRES_DB` | `gateway` | Bundled PostgreSQL database name |
| `GATEWAY_RUNTIME_ROLE` | `standalone` | Single-instance service mode used by the official compose stack |
| `GATEWAY_ROUTES_FILE` | `/data/routes.yaml` | Persistent route file path inside the container |
| `GATEWAY_STATE_DIR` | `/data/state` | Persistent Gateway state directory |

Optional variables:

- `GATEWAY_API_KEY`
- `GATEWAY_API_KEY_SECRET`
- `GATEWAY_MANAGEMENT_TOKEN`
- `GATEWAY_CONSOLE_REMOTE_ACCESS`

The bundled PostgreSQL port is never published to the host, so the default
`gateway`/`gateway` credentials stay reachable only from the compose network.
Change them anyway if the host is shared, and change both
`GATEWAY_POSTGRES_PASSWORD` and `GATEWAY_DATABASE_URL` together.

## Health Checks

The compose stack waits for PostgreSQL and Redis to report healthy, then checks
Gateway via:

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
