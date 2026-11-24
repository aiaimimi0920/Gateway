# Neuro Gateway

[![CI](https://github.com/aiaimimi0920/Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/ci.yml)
[![Build Windows](https://github.com/aiaimimi0920/Gateway/actions/workflows/build-windows.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/build-windows.yml)
[![Docker](https://github.com/aiaimimi0920/Gateway/actions/workflows/docker.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/docker.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**English** | [简体中文](README.zh-CN.md)

[Documentation index](docs/README.md)

Neuro Gateway is an independently runnable Rust AI gateway and provider relay.
It exposes OpenAI-compatible and provider-specific APIs, owns provider line
manifests and routing, and ships a local Windows desktop launcher alongside
the headless service.

Repository: <https://github.com/aiaimimi0920/Gateway>

Gateway can run by itself on a developer workstation or be managed by Neuro
Platform and Loom through its API boundary. Platform remains responsible for
accounts, permissions, quota, billing, and public web policy. Loom remains
responsible for orchestration and local AI-brain workflows. Otter remains the
foreground capture and integration client.

## What Gateway Owns

- Rust gateway runtime and HTTP/WebSocket APIs (`gateway`).
- Provider routing, credentials loaded by the runtime, and relay behavior.
- Provider line manifests under `manifests/` and route configuration.
- Browser-backed provider workers under `scripts/`.
- Gateway management endpoints and the local desktop shell
  (`gateway-ui`).
- Release packaging, integrity manifests, and operational evidence tools.

Platform, Loom, and Otter implementation code is intentionally not copied into
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
- Rust `1.98.0` or the version pinned in `rust-toolchain.toml`.
- Node.js `>=22.22.0` and npm for browser workers and the desktop frontend.
- Python `3.11+` for repository validators and contract tests.
- PowerShell 5.1+ for the Windows build and smoke scripts.
- Docker Engine or Docker Desktop for container builds.

## Clone And Build

Gateway is a standalone repository. Commands below are run from the Gateway
repository root, not from a Neuro monorepo checkout:

```powershell
git clone https://github.com/aiaimimi0920/Gateway.git
Set-Location Gateway

npm run test:effective-lines --prefix scripts
npm run check:effective-lines --prefix scripts
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked -- --test-threads=1
```

The effective-code-line gate uses the Neuro thresholds of 150/500/700/1500
with a checked-in adoption baseline. CI rejects new oversized files and growth
in recorded files above 700 lines without forcing unrelated work to clear all
historical debt. The scanner, strict audit, exception schema, supported
languages, and reviewed exclusions are documented in
[`docs/effective-code-lines.md`](docs/effective-code-lines.md).

The repository Cargo config uses a conservative fallback for direct builds.
For routine Windows packages, use the [incremental devtest workflow](docs/devtest-build.md);
the formal release tooling is separate.

Validate manifests and Python contracts:

```powershell
python -m pip install --disable-pip-version-check -r tests/python/requirements.txt
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
```

Install and test browser workers:

```powershell
npm ci --prefix scripts
npm run audit:prod --prefix scripts
node --test scripts/tests/*.test.mjs
```

Build and check the desktop shell:

```powershell
npm ci --prefix apps/desktop
npm run audit:prod --prefix apps/desktop
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop run build
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml
```

## Runtime Configuration

The service reads runtime settings from environment variables. Server mode
(`GATEWAY_STORAGE_MODE=server`) requires `GATEWAY_REDIS_URL`; `GATEWAY_DATABASE_URL`
or `DATABASE_URL` enables PostgreSQL-owned services. Local mode
(`GATEWAY_STORAGE_MODE=local`) uses embedded SQLite and a standalone process without
external database services. Desktop-managed backends select local mode automatically.
Windows EXE defaults and the stable `.ng` data directory are documented in
[local data storage](docs/local-data-storage.md); shared quota rules are documented in
[local access keys](docs/local-access-keys.md). Copy `.env.example` to a local `.env`
for a server development run. Local environment files remain ignored.

The optional shared startup file `.ng/gateway.json` contains
`enable_management_ui`. Its value is ANDed with the compiled edition capability;
see [`docs/release-editions.md`](docs/release-editions.md). An absolute
`GATEWAY_CONFIG_FILE` can select a different file.

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
`release/Gateway/<versionId>` directory by default. Gateway publishes three
edition shapes; see [`docs/release-editions.md`](docs/release-editions.md) for
the capability matrix and configuration switch.

Build and package a candidate from the repository root:

Before every release, complete the [static model display catalogue review](docs/model-display-catalog-release.md).
Refresh official model identities, review the fixed company/model prominence order,
record sources and unknown-model fallback, then freeze the table before building.

```powershell
$id = "gateway-api-" + (Get-Date -Format "yyyyMMdd-HHmmss")
.\tools\build-gateway-release.ps1 -Edition api
.\tools\package-gateway-release.ps1 -Edition api -VersionId $id -SkipBuild -UseExampleRoutes
```

Use `-Edition web` for the browser management backend without the desktop
launcher, or `-Edition desktop` for both the browser backend and
`gateway-ui.exe`. `-Edition all` builds `api`, `web`, and `desktop` serially.

`-UseExampleRoutes` packages `routes.example.yaml` under both route filenames.
Automated candidates and tagged releases always use this mode. Without the
switch, local packaging retains the developer's `routes.yaml`; use example
mode before distributing a package. Neither mode modifies the source route files.

`build-gateway-release.ps1` requires Node.js `>=22.22.0`. API builds only audit
the browser-worker dependency tree and compile with
`--no-default-features --features runtime-defaults`; Web and Desktop builds
also install and audit the desktop dependencies, typecheck, and perform
`npm run build:web` before the Cargo release build. It exports
`GATEWAY_PREBUILT_WEB_UI=1` for that Cargo step so the root `build.rs` reuses
prebuilt web assets instead of launching another implicit browser-console
build. Desktop additionally builds the Tauri shell. The script enforces one
Cargo job with incremental compilation disabled.

The package command can build for you when `-SkipBuild` is omitted. A skipped
build is accepted only when the edition-specific provenance matches the
requested capabilities and current source tree. Use a new version id for every
package; an existing directory is never overwritten. Release copying excludes
test directories and generated test artifacts; source tests remain available in
the repository.

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
.\tools\smoke-gateway-local-storage.ps1 `
  -ReleaseDir ".\release\Gateway\$id"
```

Browser workers are deliberately shipped without `node_modules`. Install them
after extracting a package:

```powershell
npm ci --prefix ".\release\Gateway\$id\scripts"
npm run audit:prod --prefix ".\release\Gateway\$id\scripts"
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

Use the [deployment guide](deploy/README.md#quick-start) for Compose setup,
secrets, persistence, and verification. For updates to an existing database,
see the [schema and migration instructions](deploy/README.md#postgresql-and-the-gateway-schema);
do not remove the persistent volume to upgrade.

For management after startup, see the [operations manual](docs/operations-manual.md)
and [credential test policies](docs/credential-test-policies.md).

### 可选 Codex 通道与独立站点健康

See [Basispoints and site health](docs/basispoints-and-site-health.md) and
[upstream client profiles](docs/request-client-profiles.md).

## GitHub Automation

- Windows candidates, Docker images, and tagged releases depend on the reusable
  Security workflow. Dependency and secret scans resolve one immutable commit;
  the publishing job checks out that same commit only after all security jobs
  succeed. See [security release gates](docs/security-release-gates.md).
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
