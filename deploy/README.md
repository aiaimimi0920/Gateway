# Gateway Docker Deployment

This directory contains the official server-side Docker Compose deployment
stack for Gateway.

## Deployment Variants

| File | Storage mode | Best for |
| --- | --- | --- |
| `docker-compose.yml` | Named volumes | Long-running servers where Docker manages persistent volumes |
| `docker-compose.local.yml` | Local directories | Easier backup, inspection, and migration of `gateway_data/` and `redis_data/` |
| `docker-deploy.sh` | One-click local-directory preparation | Linux/macOS server deployment aligned with the recommended Sub2API Docker workflow |

Both variants deploy the current minimum official runtime stack:

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
