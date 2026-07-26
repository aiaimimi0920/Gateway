# Rust Gateway Migration Plan

## Purpose

This plan corrects the previous migration model.

The previous draft assumed a long-term architecture of:

- Rust runtime gateway
- TypeScript control plane owner
- TypeScript relay as a temporary but structurally central dependency

That is not the target anymore.

The corrected target is:

- Rust `gateway/` becomes the sole long-term gateway product
- TypeScript gateway runtime must be eliminated
- website/admin surfaces may remain in TypeScript, but they consume Rust gateway APIs rather than owning the gateway truth

## Hard Decisions

These decisions are fixed unless explicitly re-approved:

1. `gateway/` is the only long-term public gateway.
2. The old TypeScript gateway has been removed from the repository and is not an accepted target.
3. `packages/ai-gateway-domain` is not the long-term AI gateway owner.
4. Rust gateway must directly own gateway runtime plus gateway management APIs.
5. Rust gateway may use `PostgreSQL + Valkey + object storage` directly; it is not restricted to Redis projection consumption only.
6. Website/admin surfaces are clients of the Rust gateway, not the permanent owner of its key system, credential libraries, or runtime control plane.

## Target Product Model

Rust gateway is not just a relay.

It is a standalone product that must eventually provide:

- public inference APIs
- `baseURL + api_key` access model
- user-bound API key validation
- special platform/project API key validation
- quota/eligibility checks
- real provider credential library
- user credential library
- key distribution and key management APIs
- single and batch real credential upload APIs
- prewarm / cache / keepalive / session orchestration
- routing / retry / failover / audit / metrics

## External Key Model

The gateway must support two external key classes.

### 1. Special API Key

Behavior:

- not bound to a single end user
- gateway validates the key and its quota/eligibility
- if valid, gateway chooses a real credential from the real credential library based on model and route policy

### 2. User API Key

Behavior:

- bound to a user
- gateway validates user eligibility, remaining calls, or quota
- if valid, gateway still chooses a real credential from the real credential library based on model and route policy

### Shared Rule

Both key classes must converge into the same unified request pipeline after auth.

External keys do not directly equal upstream provider secrets.

## Data Ownership

Rust gateway must long-term own direct access to:

- API keys
- user credential library
- real credential library
- route policies
- model association and routing metadata
- sessions
- request audits
- prewarm/runtime cache state

Storage model:

- `PostgreSQL` = truth store
- `Valkey` = hot cache / locks / runtime state / prewarm indexes
- object storage = large runtime material, artifacts, browser state bundles

Redis is an acceleration layer, not the only source of truth.

## Current Wrong Assumptions To Remove

These assumptions must be removed from docs and planning:

1. "TypeScript control plane remains the truth owner"
2. "Rust gateway should not read the database directly"
3. "User credential issue/verify/revoke should remain in TypeScript by default"
4. "Provider credential CRUD should remain in TypeScript by default"
5. "Website backend is the gateway owner and Rust is only the runtime"

## Migration Scope

### In Scope

The migration must move all gateway-relevant ownership into Rust, including:

- request auth ingress
- API key validation
- user eligibility/quota checks
- real credential selection
- real credential library management
- user credential library management
- provider payload/runtime metadata consumption
- route policy enforcement
- response cache behavior
- usage tracking hooks
- prewarm/keepalive/session orchestration
- management APIs for key distribution and credential upload
- deployment cutover

### Out Of Scope

The following may remain outside Rust gateway as clients or adjacent systems:

- website UI pages
- operator UI rendering
- account-center visual workflows
- generic non-gateway business features unrelated to gateway operation

## Ownership Matrix

| Concern | Final Owner | Notes |
| --- | --- | --- |
| Public AI gateway runtime | Rust `gateway/` | Sole long-term entry point |
| API key system | Rust `gateway/` | Includes issue/rotate/revoke and distribution APIs |
| User credential library | Rust `gateway/` | Truth store plus management APIs |
| Real credential library | Rust `gateway/` | Truth store plus single/batch upload APIs |
| Runtime routing and failover | Rust `gateway/` | Includes retry, cooldown, prewarm, sticky/session behavior |
| Website/admin UI | Web clients | Consume Rust gateway APIs |
| Legacy TS relay/domain code | Transitional only | Source for migration, not final owner |

## Migration Inventory

### A. Public Runtime

Rust must own:

- `/v1/chat/completions`
- `/v1/responses`
- `/v1/messages`
- `/v1/images/generations`
- `/v1/music/generations`
- `/v1/videos/generations`
- `/v1/search`
- `/v1/fetch`
- `/v1/research*`
- `/v1/credits/balance`
- `/v1/models`

### B. Key System

Rust must own APIs and storage for:

- special API key issuance/rotation/revocation
- user API key issuance/rotation/revocation
- access-key quota and eligibility validation
- key distribution to callers

### C. User Credential Library

Rust must own:

- storage
- read APIs
- write APIs
- validation logic
- eligibility accounting inputs

### D. Real Credential Library

Rust must own:

- single credential upload
- batch credential upload
- credential status management
- provider runtime metadata
- health/cooldown/failover state
- prewarm inputs

### E. Runtime Support Systems

Rust must own:

- response cache
- usage tracking
- route policy
- candidate queue
- key selection strategy
- browser-backed dispatch orchestration
- keepalive/session ensure
- request audit

## Current Codebase Classification

### Rust Target

- `gateway/`

### TypeScript Migration Sources

- `packages/ai-gateway-domain/`

These two areas are sources to mine behavior from, not long-term architecture anchors.

### Website/Admin Clients

- `web/`
- other UI/operator surfaces

These should eventually call Rust gateway APIs instead of owning gateway truth.

## Execution Phases

### Phase 0: Freeze The Correct Boundary

Tasks:

- mark Rust as the sole long-term gateway owner
- mark TypeScript relay/domain code as migration sources only
- document website/admin as Rust API clients

Exit criteria:

- no planning doc still describes TS as the long-term gateway owner

### Phase 1: Canonical Data Model

Tasks:

- define Rust-owned truth objects for:
  - special API keys
  - user API keys
  - user credential library
  - real credential library
  - route policies
  - request audits
- define DB/Redis/object-storage responsibilities for each

Exit criteria:

- Rust truth ownership is unambiguous

### Phase 2: Auth And Access Key System

Tasks:

- implement full Rust access-key validation model
- support both special keys and user-bound keys
- ensure both flow into one unified runtime pipeline

Exit criteria:

- Rust validates both key classes on the public gateway path

### Phase 3: Credential Libraries And Management APIs

Tasks:

- add Rust management APIs for user credential library
- add Rust management APIs for real credential library
- support single and batch real credential upload
- expose Rust APIs for website/admin use

