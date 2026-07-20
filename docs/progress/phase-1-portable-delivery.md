# Phase 1: Portable Delivery Progress

## Completion Criteria

- Package contains all runtime files for the selected mode.
- No build-machine absolute path is required.
- Profile catches missing dependencies before launch.
- Authorized drain is graceful and lifecycle state is explicit.
- Packaged E2E passes twice consecutively.
- Existing Gateway gates remain green.

## Work Items

| Item | Status | Evidence |
| --- | --- | --- |
| Package layout contract test | completed | `tests/python/test_gateway_package_contract.py` |
| Gateway-owned packager | completed | `tools/package-gateway-release.ps1` implements `gateway-package/v3`, immutable publication, source fingerprinting, and build-provenance binding |
| Packaged runtime smoke | completed | Two runs against `gateway-product-20260721-010734`; random ports, disposable Redis ownership, authorized drain with exit code 0, and cleanup all pass |
| Portable path resolution | completed | desktop Rust contract tests |
| Role and management token profile fields | completed | Rust serialization and frontend validation contracts |
| Dependency preflight | completed | Sidecar, Redis, optional DB, working directory, routes probes |
| Graceful drain outcome states | completed | `graceful`, `forced`, `exited`, startup state and diagnostics |
| Package support contract | completed | Root `.env.example`, canonical `scripts/invoke-gateway-live-provider-canary.ps1`, and `gateway-build-provenance.json` are covered by manifest/checksums; 10 package tests cover rollback, concurrent source mutation, and source-only tool exclusion |
| Isolated UI smoke | completed | Artifact integrity and optional 5-second launch smoke pass; no package-owned headless sidecar was auto-started |

## Current Known Constraints

The headless runtime requires Redis. Readiness may additionally require PostgreSQL, object storage, API-key secret, and public base URL according to the existing configuration contract; the desktop preflight must report those conditions without changing runtime semantics.

The canonical packaged canary is standalone and its dry-run contract is verified from the release directory. The complete `tools/run-gateway-line-evidence.ps1` workflow remains a source-repository tool because its offline verifier and inventory generation still consume `Cargo.toml`, `src/**`, and deployment helpers; the packager intentionally excludes that runner from the production package.
