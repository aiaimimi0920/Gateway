# Neuro Gateway — Architecture Specification

## Overview

Neuro Gateway is the long-term AI gateway product for NeuroLoom, written in Rust.

It is not just a thin relay. The current codebase already treats it as the formal owner of:

- public inference entrypoints
- unified external `api_key` validation
- explicit access-table and bundle authorization
- prepaid balance validation and pre-deduction
- provider quota snapshots and quota-aware credential selection
- route candidate construction and sticky affinity
- real provider credential exchange
- same-protocol fast path plus cross-protocol bridge
- request audit, prompt-cache metrics, anomaly governance, and operator management APIs

The website and account services may remain TypeScript clients, but they are clients of Rust `gateway`, not the permanent owner of gateway truth.

**Design goals**:

- predictable low overhead on the public hot path
- multi-protocol compatibility without client-side rewrites
- explicit authorization and prepaid billing instead of runtime guesswork
- browser-backed and session-backed provider support inside the same gateway product
- single-binary deployment for the default runtime

## System Boundary

```
Platform Clients / Operator UI         Neuro Gateway (Rust)                      Upstream Providers
┌──────────────────────────────┐       ┌──────────────────────────────────┐       ┌──────────────────┐
│ web / account-api / scripts  │ HTTPS │ Public APIs + Internal Mgmt APIs │ HTTP  │ OpenAI           │
│ operator pages               │──────>│ Auth + Access Tables + Routing   │──────>│ Anthropic        │
│ account-domain benefit flows │       │ Same-protocol / Canonical bridge │       │ Gemini / Vertex  │
└──────────────────────────────┘       │ Cache + Audit + Metrics           │       │ Bedrock / Cohere │
                                       │ Browser-backed execution          │       │ Search / Media   │
                                       └──────────────────────────────────┘       └──────────────────┘
                                                │                  │
                                                │                  │
                                                ▼                  ▼
                                       PostgreSQL / Valkey / Object Storage
```

**Gateway owns**:

- public `/v1/*` runtime entrypoints
- internal management APIs under `/v1/internal/gateway/*`
- unified `access_keys` and access-table truth consumption
- user eligibility and prepaid balance enforcement on the public hot path
- provider payload/runtime material loading
- route policy application, retry, fallback, cooldown, and sticky affinity
- response cache, prompt-cache accounting, request audit, anomaly/remediation reads and writes
- browser-backed execution orchestration and local browser worker/runtime management

**Platform clients own**:

- account-center presentation
- operator UI rendering
- benefit-center and back-office workflows
- generic business features unrelated to gateway runtime

**Communication model**:

- Rust `gateway` directly reads and writes `PostgreSQL`, `Valkey`, and object storage
- web/account services call Rust gateway management APIs over private HTTP where possible
- Redis/Valkey is an acceleration layer and runtime state layer, not the only source of truth

## Tech Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Zero GC, predictable latency, memory safety |
| HTTP Server | axum 0.7 + tower | Tokio-native, zero-copy extractors, middleware composable |
| Async Runtime | tokio | Industry standard, mature, full-featured |
| HTTP Client | wreq 0.16.1 | URI-based API, Chrome TLS profiles and Gateway-owned environment/Windows proxy compatibility policy |
| PostgreSQL | sqlx | Gateway truth store for access keys, audits, policies, operator reads |
| Redis / Valkey | deadpool-redis | Runtime cache, sticky affinity, pre-deduct state, warm indexes |
| Object Storage | S3-compatible abstraction | Browser state bundles, artifacts, large runtime material |
| Concurrency | dashmap + parking_lot | Lock-free maps, non-poisoning mutexes |

Default HTTP construction and proxy compatibility are documented in
[HTTP client proxy policy](docs/http-client-proxy-policy.md).

## 5-Stage Pipeline

Every request flows through five sequential stages:

```
Stage 1: Auth      → Validate unified external key (`access_keys`)
Stage 2: Filter    → Content filter chain + prepaid balance pre-deduction
Stage 3: Route     → Load precompiled allowed access rows + sticky-first candidate selection + provider quota filtering
Stage 4: Send      → Exchange real provider credential → upstream call (retry + fallback loop)
Stage 5: Finalize  → Usage/audit/cache write-back + settle/refund
```