Implemented in current Rust gateway:

- project/special key resolve + rotate
- user credential issue/verify/revoke
- provider credential CRUD + batch upload + cache warm/invalidate
- provider account CRUD
- route policy CRUD
- model alias CRUD
- operator readiness / provider inventory / provider probe internal APIs

Exit criteria:

- website/admin no longer depend on TypeScript gateway ownership for these operations

### Phase 4: Runtime Behavior Parity

Tasks:

- port response cache scope and TTL logic
- port quota hooks
- port usage reporting
- port failover / cooldown / multi-key selection
- port runtime keepalive/session behavior

Implemented in current Rust gateway:

- request body limit parity for public endpoints
- DB-owned route policy resolution on the runtime path
- project-aware `/v1/models`
- quota pre-check + pre-deduct hook
- object-storage-backed provider payload loading
- provider probe success/failure state write-back
- operator inventory aggregation over provider health + recent usage samples
- runtime provider success/failure persistence wired into `stage_send`
- cooling sweep wired into route/model resolution and internal operator API
- Redis-backed quota pre-deduct now settles on success and refunds on failure
- `gateway_request_audits` now start/finalize on the Rust pipeline path for project-api-key traffic, including stream completion/failure snapshots and minimal route trace persistence
- Rust internal request-audit read surfaces now expose `/v1/internal/gateway/requests` and `/v1/internal/gateway/requests/summary`
- `gateway_request_audits` now accept either `api_key_id` or `user_credential_id`, so user-credential traffic can enter the same Rust audit pipeline
- Rust request-audit detail reads now expose `/v1/internal/gateway/requests/:requestAuditId` and `/v1/internal/gateway/requests/by-response/:responseId`
- Rust request-artifact reads now expose `/v1/internal/gateway/requests/:requestAuditId/artifacts` and `/v1/internal/gateway/requests/by-response/:responseId/artifacts`
- Rust analysis reads now expose `/v1/internal/gateway/analysis/samples` and `/v1/internal/gateway/analysis/summary` on top of `gateway_request_audits`
- Rust now persists winning-candidate routing score metadata into `routeTrace.selectedCandidate` and exposes `/v1/internal/gateway/analysis/provider-routing/summary` from the same request-audit source
- Rust now also exposes `/v1/internal/gateway/analysis/provider-routing/anomaly-report`, including TS-aligned `balanced / conservative / aggressive` threshold profiles and override support
- Rust now owns provider-routing anomaly incident sync over `gateway_analysis_anomaly_incidents`, plus `/v1/internal/gateway/analysis/provider-routing/anomaly-incidents/sync`, `/v1/internal/gateway/analysis/anomaly-incidents`, and `/v1/internal/gateway/analysis/anomaly-incidents/summary`
- Rust anomaly incident management now also covers `/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/history`, `.../:incidentId/acknowledge`, `.../:incidentId/resolve`, and `.../:incidentId/follow-up`
- Rust anomaly remediation now exposes `/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/remediation-plan`, `/v1/internal/gateway/analysis/anomaly-incidents/:incidentId/remediation-runs`, `POST /v1/internal/gateway/analysis/anomaly-incidents/:incidentId/remediation-runs`, `/v1/internal/gateway/analysis/remediation-runs`, and `/v1/internal/gateway/analysis/remediation-runs/summary`
- Rust remediation impact/effectiveness now also exposes `/v1/internal/gateway/analysis/remediation-runs/:runId/impact`, `POST /v1/internal/gateway/analysis/remediation-runs/:runId/capture-impact`, and `/v1/internal/gateway/analysis/remediation-runs/effectiveness`
- Rust remediation effectiveness now also supports persisted snapshots and trend reads through `/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshot`, `/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots`, `/summary`, and `/trend-report`
- Rust remediation snapshot governance now also exposes `/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/anomaly-report`
- Rust remediation snapshot reads now also cover `GET /v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/:snapshotId`
- Rust now also supports persisted remediation-effectiveness anomaly snapshots through `/v1/internal/gateway/analysis/remediation-runs/effectiveness/snapshots/anomaly-snapshots`, including list/get/create parity with the TypeScript control-plane surface
- Rust anomaly governance now also exposes the first remediation queue read model through `GET /v1/internal/gateway/analysis/remediation-queue`, using Rust-owned incident/plan/run data plus queue scheduling semantics instead of the old TypeScript service layer
- Rust remediation automation now also exposes `POST /v1/internal/gateway/analysis/remediation-runs/sweep`, reusing the Rust queue scheduler plus the existing remediation executor instead of the legacy TypeScript sweep path
- Rust remediation queue/sweep now also resolve anomaly policies directly from SQL, carry `policy` back in queue items, and reuse the same policy sync schedule semantics as the old TypeScript governance layer
- Rust hotspot auto-remediation now also covers `tighten-project-rate-limit`, `tighten-api-key-rate-limit`, `tighten-model-rate-limit`, and `tighten-endpoint-rate-limit`, with route-policy patch guards that only allow tighter limits and normalize scoped keys before persistence
- Rust gateway now also exposes the first anomaly-policy management surface through `/v1/internal/gateway/analysis/anomaly-policies` and `/summary`, and can persist anomaly-policy SQL truth without routing those writes back through the legacy TypeScript service
- Rust gateway now also exposes rate-limit hotspot governance reads through `/v1/internal/gateway/analysis/rate-limit-hotspots`, `/trend-report`, and `/anomaly-report`, all derived directly from Rust-owned `gateway_request_audits`
- Rust gateway now also persists and reads rate-limit hotspot snapshots and anomaly snapshots through `/v1/internal/gateway/analysis/rate-limit-hotspots/snapshot`, `/snapshots`, `/snapshots/summary`, `/snapshots/trend-report`, `/anomaly-snapshot`, and `/anomaly-snapshots`, backed by Rust-owned object-storage manifests
- Rust gateway now also owns `POST /v1/internal/gateway/analysis/rate-limit-hotspots/anomaly-incidents/sync`, consuming Rust-owned hotspot anomaly snapshots and writing the shared anomaly incident table/history without routing the sync back through TypeScript
- Rust gateway now also exposes `POST /v1/internal/gateway/analysis/anomaly-policies/{policyId}/sync` and `/sweep-sync`, dispatching supported policy tags (`provider-routing`, `rate-limit-hotspot`, `analysis-export`) onto Rust-owned sync paths and writing policy sync state in SQL
- Rust gateway now also owns anomaly incident alert operations through `GET /v1/internal/gateway/analysis/anomaly-incidents/alert-queue` and `POST /v1/internal/gateway/analysis/anomaly-incidents/{incidentId}/alert-dispatch`, including policy-aware scheduling, remediation-action docketing, and alert delivery write-back/history
- Rust gateway now also exposes `POST /v1/internal/gateway/analysis/anomaly-incidents/sync` as a unified sync entrypoint for already-migrated anomaly families, so provider-routing, rate-limit-hotspot, and analysis-export orchestration no longer needs the old TypeScript sync wrapper
- Rust gateway now also exposes `GET /v1/internal/gateway/analysis/export` and `POST /v1/internal/gateway/analysis/export`, generating sanitized analysis rows directly from Rust-owned request audit/artifact data and persisting dataset/manifest artifacts into Rust-owned object storage plus `gateway_analysis_exports`
- Rust gateway now also exposes `GET /v1/internal/gateway/analysis/exports`, `/summary`, and `/:exportId`, so persisted analysis export list/get/inventory reads no longer depend on the legacy TypeScript gateway service
- Rust gateway now also exposes `POST /v1/internal/gateway/analysis/exports/:exportId/metadata` and `POST /v1/internal/gateway/analysis/exports/cleanup-expired`, so persisted export metadata edits and retention cleanup also stay inside Rust gateway instead of bouncing back to the legacy TypeScript layer
- Rust gateway now also exposes `GET /v1/internal/gateway/analysis/exports/diff`, `/baseline-report`, `/timeline-report`, `/trend-report`, and `/anomaly-report`, so persisted analysis export comparison, trend, and anomaly governance reads no longer depend on the legacy TypeScript gateway service
- Rust gateway now also wires persisted analysis export anomaly reports into the shared SQL-backed incident lifecycle, so export anomaly sync, escalation, and history logging no longer bounce back through the legacy TypeScript gateway service
- Rust gateway now also exposes `GET /v1/internal/gateway/model-associations`, `/costs`, and `/pressure`, so the operator model matrix, cost governance overview, and runtime pressure dashboard no longer require the legacy TypeScript gateway service; `/pressure` currently prefers Rust `ConcurrencyRegistry + gateway_request_audits` while still honoring legacy `ai-gateway:*:concurrency` Redis counters during the cutover window
- Rust gateway now also owns the unified access system truth layer: `provider_capability_catalog`, `platform_access_catalog`, `access_bundles`, `access_bundle_items`, `access_keys`, `access_key_balances`, and `access_key_aggregate_memberships`, with migration backfill from legacy project API keys and user credentials into the new `access_keys` model
- Runtime auth and route selection now prefer unified `access_keys`, including explicit access-table filtering, balance checks, sticky access affinity, `auto_route` source-key selection, and request-audit persistence of `access_key_id` / `source_access_key_id`
- Public `neuro_*`, legacy `new_api_*`, and `gw-user-*` auth now resolve against unified `access_keys` only; legacy `gateway_api_keys` / `gateway_user_credentials` are no longer the active public hot-path fallback
- Rust gateway now also owns provider-side quota snapshots as a first-class runtime concern, including a unified `provider quota snapshot` shape, Redis-backed hot cache/refresh locks, operator-readable quota views, and route-time penalty/exclusion semantics that stay distinct from platform `access_key` balances
- Codex provider quota is now formally sourced from `GET https://chatgpt.com/backend-api/wham/usage`, normalized into `5h / 7d` window snapshots, exposed through `/v1/internal/gateway/provider-accounts/{providerAccountId}/quota`, and consumed during candidate selection so exhausted Codex credentials are filtered before upstream dispatch
- Rust gateway now also exposes `/v1/internal/gateway/access/*` for provider capability CRUD, platform access CRUD, bundle CRUD, bundle-item replacement, access-key CRUD/rotate/revoke, balance adjustment, aggregate-membership replacement, candidate preview, route-decision preview, and sticky-affinity inspect/reset
- Web operator console now also exposes `/ops/gateway/access`, so operators can directly manage the explicit access table, bundle bindings, unified access keys, prepaid balances, auto-route memberships, and sticky affinity through Rust-owned APIs instead of legacy project/user key panels

