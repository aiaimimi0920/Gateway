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

- Rust gateway runtime and HTTP/WebSocket APIs (`neuro-gateway`).
- Provider routing, credentials loaded by the runtime, and relay behavior.
- Provider line manifests under `manifests/` and route configuration.
- Browser-backed provider workers under `scripts/`.
- Gateway management endpoints and the local desktop shell
  (`neuro-gateway-ui`).
- Release packaging, integrity manifests, and operational evidence tools.

Platform, Loom, and Hook implementation code is intentionally not copied into
this repository. The ownership and integration rules are documented in
[`INTEGRATION_CONTRACT.md`](INTEGRATION_CONTRACT.md).

## Repository Layout

```text
./
├── apps/desktop/       # TypeScript frontend and Tauri launcher
├── manifests/          # Provider line manifests and JSON schema
├── scripts/            # Browser workers and worker tests
├── src/                # Rust library and neuro-gateway binary
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
cargo run --locked --bin neuro-gateway
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
provenance, manifests, browser workers, documentation, tools, `manifest.json`,
and `checksums.sha256`.

Build and package a candidate from the repository root:

```powershell
$id = "gateway-product-" + (Get-Date -Format "yyyyMMdd-HHmmss")
.\tools\build-gateway-release.ps1
.\tools\package-gateway-release.ps1 -VersionId $id -SkipBuild
```

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

Build the service image from the Gateway root:

```powershell
docker build -t neuro-gateway:local .
docker run --rm -p 4200:4200 --env-file .env neuro-gateway:local
```

The image contains the release binary, route files, provider manifests, and
browser workers. Runtime secrets are supplied through environment variables or
an env file; they are not read from `.env` during the image build.

## GitHub Automation

- `ci.yml` validates Windows and Linux builds, Python contracts, Node workers,
  Rust targets, the desktop frontend, and the Tauri wrapper.
- `build-windows.yml` creates and uploads an immutable Windows candidate under
  `release/Gateway`.
- `docker.yml` builds on pull requests and publishes
  `ghcr.io/aiaimimi0920/gateway` only for `main` and `Vx.y.z` tag pushes.
- `release-tag.yml` accepts only `Vx.y.z`, validates the tree, packages the
  release, creates a deterministic ZIP plus SHA-256 file, and publishes a
  GitHub Release.

## License

Gateway is distributed under the [MIT License](LICENSE).
