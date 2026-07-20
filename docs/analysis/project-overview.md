# Gateway Project Overview

## Scope

This document describes the standalone Gateway repository, integrated into Neuro
workspace and the productization target defined in
`docs/superpowers/specs/2026-07-18-gateway-productization-design.md`.

## Runtime Entry Points

| Entry point | Responsibility |
| --- | --- |
| `src/main.rs` | Parses `--help`/`--version`, loads environment configuration, and dispatches `splitter`, `worker`, or `standalone`. |
| `src/runtime.rs` | Builds `AppState`, opens Redis and optional PostgreSQL, loads route configuration, starts maintenance tasks, and binds Axum. |
| `src/splitter.rs` | Manages worker launch, readiness polling, drain, replacement, and traffic cutover. |
| `src/http/router.rs` | Mounts public protocol routes, internal management routes, health/readiness, metrics, and websocket endpoints. |
| `src/pipeline/mod.rs` | Runs the canonical auth/filter/route/send/finalize request pipeline. |
| `apps/desktop/src-tauri/src/lib.rs` | Tauri command registration for profiles, sidecar lifecycle, logs, and diagnostics. |
| `tools/build-gateway-release.ps1` | Builds the headless binary and desktop shell. |
| `tools/smoke-gateway-ui-release.ps1` | Verifies an existing Gateway desktop artifact and optionally launches the UI shell. |

## Runtime Modes and Dependencies

`GATEWAY_RUNTIME_ROLE` accepts `splitter`, `worker`, and `standalone`; the
default is `splitter` (`src/config.rs:4-21,93-103`).

`GATEWAY_REDIS_URL` is required. PostgreSQL is optional at process startup, but
is required for the Rust-owned user-key, provider-account, quota, operator, and
audit surfaces that use DB state (`src/runtime.rs:33-57`). Routes load from
Redis first, then `GATEWAY_ROUTES_FILE`/`routes.yaml`; if neither exists the
process remains alive but rejects model requests (`src/runtime.rs:205-236`).

## Request Architecture

Public handlers normalize requests into canonical protocol structures and pass
them through one pipeline:

1. Auth and credential-source resolution.
2. Content filtering and quota pre-deduction.
3. Candidate construction, health/cooldown filtering, and affinity.
4. Provider credential exchange and upstream retry/fallback.
5. Usage settlement, audit, cache, and failure refund.

The main implementation is split across `src/pipeline/`, `src/routing/`,
`src/protocol/`, and `src/upstream/`. `src/upstream/client.rs` remains the
largest concentration of provider execution logic and is a later
maintainability target, not a Phase 1 prerequisite.

## Product Surfaces

- Public compatibility APIs: chat, completions, responses, messages, models,
  embeddings, audio, images, search, fetch, research, music, video, realtime,
  and Gemini Live.
- Internal Gateway APIs under `/v1/internal/gateway/**` for access keys,
  provider accounts/credentials, quota, audits, analysis, browser executor,
  runtime drain, and operations.
- Browser executor direct APIs under `/v1/internal/browser-executor/**`.
- Desktop shell for profile management, sidecar lifecycle, health/readiness,
  model probing, API smoke requests, log access, and diagnostics.

## Build and Validation

The existing local gate is:

```powershell
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
node --test scripts/tests/*.test.mjs
cargo test --locked
cd apps/desktop; npm run typecheck
```

The productization gate adds a staged-package E2E that uses an isolated Redis,
an isolated port, and no real upstream calls by default.

## Productization Boundaries

Phase 1 changes the desktop/release boundary without moving routing or
credential logic out of the headless runtime. Phase 2 adds provider evidence
and canary contracts around existing lines. Phase 3 adds observability,
reliability, access isolation, and recovery contracts around the same runtime.

All source changes remain in this repository; release output remains under
`release/Gateway/`.