Exit criteria:

- Rust gateway owns the real hot path behavior end to end

### Phase 5: Prompt Cache Intelligence (Anthropic-Specific)

Tasks:

- implement automatic prompt cache detection (check for existing `cache_control` markers)
- implement provider-aware auto-cache logic (only apply to Anthropic Claude models)
- implement intelligent content analysis (static vs dynamic system prompt blocks)
- implement cache effectiveness monitoring and cost tracking
- expose cache statistics in user dashboard

Key constraints:

- **Only apply to Anthropic Claude models** - `cache_control` is Anthropic-specific, adding to OpenAI/Gemini requests causes errors
- **Only apply when client hasn't already added cache markers** - respect Claude Code and other clients' own caching strategies
- **Monitor cache effectiveness** - track `cache_creation_input_tokens` and `cache_read_input_tokens` from responses
- **Calculate cost savings** - cached tokens cost 10% of normal ($1.5/M vs $15/M)

Implementation phases:

1. **Detection phase** (Week 1): Detect existing `cache_control` markers, log statistics
2. **Simple auto-cache** (Week 2): Cache last system block + last tool for non-cached requests
3. **Intelligent analysis** (Week 3-4): Split static vs dynamic content, apply targeted caching
4. **Monitoring dashboard** (Week 5+): User-facing cache hit rates and cost savings

Implemented in current Rust gateway:

