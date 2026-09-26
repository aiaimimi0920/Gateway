# Gateway orientation - 2026-09-25

## Scope

Subsequent development is scoped to the independent Gateway repository.
Origin: https://github.com/aiaimimi0920/Gateway.git
Observed HEAD: 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d.
Extensive tracked and untracked changes already exist; preserve them.
After each major implementation task, build and verify a fresh version under
C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/<versionId>.

## Graphify analysis

Queried Graphify with Gateway as explicit project_path. Existing graph:
17,316 nodes, 49,618 edges, 642 communities; 97% EXTRACTED, 3% INFERRED.
High-degree nodes: GatewayError (2,107 edges), ProviderAccountPayload (665),
CanonicalRelayRequest (543), AppState (468). Degree is a navigation aid.

The graph was last modified on 2026-08-16, about 40 days ago. It was not
rebuilt here. Broad queries were truncated and mixed tests with production
code. Use specific-symbol queries and verify current source before editing.

## Current-source sampled runtime entries

- src/main.rs:65-86: async_main selects standalone/worker/splitter.
- src/runtime.rs:22-85: run_gateway_runtime constructs state and resources.
- src/http/router.rs:29-58: build_router mounts endpoints and middleware.
- src/http/router/inference.rs:15-48: protocol routes and body limits.
- src/protocol/canonical.rs:5-47,167: ProtocolFamily, EndpointKind,
  CanonicalRelayRequest form the normalized request boundary.
- src/pipeline/mod.rs:4-10,28-36,63-80: auth -> filter -> route -> rate
  limit -> send -> finalize; PipelineContext owns per-request state.
- src/pipeline/stage_route.rs:1-50: candidate selection; reuse
  credential_to_candidate and build_candidate_queue.
- src/pipeline/stage_send.rs:1-12: fallback, execute_with_retry, TrackedStream.
- src/credential_store.rs:1-70: bounded TTL CredentialMemoryCache.

Flow: HTTP handlers -> protocol normalization -> canonical request ->
pipeline -> candidate queue -> upstream adapters/browser workers -> response
and stream handling. Reuse existing routing, retry and resource owners.

## Graph-derived targets requiring current-source verification

- Console: src/console/document.rs, revision.rs, secrets.rs.
- Desktop: apps/desktop/src/App.tsx, platform/HostProvider.tsx,
  api/client.ts, api/contracts.ts, platform/browserHost.ts.
- Native launcher: apps/desktop/src-tauri/src/main.rs.
- Workers: scripts/producer-browser-worker.mjs,
  scripts/chatgpt-web-session-worker.mjs. Producer integration was indexed
  under src/upstream/producer_media_helpers.rs; owners may have moved.
- Provider metadata: manifests/; runtime adapters: src/upstream/.
- Tests: tests/, scripts/tests/, and desktop-local tests.

## Boundaries and next-task checks

README and INTEGRATION_CONTRACT were read directly. Gateway owns relay,
routing, credentials, management APIs, workers, desktop and packaging.
Platform owns public accounts/permissions/quota/billing; Loom owns
orchestration; Hook owns foreground capture. Integrate through APIs.
Documented configuration requires Redis; PostgreSQL is optional.

Read owning subsystem docs and docs/plan/parallel-refactor-board.md before
selecting a write scope. Apply formatter, focused tests, dependent compile
or typecheck, effective-line ratchet and scoped Git diff checks.

## Major-task release workflow

Use tools/build-gateway-release.ps1, then tools/package-gateway-release.ps1
with a fresh -VersionId, -SkipBuild, -AllowCustomReleaseRoot and -ReleaseRoot
C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway.
The documented builder produces gateway, gateway-ui, web assets and
provenance. SkipBuild requires provenance matching binaries and source.
Run tools/smoke-gateway-packaged-runtime.ps1 -IntegrityOnly and
tools/smoke-gateway-ui-release.ps1 against the resulting -ReleaseDir.
Add task-relevant runtime verification; integrity alone does not prove
live provider acceptance. Version directories are immutable.

## Evidence boundary

Completed Graphify queries/statistics, current runtime-source sampling,
Git identity/status inspection, and direct reading of project instructions,
README and integration contract. No runtime code changes, tests, release
build, live provider calls, commit or push were performed for this analysis.