Each stage is a plain async function, not a trait object. The `PipelineContext` carries state through stages.

## Provider Integration Model

### Two-tier configuration

| Tier | What | Stored where | Changes |
|------|------|--------------|---------|
| **Preset** | Invariant protocol config (adapter, forced headers, extra_body) | Code (built-in) or Redis | Rarely |
| **Account** | Variable per-site config (base_url, api_key, account headers) | Redis (pushed by platform) | Per credential |

`compile_provider_account(preset, account)` merges both at load time → zero per-request overhead.

### Adding a new provider

**If OpenAI/Anthropic compatible**: Create a preset (data only), no code change.

```json
{ "id": "codex", "adapter": "openai_compatible",
  "headers": { "User-Agent": "codex_cli_rs/...", "Originator": "codex_cli_rs" },
  "extra_body": { "store": false }, "default_model": "gpt-5.4" }
```

**If needs protocol-level logic** (e.g., Mistral tool_call_id rewriting): Add a ProtocolHook.

**If entirely new wire format**: Add a new protocol adapter in `protocol/`.

Current non-OpenAI adapters already in tree:

- `accio_compatible` — body-auth + SSE wrapper translation
- `grok_compatible` — cookie-auth + NDJSON translation back to OpenAI SSE
- `gemini_business_compatible` — image-native Google Gemini Business widget/reverse surface
- `chataibot_compatible` — image-native ChatAIBot reverse surface
- `lumalabs_compatible` — reverse-web LumaLabs image/video/audio generation surface
- `gemini_canvas_compatible` — Gemini Canvas Web reverse-web direct HTTP text/TTS/media surface
- `producer_compatible` — Producer.ai reverse-web image/audio/video surface using session JWT auth plus browser-backed video execution where needed
- `suno_compatible` — reverse-web Suno image/audio/video generation surface
- `udio_compatible` — cookie/session-state auth + browser worker that executes `POST /api/generate-proxy` and `GET /api/songs` inside a real Chromium context
- `perplexity` preset — OpenAI-compatible routing to Perplexity chat (`/chat/completions`) and responses (`/v1/responses`)
- `perplexity-search` preset — Perplexity Search passthrough (`POST /search`, request field `query`)

`accio_compatible` now underpins the first-class `accio` provider profile:

- `service_provider_key = accio_platform`
- `protocol_profile = accio`
- canonical outbound family = `openai_responses`
- `source_kind = web_reverse_api`
- `web_reverse_access_mode = direct_http_replay`
- runtime truth stays in single-row `gateway_provider_credentials`; `accio-manager` account JSON is only an import source

Current `phoenix-gw` contract is split into runtime-send and control-plane probes:

- runtime send path: `/api/adk/llm/generateContent`
- runtime auth token stays in the JSON body as `token`
- runtime browser/app headers use un-prefixed names: `utdid`, `version`, optional legacy `appKey`
- runtime default headers also include `Accept: text/event-stream`, `accept-language: *`, `sec-fetch-mode: cors`, `user-agent: node`
- control-plane catalog/quota probes prefer `/api/llm/config` and `/api/llm/config/v2`, and use prefixed headers `x-utdid`, `x-app-version`, `x-cna`
- message packing supports `tools`, `thinking`, `conversation_id`, `conversation_name`, and `session_key`
- caller-visible ingress may arrive as `OpenAI chat/completions/responses`, `Anthropic Messages`, or `Gemini generateContent`, then bridge onto the Accio responses-style upstream family
- response parsing accepts wrapped Anthropic-style SSE events plus direct `candidates[]` / `choices[]` fragments so Accio can proxy both text and tool calls without a provider-specific sidecar
- upstream `error_code=5015` / `user not activated` is treated as an explicit Accio-account-unavailable signal: the current candidate is marked unusable and the gateway may fall through to the next candidate instead of surfacing it as a generic stream success
- dedicated vendor-live verification currently archives `accio_live` plus the complete `accio_full_live` HTTP regression
- `search_api_compatible` — execution-layer JSON passthrough adapter for Linkup/Tavily/Perplexity Search/You.com/Exa/Jina/WebSearchAPI-style providers, with either bearer auth or a provider-specific API-key header

`Gemini Platform` is now frozen as one provider identity with three distinct surface classes:

- `google_gemini_api`
  - `source_kind = official_model_api`
  - adapter = `gemini_api_compatible`
  - outbound families = `gemini_generate_content` plus `gemini_live`
- `google_vertex_gemini`
  - `source_kind = official_model_api`
  - adapter = `gemini_api_compatible`
  - outbound families = `gemini_generate_content` plus `gemini_live`
- `gemini_business`
  - `source_kind = official_vendor_api`
  - adapter = `gemini_business_compatible`
  - outbound family = `gemini_business_images`
- `gemini_web`
  - `source_kind = web_reverse_api`
  - reverse-web protocol replay against the generic Gemini Web product surface
  - adapter = `gemini_web_compatible`
  - outbound family = `gemini_web_chat`
- `gemini_canvas`
  - `source_kind = web_reverse_api`
  - reverse-web execution against the Gemini Canvas Web product surface
  - direct HTTP execution against the Canvas reverse-web contract
  - adapter = `gemini_canvas_compatible`
  - outbound families = `gemini_canvas_images / gemini_canvas_music / gemini_canvas_videos`
  - current text/TTS/runtime truth stays on the Canvas reverse-web direct HTTP lane rather than the official Gemini API lane

Dedicated archived suites currently pin that split:

- Google Gemini API full fixture = `45 / 45`
- Google Vertex Gemini full fixture = `42 / 42`
- Gemini Business Images fixture = `4 / 4`
- Gemini Canvas text/TTS fixture = `43 / 43`
- Gemini Canvas media fixture = `5 / 5`

### Search providers in the unified scheduler

Linkup/Tavily/Perplexity Search/You/Exa/Jina/WebSearchAPI-style JSON APIs still go through the same provider-account and credential-pool scheduler as chat providers. The gateway exposes dedicated passthrough entrypoints, keeps the public ingress family as `search`, and resolves explicit outbound families such as `linkup_search`, `perplexity_search`, `tavily_search`, `exa_search`, `jina_search`, `jina_reader`, `you_search`, and `websearchapi_search`.

The shared public route-model defaults are `search-search`, `search-fetch`, `search-research`, and `search-balance`, with legacy `search-api-*` and `linkup-*` aliases preserved for existing route config. This lets search providers stay on the same scheduler path without flattening every vendor into one fake outbound family.

That means:

- `supported_models`, aliases, and `model_routes` remain the single routing surface
- multi-key search accounts reuse the existing `credentials:` round-robin pool format
- provider-specific JSON bodies and GET endpoints can stay non-chat-native without infecting the chat adapters
- adapter reuse does not hide the concrete outbound family or provider profile anymore

Jina uses the same Search Foundation bearer API key for both `s.jina.ai` search
and `r.jina.ai` reader fetches, but those capabilities live on different
hosts. The current baseline keeps them as two providers (`jina-search`,
`jina-reader`) that share the same credential env vars while still compiling
through the same `search_api_compatible` adapter. Jina is also a text-first
upstream by default, so the built-in presets force `Accept: application/json`
to keep the gateway on the shared JSON passthrough hot path while still
resolving `jina_search` and `jina_reader` as distinct outbound families.

For Linkup, the Rust gateway also exposes an operator-only balance snapshot:

- `GET /v1/internal/providers/linkup/credits/balance`

This route requires the gateway bearer token and returns both the aggregate balance across all configured Linkup credentials and a per-credential breakdown with raw upstream payloads / failure reasons.

Legacy compatibility deployments may still mirror the historical Linkup-style route family on `/v1/*` and `/v1/new-api/*`, but the default public/runtime owner is now Rust `gateway`, which carries the canonical auth, route resolution, request audit, guardrail, and provider concurrency path.

### Modality families in the unified scheduler

The same explicit family/profile/adapter model that now governs conversation/live/search also governs the non-dialog modality endpoints. Caller-visible OpenAI-family modality ingress is now explicit instead of being treated as a generic extension of `openai_chat`:

- `openai_embeddings`
- `openai_audio_transcriptions`
- `openai_audio_speech`
- `openai_images_generations`
- `openai_images_edits`
- `openai_music_generations`
- `openai_videos_generations`

Those are public ingress/reply families. They do not imply that the selected upstream provider speaks OpenAI on the wire. The gateway now records the selected outbound target separately as `selectedUpstreamTargetProtocolFamily`.