- Anthropic packing now preserves client-supplied `system` / `tools` cache markers for native Anthropic requests instead of flattening them away during canonical round-trip
- Anthropic packing now applies the first safe auto-cache slice for Claude models only: when no existing `cache_control` markers are present, Rust auto-marks the last `system` block and last tool with `{"type":"ephemeral"}`
- Non-Claude / non-Anthropic upstream bodies are left untouched by this auto-cache path
- Anthropic usage parsing now also captures `cache_creation_input_tokens` and `cache_read_input_tokens` in the Rust runtime token-usage structure
- Rust finalize now writes those cache token fields into `gateway_request_audits`, propagates them through usage reports, and keeps the audit schema/contracts aligned with the new columns
- `/metrics` now exports prompt-cache aggregate counters from Rust (`hit requests`, `creation requests`, `cache creation tokens`, `cache read tokens`)
- Rust internal analysis now also exposes prompt-cache summary/trend reads with estimated cost savings, so operator/user-facing surfaces can consume structured cache statistics without re-aggregating raw request audits
- Rust request audits now also persist `client_has_cache_control` and `auto_cache_applied`, so prompt-cache monitoring can distinguish client-managed caching from Rust-managed auto-caching
- Rust prompt-cache summary/trend reads now expose `clientMarkedRequests`, `autoAppliedRequests`, and coverage-rate counters for adoption analysis
- `/metrics` now exports prompt-cache adoption counters from Rust (`client marked requests`, `auto applied requests`) alongside token-hit counters
- Web operator console now has a dedicated `/ops/gateway/prompt-cache` page, and the current `account-api -> ai-gateway-domain` transitional read path now mirrors the Rust prompt-cache summary/trend contract so adoption and savings no longer require ad-hoc SQL or raw trace inspection
- User-facing AI benefits now also expose rolling prompt-cache summary + trend reads for `service_proxy` access through `/v1/me/benefits/services/:serviceId/prompt-cache-summary` and `/v1/me/benefits/services/:serviceId/prompt-cache-trend-report`, and the benefit-center UI shows personal hit rate / coverage / saved tokens / estimated savings plus recent bucketed trend data without requiring operator-only access
- Web operator gateway reads/writes now cut over to direct Rust gateway calls from the platform `web` tier, and `services/account-api` no longer needs to register `gatewayRouter` as the default `/v1/internal/gateway/*` path
- Benefit-service gateway management now also cuts over to Rust internal management APIs: Rust owns `benefit-project ensure`, project-scoped `api-access`, and project-scoped prompt-cache reads, while `packages/account-domain` no longer needs to import `@neuro/ai-gateway-domain` for those user-facing benefit flows
- Rust `gateway` now also exposes the default keepalive steward contract (`/v1/credentials/ensure` + `/v1/internal/credentials/ensure`) and owns the default `session-backed runtime ensure` path for generic / Gemini Canvas / Chataibot / LumaLabs / Producer credentials
- Rust `gateway` now also exposes the default browser-executor service-contract (`/v1/internal/browser-executor/health` + `/v1/internal/browser-executor/execute`) for local worker / browser pool execution
- Rust `gateway` now also owns the browser-executor runtime management plane (`/v1/internal/gateway/browser-executor/*`), and the transitional TypeScript `browser-executor-runtime.ts` plus its `gatewayRouter` exposure have been removed from `@neuro/ai-gateway-domain`
- Local preview / compose / K8s base now default directly to Rust `gateway`: preview no longer starts the old TypeScript gateway, `web` no longer depends on it, and the default `new-api` ingress path points to Rust `gateway`
- Canonical public/operator defaults now also stop reinforcing the legacy path: K8s `BENEFIT_SERVICE_API_PUBLIC_BASE_URL` points at `https://new-api.example.com/v1`, operator service creation defaults to the canonical Rust base URL, and Rust no longer rewrites legacy keepalive URLs at runtime
- Default image build/push helpers now only build/push Rust `gateway`; the old TypeScript gateway has been removed from the default release toolchain
- Rust gateway public protocol compatibility now also covers:
  - `POST /v1/completions`
  - `POST /v1/embeddings`
  - `POST /v1/audio/transcriptions`
  - `POST /v1/audio/speech`
  - plus mirrored `/v1/new-api/*` aliases for the same endpoints
- Rust gateway tool-call compatibility now also covers the first cross-protocol bridge set:
  - OpenAI Chat `tool_calls` plus legacy `function_call`
  - OpenAI Responses `function_call` / `function_call_output`
  - Anthropic `tool_use` / `tool_result`
  - Gemini / Vertex `candidates[].content.parts[].functionCall`
  - Bedrock Converse `output.message.content[].toolUse`
  - Cohere `message.tool_calls` and top-level `tool_calls`
  - bidirectional `tool_choice` mapping between OpenAI and Anthropic
  - Anthropic `stop_reason:"tool_use"` normalization to OpenAI/canonical `finish_reason:"tool_calls"`
  - same-protocol request packing still preserves original tool-choice/tool schema shapes where possible instead of forcing unnecessary re-encoding
- Upstream JSON response unpacking now also falls back through the generalized multi-format parser after native OpenAI / Responses / Anthropic decoding fails, so non-OpenAI-family tool-call response shapes no longer require bespoke public exit adapters just to be normalized back into OpenAI / Anthropic caller formats
- Rust gateway now also has first-party request packers for:
  - `gemini_api_compatible`
  - `bedrock_converse_compatible`
  - `cohere_compatible`
  and reuses the generalized multi-format stream translator / accumulator for their first streaming tool-call pass, instead of only relying on response-side fallback parsing
- The non-OpenAI-family tool-call bridge now also covers deeper stream ingestion:
  - Gemini / Vertex `functionCall` incremental content parsing
  - Bedrock Converse `messageStart / contentBlockStart / contentBlockDelta / messageDelta`
  - native AWS eventstream frame payload decoding inside the generalized fallback parser
  - Cohere `tool-call-start / tool-call-delta / tool-call-end` stream event parsing
- Rust gateway inbound caller auth compatibility now also covers:
  - `Authorization: Bearer <key>`
  - `x-api-key: <key>`
  - `api-key: <key>`
  - operator-configured header aliases via `GATEWAY_INBOUND_API_KEY_HEADER_ALIASES`
- OpenAI-style multipart audio transcription requests are now normalized into the unified canonical pipeline and reconstructed as upstream multipart bodies at dispatch time
- Binary passthrough responses are now part of the public runtime for OpenAI-compatible audio endpoints, so `/v1/audio/speech` and non-JSON transcription formats can return upstream bytes without breaking audit/auth/quota/routing ownership

Exit criteria:

- Auto-cache only applies to Anthropic Claude requests
- No interference with requests that already have `cache_control` markers
- Cache hit rate > 60% for auto-cached requests
- Cost savings visible in user dashboard
- Monitoring shows 0% of Claude Code requests get modified

### Phase 6: Browser-Backed And Prewarm Systems

Tasks:

- bring browser-backed execution under Rust entry ownership
- ensure prewarm/cache/lease behavior is Rust-owned
- keep temporary helpers internal-only where needed

Exit criteria:

- browser-backed providers no longer require a public TS relay

### Phase 7: Cutover And Deletion

Tasks:

- switch ingress and local deployment to Rust gateway
- replace TS gateway runtime paths with Rust
- delete or quarantine obsolete TS gateway runtime code

Exit criteria:

- TypeScript gateway runtime is no longer part of the active public architecture

## Testing Strategy

### Contract Tests

- access key validation
- user credential lookup
- real credential selection
- management API request/response contracts

### Runtime Tests

- request routing parity
- cache scope correctness
- quota enforcement correctness
- provider failover correctness
- browser-backed dispatch correctness

### Product Tests

- standalone deployment works without website backend
- website/admin can call Rust gateway management APIs when present

## Acceptance Criteria

This migration is complete only when:

