# Neuro Gateway

[![CI](https://github.com/aiaimimi0920/Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/ci.yml)
[![Build Windows](https://github.com/aiaimimi0920/Gateway/actions/workflows/build-windows.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/build-windows.yml)
[![Docker](https://github.com/aiaimimi0920/Gateway/actions/workflows/docker.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/docker.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**English** | [简体中文](README.zh-CN.md)

Neuro Gateway is an independently runnable Rust AI gateway and provider relay.
It exposes OpenAI-compatible and provider-specific APIs, owns provider line
manifests and routing, and ships a local Windows desktop launcher alongside
the headless service.

Repository: <https://github.com/aiaimimi0920/Gateway>

Gateway can run by itself on a developer workstation or be managed by Neuro
Platform and Loom through its API boundary. Platform remains responsible for
accounts, permissions, quota, billing, and public web policy. Loom remains
responsible for orchestration and local AI-brain workflows. Hook remains the
foreground capture and integration client.

## What Gateway Owns

- Rust gateway runtime and HTTP/WebSocket APIs (`gateway`).
- Provider routing, credentials loaded by the runtime, and relay behavior.
- Provider line manifests under `manifests/` and route configuration.
- Browser-backed provider workers under `scripts/`.
- Gateway management endpoints and the local desktop shell
  (`gateway-ui`).
- Release packaging, integrity manifests, and operational evidence tools.

Platform, Loom, and Hook implementation code is intentionally not copied into
this repository. The ownership and integration rules are documented in
[`INTEGRATION_CONTRACT.md`](INTEGRATION_CONTRACT.md).

## Repository Layout

```text
./
├── apps/desktop/       # TypeScript frontend and Tauri launcher
├── deploy/             # Official Docker Compose deployment stacks
├── manifests/          # Provider line manifests and JSON schema
├── scripts/            # Browser workers and worker tests
├── src/                # Rust library and gateway binary
├── tests/              # Rust and Python contract tests
├── tools/              # Validation, build, package, and smoke tooling
├── routes.yaml         # Current development route configuration
└── routes.example.yaml # Portable configuration template
```

## Requirements

- Windows 10/11 or a current Linux distribution.
- Rust `1.91.1` or the version pinned in `rust-toolchain.toml`.
- Node.js `22` and npm for browser workers and the desktop frontend.
- Python `3.11+` for repository validators and contract tests.
- PowerShell 5.1+ for the Windows build and smoke scripts.
- Docker Engine or Docker Desktop for container builds.

## Clone And Build

Gateway is a standalone repository. Commands below are run from the Gateway
repository root, not from a Neuro monorepo checkout:

```powershell
git clone https://github.com/aiaimimi0920/Gateway.git
Set-Location Gateway

cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked
```

Gateway now carries a local-default Cargo throttle under
[`./.cargo/config.toml`](.cargo/config.toml) with `build.jobs = 1` so routine
Rust builds do not saturate a workstation by default. The official development
container and release builder also force `CARGO_BUILD_JOBS=1` and
`CARGO_INCREMENTAL=0`; inherited shell, `.env`, or CI values cannot raise the
parallelism of those supported build paths.

Validate manifests and Python contracts:

```powershell
python -m pip install --disable-pip-version-check -r tests/python/requirements.txt
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
```

Install and test browser workers:

```powershell
npm ci --prefix scripts
node --test scripts/tests/*.test.mjs
```

Build and check the desktop shell:

```powershell
npm ci --prefix apps/desktop
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop run build
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml
```

## Runtime Configuration

The service reads runtime settings from environment variables. `GATEWAY_REDIS_URL`
is required by `Config::from_env`; `GATEWAY_DATABASE_URL` or `DATABASE_URL` is
optional. Copy `.env.example` to a local `.env` and fill in the values for a
development run. `.env` and other local environment files remain ignored.

The current development snapshot intentionally retains its versioned route
configuration and other local development state. Do not replace `routes.yaml`
with an empty file when testing the provider matrix.

Run the headless service directly after configuring the environment:

```powershell
cargo run --locked --bin gateway
```

For the desktop shell during development:

```powershell
Push-Location apps/desktop
npm run tauri dev
Pop-Location
```

## Portable Windows Release

Each version directory is immutable and is written to the repository-local
`release/Gateway/<versionId>` directory by default. A package contains the two
Windows executables, the selected route files, `.env.example`, build
provenance, manifests, browser workers, documentation, the official `deploy/`
directory, tools, `manifest.json`, and `checksums.sha256`.

Build and package a candidate from the repository root:

```powershell
$id = "gateway-product-" + (Get-Date -Format "yyyyMMdd-HHmmss")
.\tools\build-gateway-release.ps1
.\tools\package-gateway-release.ps1 -VersionId $id -SkipBuild
```

`build-gateway-release.ps1` now performs `npm run build:web` explicitly before
the headless Cargo release build, then exports `GATEWAY_PREBUILT_WEB_UI=1` for
  that Cargo step so the root `build.rs` reuses prebuilt web assets instead of
  launching another implicit browser-console build. The script enforces one
  Cargo job with incremental compilation disabled for both the headless and
  desktop builds.

The package command can build for you when `-SkipBuild` is omitted. A skipped
build is accepted only when `target/release/gateway-build-provenance.json`
matches both executables and the current source tree. Use a new version id for
every package; an existing directory is never overwritten.

The Neuro workspace policy may keep release artifacts outside this checkout.
Pass an explicit release root when that is required:

```powershell
$neuroReleaseRoot = "C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway"
.\tools\package-gateway-release.ps1 `
  -VersionId $id -SkipBuild `
  -ReleaseRoot $neuroReleaseRoot -AllowCustomReleaseRoot
```

Create a deterministic archive and checksum for distribution:

```powershell
.\tools\compress-gateway-release.ps1 `
  -ReleaseDir ".\release\Gateway\$id" `
  -OutputDir ".\release\Gateway\packages" `
  -VersionId $id
```

The archive is named `Gateway-<versionId>-windows-x64.zip`; the adjacent
`.sha256` file contains the SHA-256 digest and archive filename.

Verify the packaged artifacts without modifying the package directory:

```powershell
.\tools\smoke-gateway-packaged-runtime.ps1 `
  -ReleaseDir ".\release\Gateway\$id" -IntegrityOnly
.\tools\smoke-gateway-ui-release.ps1 `
  -ReleaseDir ".\release\Gateway\$id"
```

Browser workers are deliberately shipped without `node_modules`. Install them
after extracting a package:

```powershell
npm ci --prefix ".\release\Gateway\$id\scripts"
node --test ".\release\Gateway\$id\scripts\tests\*.test.mjs"
```

The full evidence matrix is a **source repository only** tool because it needs
Cargo sources and repository verification helpers. The portable package
includes `scripts/invoke-gateway-live-provider-canary.ps1`; run it without
`-AllowLiveProviderCalls` for the safe dry-run contract, or provide the live
inputs documented in [`docs/provider-evidence.md`](docs/provider-evidence.md).
Keep evidence under `target/release-evidence/<versionId>` or another external
evidence root, never inside an immutable package.

## Docker

For direct server deployment, use the official Compose stack under
[`deploy/`](deploy/README.md). The recommended local-directory variant keeps
route and Redis data on the host filesystem for easy backup and migration:

```bash
cd deploy
chmod +x docker-deploy.sh
./docker-deploy.sh
docker compose -f docker-compose.local.yml up -d
```

That one-click preparation flow is the closest Gateway equivalent to the
recommended Sub2API Docker Compose deployment pattern: prepare `.env`, generate
the required secrets, create local data directories, then start the
local-directory compose stack.

```bash
cd deploy
cp .env.example .env
mkdir -p gateway_data redis_data
docker compose -f docker-compose.local.yml up -d
docker compose -f docker-compose.local.yml logs -f gateway
```

The image boots in `standalone` mode by default and auto-seeds the persistent
route file from `routes.example.yaml` on first start.

Named-volume deployment is also provided:

```bash
cd deploy
cp .env.example .env
docker compose up -d
docker compose logs -f gateway
```

Raw `docker build` / `docker run` remains available for development-only image
testing:

```powershell
docker build -t gateway:local .
docker run --rm -p 4200:4200 --env-file .env gateway:local
```

The image contains the release binary, route files, provider manifests, browser
workers, and a container entrypoint that initializes `/data/routes.yaml` when a
persistent route file is missing. Runtime secrets are supplied through
environment variables or an env file; they are not read from `.env` during the
image build.

For one-command Docker operations from the repository root, use:

```powershell
.\tools\deploy-gateway-docker.ps1 -Action up -Mode local
.\tools\deploy-gateway-docker.ps1 -Action logs -Mode local -Follow
.\tools\deploy-gateway-docker.ps1 -Action down -Mode local
```

The root PowerShell helper is optimized for a fresh local workstation flow. On
the first `-Action up`, it writes `GATEWAY_BIND_HOST=127.0.0.1` by default and,
for loopback-bound stacks, seeds these missing console values without
overwriting existing user settings:

- `GATEWAY_MANAGEMENT_TOKEN=123456`
- `GATEWAY_CONSOLE_REMOTE_ACCESS=true`

After the stack is up, open:

- `http://127.0.0.1:4200/ui/`

and sign in with the management token `123456`. If you want a server-style
public bind instead, pass `-BindHost 0.0.0.0` and set your own management
token before exposing the port externally.

The browser console now includes two account-pool management workspaces on top
of the existing route-config and revision flows:

- `Accounts` treats `provider.credentials[]` inside `routes.yaml` / the active
  route-config document as reusable account units and groups them by provider /
  service provider;
- `Groups` lets you organize those accounts into logical pools and persist
  `billing_multiplier` metadata for later Platform-side billing integration.

This metadata lives in the same route-config document under the top-level
`account_groups` field, so it works in the current standalone / Redis-managed
product shape without requiring PostgreSQL.

Runtime routing stays backward-compatible by default:

- requests **without** an account-group selector keep the existing routing behavior;
- trusted internal callers can set `x-neuro-account-group` (or
  `x-account-group-id`) together with `x-internal-api-key` to force YAML/static
  routing to stay inside one configured account pool.

For Platform-side billing and account-pool orchestration, the gateway now also
exposes a management-only summary endpoint:

```bash
curl http://127.0.0.1:4200/v1/internal/gateway/account-groups \
  -H "x-internal-api-key: 123456"
```

The response includes:

- `accountGroups[]`: effective `billingMultiplier`, `memberCount`, enabled
  state, and member account IDs;
- `accounts[]`: reverse mapping from account ID to provider and group IDs;
- `providers[]`: provider-to-account inventory summary.

The `Test` action in the Accounts workspace uses a standalone credential
connectivity probe:

```text
POST /v1/internal/gateway/console/credentials/{credential_id}/probe
```

The endpoint requires an authenticated management session and the exact active
Secret Grant returned by the confirmation endpoint. Clients must send that
value in the `x-secret-grant` header; a missing, expired, unknown, or
context-mismatched grant is rejected with HTTP `403` and
`console_secret_access_required`. Grants are bound to the management-token
fingerprint, request origin, and client IP. The probe uses the compiled
credential from the active route snapshot, so it does not require PostgreSQL;
disabled credentials never make a network request. The only response statuses
are:

- `passed`: a supported HTTP credential probe completed successfully;
- `failed`: the upstream request failed and the returned message was sanitized;
- `unsupported`: the adapter is fixed-model, browser-backed, stateful, or
  otherwise has no safe side-effect-free probe.

The response shape is `{ "result": { "credentialId", "providerId", "status",
"message", "checkedAt" } }`; API keys, cookies, tokens, and upstream bodies
are never returned.

To run an end-to-end local Docker verification against a locally built image:

```powershell
.\tools\verify-gateway-docker-stack.ps1 -BuildImage
```

## GitHub Automation

- `ci.yml` validates Windows and Linux builds, Python contracts, Node workers,
  Rust targets, the desktop frontend, and the Tauri wrapper.
- `docker.yml` locally verifies the official Compose deployment stack before any
  optional GHCR publish.
- `release-tag.yml` also exports `Gateway-Vx.y.z-docker-deploy.zip` plus a
  SHA-256 checksum alongside the Windows release ZIP.
- `build-windows.yml` creates and uploads an immutable Windows candidate under
  `release/Gateway`.
- `docker.yml` builds on pull requests and publishes
  `ghcr.io/aiaimimi0920/gateway` only for `main` and `Vx.y.z` tag pushes.
- `release-tag.yml` accepts only `Vx.y.z`, validates the tree, packages the
  release, creates a deterministic ZIP plus SHA-256 file, and publishes a
  GitHub Release.

## License

Gateway is distributed under the [MIT License](LICENSE).