Native outbound media providers are also explicit families now:

- `gemini_business_images`
- `chataibot_images`
- `lumalabs_images`
- `lumalabs_audio`
- `lumalabs_videos`
- `gemini_canvas_images`
- `gemini_canvas_music`
- `gemini_canvas_videos`
- `producer_images`
- `producer_music`
- `producer_videos`
- `suno_images`
- `suno_music`
- `suno_videos`
- `udio_images`
- `udio_music`
- `udio_videos`

This keeps the scheduler honest about two different facts:

- what protocol family the caller used to enter the gateway
- what protocol family the selected `credential + model` actually supports upstream

The same-provider multi-family rule still applies. One provider identity may expose multiple protocol families, and one credential may expose different families per model. Route selection therefore resolves outbound protocol family at `credential + model` granularity and prefers same-family direct routing whenever the selected `credential + model` supports the ingress family.

For Gemini specifically, the scheduler now distinguishes three separate kinds of surfaces that share `service_provider_key = gemini_platform`:

- official model API surfaces (`google_gemini_api`, `google_vertex_gemini`)
- official vendor image surface (`gemini_business`)
- direct_http web-reverse surfaces (`gemini_canvas`)

They intentionally do not collapse into one fake `gemini` adapter or one fake outbound family.

### Session-backed hosted credentials

Session-backed providers still compile into the same `ProviderAccountPayload` / `RouteCandidate` path as ordinary hosted credentials. The only extra runtime metadata is:

- `session_auth` — how to render the current session token on the wire
- `keepalive` — how to call the external keepalive steward before sending
- `credential_id` — optional Redis-backed credential identifier for write-back

This keeps Grok/browser-gated providers inside the normal candidate queue instead of introducing a second request path.

Formal layered terminology:

- `seed credential material`
  - long-lived or recoverable provider-account material stored in the control plane / credential pool
- `runtime session material`
  - current request-time auth/state material carried through `api_key`, `headers`, `extra_body`, `session_auth`, `expires_at`, and optional `runtime_state_object_key`
- `browser-backed execution capability`
  - warmed browser/runtime capacity leased by a worker or executor, not a credential entry

Current field semantics:

- `session_auth` only describes how to render the current `runtime session material` on the wire
- `keepalive` only describes how to ensure / refresh that runtime material before send
- `runtime_state_object_key` is a handle to a browser/runtime state bundle; it may be a recoverable seed and/or a write-back target, but it is not itself a warmed browser context

This means browser-gated providers still use the same candidate queue, but the real send path may switch from `direct_http` to `browser_backed` execution after routing.

Current implementation status:

- provider configuration now carries an explicit `executionMode`
- mixed providers may override that mode by endpoint kind through `endpointExecutionModes`
- browser executor warm-pool / lease / health state now belongs to the Rust `gateway` runtime data plane by default
- Rust `gateway` now executes `browser_backed` candidates with local browser workers / pools by default and writes executor metadata into request-audit route traces
- optional remote browser-executor override remains available only when explicitly configured:
  - `GET /v1/internal/browser-executor/health`
  - `POST /v1/internal/browser-executor/execute`
  - these routes now live on Rust `gateway` itself for the default local-worker path
  - runtime management also now lives on Rust `gateway` itself:
    - `POST /v1/internal/gateway/browser-executor/nodes/heartbeat`
    - `GET /v1/internal/gateway/browser-executor/nodes`
    - `GET/POST /v1/internal/gateway/browser-executor/slots`
    - `GET /v1/internal/gateway/browser-executor/leases`
    - `POST /v1/internal/gateway/browser-executor/leases/acquire`
    - `POST /v1/internal/gateway/browser-executor/leases/{lease_id}/release`
    - `GET /v1/internal/gateway/browser-executor/health`
  - default hot paths are fully satisfied by Rust `gateway`; no separate TypeScript relay is required for browser-backed execution
  - `browserExecutionStatus`
  - `executorNodeId`
  - `executorSlotId`
  - `executorLeaseId`
  - `executorLeaseIssuedAt / executorLeaseExpiresAt / executorLeaseReleasedAt`
  - `executorLeaseReleaseReason`
- control-plane provider accounts remain a single object; seed/runtime material is not split into separate account entities