1. Rust gateway is the only public gateway runtime.
2. Rust gateway directly owns gateway keys, credential libraries, and management APIs.
3. Website/admin surfaces operate as Rust gateway clients rather than permanent owners.
4. Both special API keys and user API keys flow through the unified Rust request pipeline.
5. Real provider credentials are always selected from the Rust-owned real credential library.
6. TypeScript gateway runtime is eliminated from the target architecture.

## Completion Status

The gateway migration plan is closed for the current Neuro workspace baseline.
The last runtime-bearing Gateway/Platform closure commit reviewed for this
record is `971172a` (`retry aistudio auto prompt during polling`);
the final documentation commit that updates this section is verified separately
as a documentation-only follow-up.

The former immediate-next-step items have been closed in Rust-owned surfaces:

- Provider inventory, provider quota, probe, readiness, and cooling sweep
  surfaces are owned by Rust Gateway internal APIs.
- Quota pre-deduct, settlement, refund, unified access-key balance writes, and
  runtime request-audit persistence are handled on the Rust pipeline path.
- Provider health, cooldown, breaker state, quota snapshots, and selected
  candidate score metadata are persisted/read by Rust-owned runtime and
  management surfaces.
- Browser-backed ensure, lease, health, execution, and runtime management are
  owned by Rust Gateway browser-executor and keepalive APIs.
- Platform/operator web surfaces call Rust Gateway through
  `AI_GATEWAY_INTERNAL_URL`; the legacy `packages/ai-gateway-domain`
  `gatewayRouter` is retained only as migration-period legacy surface and is
  not registered as the default account-api Gateway owner path.

AIStudio replay-readiness hardening is included in this closure as provider
material hygiene, not as a blocker for Gateway ownership:

- `generationId` is replay-critical and is accepted only when the
  `CodeAssistantOffline` response id and the
  `StreamCodeAssistantOfflineGeneration` request slot `[0]` are both present
  and identical.
- Mismatched generation chains keep `capturedTargetRpcContract=true`, but set
  `generationId=null` and `replayReadyTargetRpcContract=false`.
- `appId` remains request-slot based only; it is not inferred from generation,
  prompt, or stream UUIDs.
- `Gateway/scripts/export-aistudio-storage-state.mjs` provides a safe manual
  storage-state refresh path using an isolated visible browser context. It does
  not mutate or copy a live Chrome/Edge user profile.
- `Gateway/scripts/probe-aistudio-live-request.mjs` best-effort applies the
  AIStudio `Remix ...` modal before prompting so the live probe can continue
  into owned-app capture instead of stalling at the confirmation layer.
- The live probe summary exposes target RPC diagnostics
  (`targetRpcModelPath`, `targetRpcResponseStatuses`, and `targetRpcFailure`)
  so operator-visible output can distinguish missing target RPC capture from a
  captured target RPC that was rejected upstream.
- The live probe captures page-level UI diagnostics and dismisses
  non-destructive AIStudio overlays before prompting, so permission/cookie/
  budget-control UI states are preserved in capture artifacts without clicking
  destructive budget actions.
- The live probe supports explicit browser proxy/direct-mode launch settings
  for operator-controlled AIStudio capture network routing without mutating
  browser fallback behavior.
- Browser proxy mode/server diagnostics are surfaced in probe summaries and
  failure output so operator-visible artifacts identify the network route used
  by failed AIStudio capture attempts.
- Explicit local browser proxy endpoints (`127.0.0.1`, `localhost`, `::1`) are
  TCP-preflighted before Chromium launch. If the endpoint is down, the probe
  fails early with `aistudio_browser_proxy_unreachable` and writes
  `browserProxyPreflight` diagnostics instead of letting a generic
  `ERR_PROXY_CONNECTION_FAILED` look like an AIStudio auth or permission issue.
- Probe summaries now classify Google auth recovery / account chooser final
  URLs as `authRecoveryState` and keep such runs `ok=false` unless the target
  RPC contract was actually captured, so an `ActiveTrigger`-only login redirect
  cannot be mistaken for useful AIStudio Apps evidence.
- Visible account-chooser retries are opt-in through an explicit account email,
  and auto-prompt submission now retries for late-loading prompt textareas
  before recording a diagnostic failure.
- If the prompt textarea appears only after the initial capture/polling phase,
  the live probe reattempts auto-prompt submission during polling instead of
  giving up after the first pre-loop attempt.
- Local WebSocket proxy error frames are summarized as `localProxyErrors`, so
  operator output can distinguish browser/proxy-side permission failures from
  missing target RPC capture.
- The local WebSocket capture server destroys open sockets during close to
  avoid leaving the probe stuck while flushing diagnostic capture artifacts.

Final acceptance matrix:

| Criterion | Evidence | Status |
| --- | --- | --- |
| Rust gateway is the only public gateway runtime | `Gateway/src/http/router.rs`, `Gateway/src/pipeline/mod.rs`, Gateway build/test evidence | Pass |
| Rust gateway owns gateway keys, credential libraries, and management APIs | `Gateway/src/http/routes/internal_gateway.rs`, `Gateway/src/gateway_api_key.rs`, `Gateway/src/db/provider_credentials.rs`, `Gateway/src/db/provider_accounts.rs` | Pass |
| Website/admin surfaces operate as Rust gateway clients | `Platform/web/src/lib/gateway-request.ts`, `Platform/web/src/lib/account-client.ts`, Platform smoke/typecheck evidence | Pass |
| Special API keys and user API keys flow through the unified Rust request pipeline | `Gateway/src/auth/project_api_key.rs`, `Gateway/src/pipeline/mod.rs` | Pass |
| Real provider credentials are selected from the Rust-owned real credential library | `Gateway/src/db/provider_credentials.rs`, `Gateway/src/routing/config.rs`, provider-line pass logs | Pass |
| TypeScript gateway runtime is eliminated from the target architecture | `Platform/services/account-api/src/server.ts` does not register the legacy gateway router as the default owner; `Platform/packages/ai-gateway-domain/src/modules/gateway/router.ts` is legacy migration surface only | Pass |

Verification evidence captured during the Neuro workspace migration closure:

- `python Gateway/tools/validate-gateway-line-manifests.py`
  - pass: `checked=41 errors=0`
- `python -m unittest discover -s Gateway/tests/python -p "test_*.py" -v`
  - pass: `Ran 7 tests`
- `node --test Gateway/scripts/tests/*.test.mjs`
  - pass: `53` worker tests
- `cargo fmt --manifest-path Gateway/Cargo.toml -- --check`
  - pass
- `git diff --check -- Gateway`
  - pass
- `cargo test --manifest-path Gateway/Cargo.toml --locked --no-run`
  - pass
- `cargo build --manifest-path Gateway/Cargo.toml --locked --release --bin gateway`
  - pass
