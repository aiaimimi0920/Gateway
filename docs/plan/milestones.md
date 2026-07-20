# Gateway Productization Milestones

## M1: Portable Package Contract

**Exit evidence:** deterministic staged directory contains headless and desktop executables, routes policy, manifests, scripts, runtime metadata, manifest, and SHA-256 checksums; package contract test is green.

## M2: Portable Desktop Runtime

**Exit evidence:** profile role/token/path/dependency validation is green; sidecar starts from a copied package outside the checkout; authorized drain completes gracefully; missing dependency errors are actionable.

## M3: Packaged Runtime Acceptance

**Exit evidence:** isolated Redis/port E2E passes twice, verifies health/readiness/models/error semantics/drain/process cleanup, and UI smoke verifies no implicit sidecar launch.

## M4: Provider Evidence Surface

**Exit evidence:** every manifest line appears in deterministic inventory; offline feature/fixture matrix produces evidence records; live canaries are explicit and classify credential/quota/challenge/region/network/upstream outcomes.

## M5: Enterprise Runtime Baseline

**Exit evidence:** stable structured metrics/logs/traces, dependency fault matrix, splitter replacement invariants, access/rate-limit contracts, and operator summary are executable.

## M6: Recovery and Release

**Exit evidence:** isolated backup/restore verification passes; operations manual matches commands/endpoints; final package and evidence are written only to `release/Gateway/<versionId>`; complete validation gate is green.