Current transport modes inside `session_auth`:

- `cookie` — render the token into `primaryCookieName` / `secondaryCookieName`
- `bearer` — render the token into `Authorization: Bearer <token>`
- `header` — render the token into a custom `headerName`

Producer.ai currently uses the `bearer` path. The upstream auth material is a
Supabase-backed session JWT, not a long-lived vendor-issued API key.

Udio currently uses the `cookie` path plus optional `runtimeStateObjectKey`. The gateway treats the browser session cookie (`sb-ssr-production-auth-token`) as the credential's runtime auth material, but the real generation path is now browser-backed: the Rust hot path launches a Playwright worker, restores cookie/storage state, performs the real `POST /api/generate-proxy` and `GET /api/songs` requests inside the page context, and snapshots the refreshed browser state back to `runtimeStateObjectKey` when configured. The older `sb-api-auth-token` naming seen in legacy wrappers is no longer the primary production cookie name. Because Udio now stores this as a Supabase SSR session, the gateway must render oversized values using the same chunking scheme as `@supabase/ssr` (`sb-ssr-production-auth-token.0`, `.1`, ...), not a single unchunked cookie.

As of **2026-04-11** live verification, the real Udio browser path also requires:

- invisible `hCaptcha` execution before `POST /api/generate-proxy`
- the canonical request shape `{"gen_params": {...}, "captchaToken": "..."}` rather than the older thin `{prompt: ...}` body
- polling through `GET /api/songs?songIds=...&readOnly=false&checkIfReadyToStream=true`
- a bearer auth token on song polling, recovered from the Supabase SSR session carried by the browser state
- treating `readyToStream = true` as the practical readiness signal even before `finished = true`

`runtimeStateObjectKey` write-back is now guarded: if a browser run falls back to an anonymous state and loses `sb-ssr-production-auth-token.*`, the worker must not overwrite an existing authenticated runtime snapshot with that degraded state.

For Udio, the browser worker is the formal send path, not keepalive:

- keepalive continues to own only session freshness / runtime material handoff
- the real provider send still belongs to the gateway hot path
- when Vercel / browser security challenge appears, the worker keeps the browser context open and retries until the challenge clears or the request times out
- any OCR / captcha-solving integration must stay an external helper operating against that browser context; it must not turn keepalive into a second send plane
- the worker runtime is configured through `UDIO_BROWSER_NODE_BIN`, `UDIO_BROWSER_EXECUTABLE_PATH`, `UDIO_BROWSER_HEADLESS`, and `UDIO_BROWSER_CHALLENGE_RETRY_INTERVAL_MS`

The same rule now applies to Producer music-video and other browser-backed providers:

- browser prewarm / warm-pool maintenance belongs to the executor/browser-runtime layer
- background prewarm should not traverse the public gateway hot path
- real user requests still traverse the gateway, which selects the credential, ensures runtime material, and then dispatches to the browser-backed execution capability

### Hook system (code-driven quirks only)

```rust
trait ProtocolHook: Send + Sync {
    fn after_pack(&self, ctx: &HookContext, body: &mut Value) {}
    fn before_send(&self, ctx: &HookContext, headers: &mut HashMap<String, String>) {}
}
```

Reserved for logic that cannot be expressed as data (regex rewrites, conditional field injection, etc.).

## Concurrency Control — AIMD

TCP-congestion-control-inspired adaptive rate limiting per provider:

- **Additive Increase**: +1 capacity after N consecutive successes
- **Multiplicative Decrease**: ×0.7 on failure, ×0.5 on 429 rate limit
- **Withheld Permits**: When decreasing, permits are held in a pool (not semaphore reduced) to avoid disrupting in-flight requests
- **Per-provider isolation**: Each provider has its own `AimdController` in the `ConcurrencyRegistry`

## Error Taxonomy

Every upstream error is classified into `GatewayError` with a `FallbackHint`:

| Hint | Meaning | Action |
|------|---------|--------|
| `Retry { delay_ms }` | Transient, same provider | Wait and retry |
| `FallbackProvider` | Provider down | Try next candidate |
| `DowngradeModel` | Context too long | Try smaller model |
| `Abort` | Permanent error | Return error to client |

## Runtime Key Families