- Provider line verification passed for the final gateway-provider batch:
  `xai-openai-official-vendor-api`, `freebuff-web-reverse-api`,
  `kiro-official-vendor-api`, `perplexity-chat-official-vendor-api`,
  `producer-web-reverse-api`, `xfyun-native-websocket-official-vendor-api`, and
  `xfyun-openai-official-vendor-api`.
- Platform-side validation passed:
  `npm run smoke`, `npm run typecheck -ws --if-present`, and
  `docker compose -f deploy/docker-compose.local.yml config --quiet`.

Provider-line evidence was refreshed with pass markers for:

- `xai-openai-official-vendor-api`
- `freebuff-web-reverse-api`
- `kiro-official-vendor-api`
- `perplexity-chat-official-vendor-api`
- `producer-web-reverse-api`
- `xfyun-native-websocket-official-vendor-api`
- `xfyun-openai-official-vendor-api`

Known non-blocking follow-up:

- `Gateway/src/upstream/client.rs` remains a reverse-web/browser-backed
  upstream-layer concentration point. This is maintenance debt for later
  provider-line refactoring, but it does not block the Rust Gateway ownership
  migration closure recorded here.

Maintenance progress after migration closure:

- 2026-06-06 `5b580f4` extracted request-time browser policy parsing and
  fail-closed error helpers into `Gateway/src/upstream/request_time_browser_policy.rs`.
- 2026-06-06 `c1e8c33` extracted canonical OpenAI SSE byte formatting into
  `Gateway/src/upstream/canonical_sse.rs` and moved modular AIStudio/Gemini API
  call sites to the new module.
- 2026-06-06 `eb72444` extracted `BinaryUpstreamResponse` and
  `UpstreamStreamingResponse` into `Gateway/src/upstream/response_types.rs`, so
  pipeline and modular upstream code no longer import these shared response
  carrier types from `client.rs`.
- 2026-06-06 `fe95a07` moved Producer Supabase refresh response/key material
  and image retry attempt count into the existing Producer helper modules, with
  helper-local tests for those contracts.
- 2026-06-06 `ac0c018` moved Gemini Canvas public-page API key fallbacks into
  `Gateway/src/upstream/gemini_canvas_direct_http_helpers.rs`, keeping the
  fallback candidate contract local to direct-http helper tests.
- 2026-06-06 `1ce7fa6` removed the duplicate `ProducerConversationJob` type
  from `Gateway/src/upstream/client.rs` by returning the existing
  `ProducerConversationJobData` helper type from Producer conversation sends.
- 2026-06-06 `9f03720` extracted Gemini Canvas runtime credential persistence
  payload merging into `Gateway/src/upstream/gemini_canvas_runtime_helpers.rs`;
  helper-local tests preserve material patch, `runtimeStateObjectKey`, adapter,
  and base URL fallback behavior.
- 2026-06-06 `52f70fd` moved Gemini Canvas program runtime material
  Redis/Postgres write-back orchestration into
  `Gateway/src/upstream/gemini_canvas_runtime_helpers.rs`, leaving `client.rs`
  call sites to pass pools, payload, and runtime patches.
- 2026-06-06 `a471edf` moved Producer Supabase bearer/header refresh
  orchestration into `Gateway/src/upstream/producer_session_helpers.rs`, so
  Producer image/browser/music/video send paths call a helper instead of
  carrying refresh request construction in `client.rs`.
- 2026-06-06 `19e71b7` moved Producer conversation send HTTP request
  construction into `Gateway/src/upstream/producer_media_helpers.rs`, so
  Producer direct video bootstrap/creative/confirmation paths call a helper
  instead of carrying conversation POST handling in `client.rs`.
- 2026-06-06 `7e24a7c` moved Producer message stream HTTP polling into
  `Gateway/src/upstream/producer_media_helpers.rs`, so Producer direct video
  bootstrap/creative/confirmation stream reads call a helper instead of
  carrying message-stream GET handling in `client.rs`.
- 2026-06-07 `8fa7a16` moved Producer video status HTTP polling into
  `Gateway/src/upstream/producer_media_helpers.rs`, so Producer direct video
  orchestration calls a helper instead of carrying video-status GET handling in
  `client.rs`.
- 2026-06-07 `4601353` moved Producer browser-worker process orchestration into
  `Gateway/src/upstream/producer_media_helpers.rs`, so Producer browser
  executor service, image fallback, and video fallback paths call a helper
  instead of carrying worker spawn/stdin/stdout handling in `client.rs`.
- 2026-06-07 `a5a1147` moved Producer direct video HTTP orchestration into
  `Gateway/src/upstream/producer_media_helpers.rs`, so `client.rs` now delegates
  the bootstrap/creative/confirmation/status direct-video sequence to a helper
  before deciding whether to fall back to the browser worker.
- 2026-06-07 `8e0f61e` moved Producer direct image HTTP orchestration into
  `Gateway/src/upstream/producer_media_helpers.rs`, so `client.rs` now delegates
  image request construction, retry resolution, and response parsing to a helper.
- 2026-06-07 `10de269` moved Producer direct music HTTP orchestration into
  `Gateway/src/upstream/producer_media_helpers.rs`, so `client.rs` now delegates
  send-message POST, stream polling, and music response parsing to a helper.
- 2026-06-07 `97449ab` moved the archived Gemini Canvas official JSON POST
  helper into `Gateway/src/upstream/gemini_canvas_official_api_helpers.rs`, so
  `client.rs` no longer owns that official-lane request/response parsing block.
- 2026-06-07 `aac043c` moved the archived Gemini Canvas official JSON GET
  helper into `Gateway/src/upstream/gemini_canvas_official_api_helpers.rs`, so
  `client.rs` now delegates operation polling GET responses to the helper module.
- 2026-06-07 `7268ef4` moved the archived Gemini Canvas official bytes GET
  helper into `Gateway/src/upstream/gemini_canvas_official_api_helpers.rs`, so
  `client.rs` now delegates official-lane binary download handling to the helper.
- 2026-06-07 `b8693a6` moved the archived Gemini Canvas official music websocket
  connector into `Gateway/src/upstream/gemini_canvas_official_api_helpers.rs`, so
  `client.rs` now delegates official-lane websocket request construction and
  connection setup to the helper.
- 2026-06-07 `c63a9f5` moved the archived Gemini Canvas official video
  orchestration into `Gateway/src/upstream/gemini_canvas_official_api_helpers.rs`,
  so `client.rs` now delegates official-lane predict/poll/download response
  handling to the helper.
- 2026-06-07 `48758f9` moved the archived Gemini Canvas official music
  orchestration into `Gateway/src/upstream/gemini_canvas_official_api_helpers.rs`,
  so `client.rs` now delegates official-lane websocket setup, audio streaming,
  and response assembly to the helper.
