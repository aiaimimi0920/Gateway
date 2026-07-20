# Neuro Gateway — Design Rules

Rules that govern all design and implementation decisions for the gateway.

## 1. Zero per-request allocation overhead for static config

Provider configuration (headers, extra_body, adapter settings) MUST be compiled once at load time via the Preset system. The hot path (per-request processing) MUST NOT perform any config merging, template rendering, or HashMap cloning.

**Do**: `compile_provider_account()` at startup → cache `ProviderAccountPayload` → reuse.
**Don't**: Merge preset + account on every request.

## 2. Data-driven vs Code-driven provider differences

| Category | Solution | Example |
|----------|----------|---------|
| Static headers | `ProviderAccountPayload.headers` (data) | Codex `User-Agent`, `Originator` |
| Forced body fields | `ProviderAccountPayload.extra_body` (data) | Codex `store: false` |
| Conditional logic | ProtocolHook (code) | Mistral tool_call_id rewriting |
| New wire format | Protocol adapter (code) | Gemini multimodal format |

**Rule**: If a provider difference can be expressed as key-value data, it MUST be data, not code. Hooks are reserved for logic that requires conditional branching, regex, or stateful transformation.

## 3. Gateway is stateless — Redis is the state boundary

The gateway binary holds no durable state. All shared state lives in Redis:
- Credentials: pushed by platform, read by gateway
- Quotas: atomic counters (DECRBY/INCRBY)
- Usage reports: queue (RPUSH/LPOP)
- Response cache: TTL-managed entries

**Consequence**: Gateway instances are interchangeable. Scale by adding instances behind a load balancer.

## 4. Fail fast, fallback smart

Error handling follows the FallbackHint taxonomy:
- `Retry`: Same provider, exponential backoff (max 2 retries)
- `FallbackProvider`: Next candidate in queue
- `DowngradeModel`: Smaller model (future)
- `Abort`: Return error to client immediately

**Rule**: Never retry an `Abort` error. Never fallback on a `Retry` error (unless retries exhausted). The error classification drives the decision, not ad-hoc if/else chains.

## 5. Streaming is first-class, not an afterthought

- `TrackedStream` wraps every streaming response
- TTFT is measured at first chunk delivery, not at HTTP response start
- AIMD permits are released via `Drop`, guaranteeing release even on client disconnect
- SSE frames are forwarded as-is when protocol families match (zero re-serialization)

## 6. One permit per upstream call

Every upstream HTTP request MUST acquire an AIMD concurrency permit before sending. The permit MUST be released exactly once:
- On stream completion (success)
- On stream/request error (failure or rate_limited)
- On `Drop` (client disconnect)

The `ConcurrencyRegistry` maintains per-provider controllers. Never share permits across providers.

## 7. Preset inheritance model

```
ProviderPreset (invariant, shared across accounts)
    ├── adapter: "openai_compatible"
    ├── headers: { "User-Agent": "...", "Originator": "..." }
    ├── extra_body: { "store": false }
    └── default_model: "gpt-5.4"

AccountOverrides (variable, per-site/per-credential)
    ├── base_url: "https://..."
    ├── api_key: "..."
    └── headers: { "Chatgpt-Account-Id": "..." }  ← merged ON TOP

compile_provider_account(preset, account) → ProviderAccountPayload
    └── Executed ONCE at load time, result cached
```

**Merge rules**:
- `headers`: preset as base, account overrides on conflict
- `extra_body`: preset as base, account overrides on conflict
- Optional fields (`default_model`, `auth_mode`, etc.): account value if present, else preset value

## 8. Content filtering is optional and configurable

The filter chain is opt-in via configuration. When disabled, the filter stage is a no-op (zero overhead). Filters are pure synchronous functions — no async I/O, no Redis calls.

## 9. Response caching: non-streaming only

Cached responses are keyed by `SHA-256(model + messages + temperature + max_tokens + tools)`. Only non-streaming, successful responses are cached. Streaming responses are never cached (too complex, marginal benefit).

## 10. Module dependency direction

```
http → pipeline → {auth, filter, routing, upstream}
                         ↓              ↓
                       redis          protocol
                         ↓
                    concurrency, metrics, balance

Leaf modules (no internal dependencies):
  config, error, retry, preset, util
```

No circular dependencies. Leaf modules can be tested in isolation.

## 11. Testing discipline

- Every module has `#[cfg(test)] mod tests`
- Pure functions: full unit test coverage
- Redis-dependent tests: `#[ignore]` tagged, run separately with live Redis
- Network-dependent tests: `#[ignore]` tagged
- No test mocking frameworks — use dependency injection via function arguments

## 12. Adding a new upstream provider (checklist)

1. Check if an existing preset covers it (OpenAI-compatible? Anthropic-compatible?)
2. If yes: create a preset entry + account overrides. **Done. No code.**
3. If needs protocol hooks: implement `ProtocolHook`, register in `resolve_hooks_for_provider()`
4. If needs new wire format: add `protocol/{name}.rs` with `pack_*` / `unpack_*` / `normalize_*`
5. Add the adapter string to `build_request_plan()` in `upstream/client.rs`
6. Add tests