The current gateway no longer uses Redis as the only truth layer, so this section describes runtime key families instead of pretending a small static key list is the architecture.

Representative families include:

- access projection cache
  - `access_key -> allowed platform access rows`
- access balance cache
  - hot balance state for `time_pass`, `token_prepaid`, `message_prepaid`
- sticky affinity cache
  - `requesting_access_key + model (+ optional session scope) -> access combination`
- response cache
  - endpoint-aware cached response bodies and metadata
- provider payload/runtime material cache
  - compiled provider account payloads, session/runtime metadata
- browser executor runtime keys
  - node, slot, lease, and local execution health state
- usage/audit queues
  - runtime usage reports and async operator/analysis aggregation feeds

Exact key builders now live in the Rust code under:

- `gateway/src/redis/keys.rs`
- `gateway/src/db/access.rs`
- `gateway/src/redis/usage_tracking.rs`

Those files are the current implementation reference when precise key names matter.

## HTTP API

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/chat/completions` | POST | OpenAI chat completions (stream + non-stream) |
| `/v1/completions` | POST | Legacy OpenAI completions compatibility |
| `/v1/responses` | POST | OpenAI Responses API (stream + non-stream) |
| `/v1/messages` | POST | Anthropic messages (stream + non-stream) |
| `/v1/embeddings` | POST | OpenAI-compatible embeddings |
| `/v1/audio/transcriptions` | POST | OpenAI-compatible multipart audio transcription |
| `/v1/audio/speech` | POST | OpenAI-compatible binary audio speech |
| `/v1/models` | GET | Public model catalog resolved from gateway routes and project access |
| `/v1/images/generations` | POST | Image generation relay |
| `/v1/images/edits` | POST | Image edit relay where supported |
| `/v1/music/generations` | POST | Session-backed music generation relay for Producer.ai / Suno / Udio (non-streaming JSON) |
| `/v1/videos/generations` | POST | Producer.ai music-video generation or Gemini Canvas video relay (non-streaming JSON) |
| `/v1/search` | POST | Search relay for Linkup/Tavily/You/Exa-style JSON providers (non-streaming JSON) |
| `/v1/fetch` | POST | Fetch/extract relay for Linkup/Exa-style JSON providers |
| `/v1/research` | POST / GET | Linkup-compatible research create/list relay |
| `/v1/research/{id}` | GET | Linkup-compatible research detail relay |
| `/v1/credits/balance` | GET | Linkup-compatible balance relay |
| `/v1/internal/providers/linkup/credits/balance` | GET | Operator aggregate + per-key Linkup balance snapshot |
| `/healthz` | GET | Liveness probe |
| `/readyz` | GET | Readiness probe (checks gateway dependencies and runtime state) |

Internal keepalive/service-contract endpoints are now exposed by Rust `gateway` by default:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v1/internal/credentials/ensure` | POST | Session keepalive / runtime material ensure |
| `/v1/internal/browser-executor/health` | GET | Local browser-worker / pool runtime health |
| `/v1/internal/browser-executor/execute` | POST | Execute a provider-specific local browser worker invocation |
| `/v1/internal/gateway/browser-executor/nodes/heartbeat` | POST | Heartbeat a Rust-owned browser-executor node |
| `/v1/internal/gateway/browser-executor/nodes` | GET | List Rust-owned browser-executor nodes |
| `/v1/internal/gateway/browser-executor/slots` | GET / POST | List or upsert Rust-owned browser capability slots |
| `/v1/internal/gateway/browser-executor/leases` | GET | List Rust-owned browser capability leases |
| `/v1/internal/gateway/browser-executor/leases/acquire` | POST | Acquire a Rust-owned browser capability lease |
| `/v1/internal/gateway/browser-executor/leases/{lease_id}/release` | POST | Release a Rust-owned browser capability lease |
| `/v1/internal/gateway/browser-executor/health` | GET | Aggregate Rust-owned browser-executor runtime health |

The legacy TypeScript gateway has been removed from the repository; browser-executor routes now belong to Rust `gateway`.

## Streaming Architecture

```
Client ← SSE ← Gateway ← SSE ← Upstream Provider
         ↑
    TrackedStream wrapper:
    - Records TTFT on first chunk
    - Counts chunks + bytes
    - Releases AIMD permit on stream end/error/drop
    - Feeds SlidingWindowMetrics
```