- 2026-06-07 `9837eb2` moved the Gemini Canvas direct-http page harvest
  single-request redirect/cookie loop into
  `Gateway/src/upstream/gemini_canvas_direct_http_helpers.rs`, so `client.rs`
  now delegates harvest-mode request construction, redirect tracing, and
  page-fetch error metadata to the helper.
- 2026-06-07 `b7b73a1` moved the Gemini Canvas direct-http page harvest
  multi-mode retry loop into
  `Gateway/src/upstream/gemini_canvas_direct_http_helpers.rs`, so `client.rs`
  keeps only the cloned-session wrapper while the helper owns harvest-mode
  ordering, failure aggregation, and `page_fetch_attempts` metadata.
- 2026-06-07 `d9d0824` moved the Gemini Canvas direct-http JSON POST helper
  into `Gateway/src/upstream/gemini_canvas_direct_http_helpers.rs`, so
  `client.rs` now delegates storage-state loading, signed/headered request
  construction, API-key transport handling, response metadata, and JSON parsing
  for the generateContent direct-http path.
- 2026-06-07 `0eeb8b0` moved Gemini Canvas image-edit upload start/finalize
  orchestration into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates browser re-encode probing, debug artifact
  snapshots, resumable upload start/finalize requests, and uploaded resource
  reference assembly to the image-edit helper.
- 2026-06-07 `07e2820` moved Gemini Canvas image-edit signaler HTTP request
  sending into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates signaler header construction, session cookie
  refresh, request body dispatch, challenge/session-invalid detection, and
  direct-http error classification to the image-edit helper.
- 2026-06-07 `88c2b5d` moved Gemini Canvas image-edit signaler long-poll HTTP
  request handling into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates poll header construction, session cookie refresh,
  long-poll body collection, early asset/app-path/refresh-token detection, and
  pure-http error classification to the image-edit helper.
- 2026-06-07 `d068a2d` moved Gemini Canvas image-edit signaler credential
  refresh request construction into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates refreshCreds URL/body construction and the
  underlying signaler request dispatch to the image-edit helper.
- 2026-06-07 `31edbde` moved Gemini Canvas image-edit signaler bootstrap
  material resolution into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates page/runtime account-id selection, API key
  candidate merging, and missing bootstrap diagnostic metadata to the
  image-edit helper.
- 2026-06-07 `f2ee121` moved Gemini Canvas image-edit signaler channel opening
  into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`, so
  `client.rs` now delegates runtime-state loading, page bootstrap fetch,
  locale resolution, chooseServer/openChannel request orchestration, failure
  aggregation, and channel assembly to the image-edit helper.
- 2026-06-07 `d6a0981` moved Gemini Canvas image-edit signaler prewarm
  orchestration into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates channel opening, first long-poll construction,
  optional refresh-token credential refresh, and `next_aid` handoff updates to
  the image-edit helper.
- 2026-06-07 `423ed0e` moved Gemini Canvas image-edit signaler poll-state
  preparation into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates follow-up session/channel reuse, pure-http
  session construction, channel opening, and the dead channel-opening wrapper
  removal to the image-edit helper path before entering the asset polling loop.
- 2026-06-07 `f202e88` moved Gemini Canvas image-edit signaler locator
  recording into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates response/conversation id extraction and
  follow-up context writes from poll/page/refreshed-page bodies to one helper
  instead of repeating that locator block through the asset polling loop.
- 2026-06-07 `6d8c3e7` moved Gemini Canvas image-edit signaler follow-up
  state recording into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates session/channel/locale persistence plus locator
  recording from successful poll/page/refreshed-page and handoff paths to one
  helper instead of repeating that state-write block through the polling loop.
- 2026-06-07 `15ee8f3` moved Gemini Canvas image-edit signaler app-path
  recording into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates page URL assembly, trace emission,
  distinct-path tracking, first-seen timestamp initialization, and follow-up
  context app URL writes before attempting the direct page fetch.
- 2026-06-07 `af6c97d` moved Gemini Canvas image-edit signaler handoff-ready
  threshold evaluation into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` now delegates the distinct app-path / first-seen elapsed-time
  decision before constructing the handoff-ready error and preserving follow-up
  state for downstream recovery.
- 2026-06-07 `b2b27b7` moved Gemini Canvas image-edit signaler poll URL
  construction into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so `client.rs` and the prewarm helper now share one deterministic long-poll
  URL builder while call sites still supply the current zx token.
- 2026-06-07 `ef11b8c` moved Gemini Canvas image-edit signaler asset/state
  extraction into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so the poll body, page body, and refreshed page body success paths now share
  one helper that extracts image assets and persists follow-up session/channel,
  locale, and locator state.
- 2026-06-07 `9d2ce8c` moved Gemini Canvas image-edit signaler missing-asset
  finish handling into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so the terminal polling failure path now shares the same follow-up
  session/channel/locale state writer before constructing the missing-asset
  error.
- 2026-06-07 `5085ae6` moved Gemini Canvas image-edit signaler next-AID
  updates into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so prewarm and the asset polling loop now share the same long-poll body
  parsing and channel `next_aid` update contract.
- 2026-06-07 `4c7db6a` moved Gemini Canvas image-edit signaler handoff-ready
  finish handling into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so the polling loop now delegates the ready threshold check, handoff error
  construction, trace emission, and follow-up state persistence to one helper.
- 2026-06-07 `1419a75` moved Gemini Canvas image-edit signaler refresh-token
  handling from long-poll bodies into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so prewarm and the polling loop now share one refresh-from-body helper and
  the client wrapper around refreshCreds dispatch was removed.
- 2026-06-07 `b9bbfd7` moved Gemini Canvas image-edit signaler page failure
  entry formatting into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so fetch, refetch, and page-refresh failure accumulation share one
  `page_url=... <label>=...` formatting contract.
- 2026-06-07 `2c3a558` moved Gemini Canvas image-edit signaler poll error
  entry formatting into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so long-poll transport failures share one `poll_aid=... error=...`
  formatting contract.
- 2026-06-07 `a08e27d` moved Gemini Canvas image-edit signaler refresh error
  entry formatting into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so refresh-token failure accumulation shares one
  `refresh_creds aid=... error=...` formatting contract.
- 2026-06-07 `6bdf55e` moved Gemini Canvas image-edit signaler poll body
  preview and locator recording into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so the successful long-poll branch now delegates 240-character body preview
  generation and follow-up locator capture to one helper.
- 2026-06-07 `f6b7ca2` moved Gemini Canvas image-edit signaler page asset
  response extraction into `Gateway/src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
  so page and refreshed-page bodies share one helper that preserves the owned
  response body while returning extracted assets.
- 2026-06-07 `21853ee` moved Gemini Canvas conversation-page poll failure
  entry formatting into `Gateway/src/upstream/gemini_canvas_followup_types.rs`,
  so page-refresh, fetch, and extract failure accumulation share explicit
  `attempt=...` formatting contracts outside `client.rs`.
- 2026-06-07 `d35d062` moved Gemini Canvas conversation-page poll timing
  policy into `Gateway/src/upstream/gemini_canvas_followup_types.rs`, so
  poll budget, inter-attempt sleep, and fetch timeout selection are tested as
  one helper instead of three inline `client.rs` match blocks.
- 2026-06-07 `b7149f7` moved Gemini Canvas conversation-page poll refresh
  target selection into `Gateway/src/upstream/gemini_canvas_followup_types.rs`,
  so image-only refresh eligibility and conversation/app URL fallback selection
  are tested outside `client.rs`.
- 2026-06-07 `0c92373` moved Gemini Canvas conversation-page poll body asset
  extraction into `Gateway/src/upstream/gemini_canvas_followup_types.rs`, so
  successful owned page-body returns and missing-asset preview/failure-entry
  recording are tested outside `client.rs`.
- 2026-06-07 `71f72fd` moved Gemini Canvas conversation-page poll remaining
  budget and sleep-window decisions into
  `Gateway/src/upstream/gemini_canvas_followup_types.rs`, so the one-second
  stop guard and retry sleep threshold are tested outside `client.rs`.
- 2026-06-07 `495a6b8` moved Gemini Canvas conversation-page poll refresh
  result entry formatting into `Gateway/src/upstream/gemini_canvas_followup_types.rs`,
  so refresh body previews and refresh error summaries are tested outside
  `client.rs`.
- 2026-06-07 `1328244` moved Gemini Canvas conversation-page poll fetch-error
  entry formatting into `Gateway/src/upstream/gemini_canvas_followup_types.rs`,
  so fetch failures now share the typed Gateway-error summary contract outside
  `client.rs`.
- Fresh maintenance-slice validation:
  - `cargo fmt --manifest-path Gateway/Cargo.toml -- --check`
  - `cargo test --manifest-path Gateway/Cargo.toml canonical_sse --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml request_time_browser_policy --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml response_types --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml producer_supabase_refresh_response_deserializes_access_token --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml producer_image_max_attempts_matches_retry_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_public_page_api_key_fallbacks_keep_candidate_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml send_producer_conversation_returns_shared_job_data_type --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml read_producer_message_stream_returns_string_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml fetch_producer_video_status_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml execute_producer_browser_worker_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml execute_producer_video_http_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml execute_producer_image_http_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml execute_producer_music_http_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml send_gemini_canvas_official_json_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml send_gemini_canvas_official_get_json_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml send_gemini_canvas_official_get_bytes_returns_bytes_and_content_type_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml connect_gemini_canvas_music_socket_returns_websocket_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml execute_gemini_canvas_official_video_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml execute_gemini_canvas_official_music_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml fetch_gemini_canvas_direct_http_page_html_once_returns_string_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml fetch_gemini_canvas_direct_http_page_html_refreshing_session_returns_string_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml send_gemini_canvas_direct_http_json_with_options_returns_json_value_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml upload_gemini_canvas_image_edit_inputs_with_http_returns_uploaded_refs_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml send_gemini_canvas_signaler_request_refreshing_session_with_http_returns_string_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml send_gemini_canvas_signaler_poll_request_refreshing_session_with_http_returns_string_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml refresh_gemini_canvas_image_edit_signaler_creds_with_http_returns_string_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml resolve_gemini_canvas_image_edit_signaler_bootstrap_material_prefers_page_account_and_merges_keys --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml open_gemini_canvas_image_edit_signaler_channel_with_http_returns_channel_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml prewarm_gemini_canvas_image_edit_signaler_with_http_returns_channel_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml prepare_gemini_canvas_image_edit_signaler_poll_state_with_http_returns_session_and_channel_result --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml record_gemini_canvas_image_edit_signaler_locator_from_body_preserves_locator_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml record_gemini_canvas_image_edit_signaler_followup_state_from_body_preserves_state_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml record_gemini_canvas_image_edit_signaler_app_path_preserves_app_url_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_image_edit_signaler_handoff_ready_preserves_threshold_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml build_gemini_canvas_image_edit_signaler_poll_url_preserves_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml extract_gemini_canvas_image_edit_signaler_assets_from_body_preserves_followup_state_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml record_gemini_canvas_image_edit_signaler_followup_state_preserves_existing_locale_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml finish_gemini_canvas_image_edit_signaler_missing_asset_records_state_and_error_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml update_gemini_canvas_image_edit_signaler_next_aid_from_body_preserves_max_aid_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml try_finish_gemini_canvas_image_edit_signaler_handoff_ready_records_state_and_error_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml refresh_gemini_canvas_image_edit_signaler_creds_from_body_with_http_skips_missing_token --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_image_edit_signaler_page_failure_entry_preserves_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_image_edit_signaler_poll_error_entry_preserves_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_image_edit_signaler_refresh_error_entry_preserves_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml record_gemini_canvas_image_edit_signaler_poll_body_preview_preserves_locator_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml try_extract_gemini_canvas_image_edit_signaler_assets_response_from_body_preserves_body_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_stage_entry_preserves_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_url_entry_preserves_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_timing_preserves_image_contracts --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_timing_preserves_media_contracts --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_refresh_target_preserves_image_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_refresh_target_skips_non_refresh_lanes --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_body_extract_preserves_success_body_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_body_extract_failure_records_preview_and_entry_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_remaining_budget_enforces_one_second_guard --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_sleep_decision_preserves_retry_window_guard --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_refresh_body_entry_preserves_preview_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_refresh_error_entry_preserves_error_summary_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml conversation_page_poll_fetch_error_entry_preserves_error_summary_contract --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_runtime_persistence_payload --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_runtime --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_direct_http_helpers --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_image_edit_local_helpers --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml gemini_canvas_followup_types --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml producer_session_helpers --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml producer_media_helpers --lib`
  - `cargo test --manifest-path Gateway/Cargo.toml --locked --no-run`
  - `cargo test --manifest-path Gateway/Cargo.toml --locked --no-run -j 1`
- Remaining debt is still material: after these slices,
  `Gateway/src/upstream/client.rs` is still about `14,966` lines, so further
  extractions should continue in small TDD-backed rounds rather than a broad
  rewrite.