`TrackedStream` implements `Drop` to guarantee permit release even if the client disconnects mid-stream.

## Platform-Injected Credential Routing

The platform may inject two gateway-only headers on relay requests:

- `X-Neuro-User` — authoritative end-user ID for hosted credential isolation
- `X-Neuro-Cred-Source` — `platform` or `hosted`

The HTTP route handlers copy these headers into `PipelineContext::request_headers`, `stage_auth` resolves them into `credential_source` / `neuro_user_id`, and `stage_route` uses that state to choose between the shared platform pool and user-hosted credentials. A trusted `credential_ref` resolved by the auth layer is also forwarded into routing so an authenticated session can pin a specific Redis credential without relying on a raw client header.

When `credential_ref` is explicitly present, routing now treats it as strict intent:

- only that credential is eligible
- the request must not silently fall back to YAML or the generic platform pool
- unsupported-model / missing-runtime-field cases fail fast

## Keepalive Steward Boundary

Some hosted credentials are session-backed instead of long-lived API-key-backed. For those providers, the gateway may call an external keepalive steward before dispatching the real upstream request.

The keepalive steward is **not** the request sender. Its only job is to:

- validate session freshness
- refresh session material when expiry is near
- return updated runtime auth fields (`api_key`, `headers`, `extra_body`, `session_auth`, `expires_at`)

The gateway then keeps ownership of the real upstream send path, retry loop, fallback loop, and AIMD concurrency control.

For browser-backed providers, this also means:

- keepalive does not own browser prewarm
- keepalive does not own worker lease / browser capacity
- keepalive does not replace the formal browser-backed send path

Background prewarm / cooling / recycle is an internal executor concern, not a public gateway API flow.

Current default steward implementation lives in Rust `gateway`, exposed as:

- `POST /v1/credentials/ensure`
- `POST /v1/internal/credentials/ensure`

The keepalive preflight request includes the currently selected runtime material plus gateway session context:

- `projectId`
- `sessionKey`
- `previousResponseId`
- `credentialId`
- `providerAccountId`
- `adapter`
- `baseUrl`
- `model`
- `apiKey`
- `headers`
- `extraBody`
- `sessionAuth`
- `expiresAt`

The steward may respond with refreshed runtime material:

- `apiKey`
- `headers`
- `extraBody`
- `sessionAuth`
- `keepalive`
- `expiresAt`
- `runtimeStateObjectKey`
- `upstreamSessionId`

When a `credential_id` is present, the gateway performs a best-effort runtime write-back so later requests see refreshed session metadata without waiting for the next manual operator action.

## Credential Refresh Invalidation

OAuth/session refresh writes renewed runtime material back into the gateway-owned runtime cache path, and the relevant local caches are invalidated so later requests pick up refreshed auth material without waiting for TTL expiry.

This still applies to bearer-style browser-gated providers such as the `Qwen WebUI` reverse surface: token refresh updates both the access token and the shared session runtime metadata (`session_auth.expires_at`, optional keepalive state) so the keepalive preflight and route candidates stay aligned. `Qwen DashScope / Coding Plan` API-key surfaces are no longer modeled as OAuth/session-refresh mainlines.

## Metrics

- **SlidingWindowMetrics**: Per-provider in-memory ring buffer (200 entries). Tracks latency, success rate, TTFT, p50/p95/p99.
- **Multi-Key Rotation**: Per-key failure tracking with automatic disable/cooldown/re-enable.
- **Balance shouldDeprioritize**: Provider-specific threshold signal fed into routing weight.

## Deployment

```bash
# Single binary, ~6MB
GATEWAY_REDIS_URL=redis://gateway-redis:6379 \
RUST_LOG=info \
./gateway

# Environment variables
GATEWAY_REDIS_URL          # Required
PORT                       # Default: 4200
GATEWAY_UPSTREAM_TIMEOUT_SECS  # Default: 120
GATEWAY_BROWSER_EXECUTOR_BASE_URL   # Optional remote browser-executor override base URL
GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN  # Optional bearer for internal executor calls
```

## Testing

- `cargo test` — 316 unit tests
- Integration tests (marked `#[ignore]`) require live Redis
- Stress testing: wrk/k6 with 1000+ concurrent SSE connections
