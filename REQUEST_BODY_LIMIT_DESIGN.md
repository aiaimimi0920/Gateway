# Request Body Limit Design and Implementation Guide

## Purpose

This document defines the design rationale, implementation strategy, and operational guidelines for HTTP request body size limits in the Rust gateway.

## Executive Summary

**Decision**: Implement a 50 MB default request body limit with per-endpoint customization.

**Rationale**:
- Support vision models with base64-encoded images (5-10 MB)
- Support long-context conversations (Claude 3.5 200K tokens = 10-20 MB)
- Support complex tool definitions (5-10 MB)
- Prevent resource exhaustion attacks (DoS)
- Maintain compatibility with industry standards (ds2api, OpenAI proxies)

**Impact**:
- Security: Prevents memory exhaustion attacks
- Compatibility: Enables vision models and long-context use cases
- Performance: Minimal overhead (checked at middleware layer)

## Implementation Status

As of 2026-06-09, the Rust gateway has implemented:

- Phase 1: global request body limit
- Phase 2: per-endpoint request body limits
- OpenAI-compatible `413 Payload Too Large` JSON error code: `request_too_large`

Still pending:

- Phase 3: monitoring and alerting
- Phase 4: wider user/operator documentation updates

---

## Current Code Anchors

- `Gateway/src/config.rs`: parses `GATEWAY_MAX_REQUEST_BODY_BYTES` plus per-endpoint body limit overrides.
- `Gateway/src/http/middleware.rs`: builds the `RequestBodyLimitLayer` used by route groups.
- `Gateway/src/http/router.rs`: applies per-endpoint body limits before handlers parse request bodies.
- `Gateway/src/http/extractors.rs`: maps body-too-large extraction failures to JSON `413` responses with error code `request_too_large`.
- `Gateway/tests/smoke.rs`: covers oversized request rejection and verifies search uses a smaller limit than chat.

---

## Problem Statement

### Current State

The current Rust gateway enforces explicit global and per-endpoint request body size limits. Before this implementation existed, an unlimited request body path created several risks that this design intentionally mitigates:

1. **Security Risk**: Malicious clients could send arbitrarily large requests
   ```
   Attacker → 10 GB request → Gateway memory exhausted → DoS
   ```

2. **Resource Exhaustion**: Large requests still consume resources and must be bounded:
   - Memory (request buffering)
   - Network bandwidth
   - CPU (parsing large JSON)
   - Disk I/O (if spilled to disk)

3. **Compatibility Gap**: Other AI gateways have explicit limits:
   - ds2api: 50 MB
   - OpenAI API: ~100 MB (undocumented)
   - Anthropic API: ~32 MB (undocumented)

### Why This Matters Now

Modern AI use cases require large request bodies:

#### 1. Vision Models (GPT-4 Vision, Claude 3.5 Sonnet)
```json
{
  "model": "gpt-4-vision",
  "messages": [{
    "role": "user",
    "content": [
      {"type": "text", "text": "Analyze this image"},
      {
        "type": "image_url",
        "image_url": {
          "url": "data:image/jpeg;base64,/9j/4AAQ..." // 5-10 MB
        }
      }
    ]
  }]
}
```

**Size breakdown**:
- 1920x1080 JPEG: ~500 KB raw → ~700 KB base64
- 4K image: ~2 MB raw → ~3 MB base64
- Multiple images: 5-10 MB total

#### 2. Long-Context Conversations
```json
{
  "model": "claude-3-5-sonnet",
  "messages": [
    // 100+ historical messages
    // Each message: 1-2 KB
    // Total: 10-20 MB
  ]
}
```

**Size breakdown**:
- Claude 3.5: 200K tokens context
- Average token: ~4 characters
- 200K tokens = ~800 KB text
- With JSON overhead: ~1-2 MB
- With conversation history: 10-20 MB

#### 3. Complex Tool Definitions
```json
{
  "model": "gpt-4",
  "messages": [...],
  "tools": [
    // 10+ tool definitions
    // Each with complex JSON schema
    // Total: 5-10 MB
  ]
}
```

**Size breakdown**:
- Single tool schema: 100-500 KB
- 10 tools: 1-5 MB
- With examples and descriptions: 5-10 MB

---

## Design Principles

### 1. Security First
- **Principle**: Prevent resource exhaustion attacks
- **Implementation**: Hard limit at middleware layer
- **Rationale**: Security cannot be optional or configurable per-request

### 2. Compatibility
- **Principle**: Support all legitimate AI use cases
- **Implementation**: 50 MB default (matches ds2api)
- **Rationale**: Enables vision + long-context + complex tools

### 3. Flexibility
- **Principle**: Different endpoints have different needs
- **Implementation**: Per-endpoint limit configuration
- **Rationale**: Search endpoints don't need 50 MB, music generation might need more

### 4. Observability
- **Principle**: Monitor and alert on large requests
- **Implementation**: Log requests > 10 MB, metrics for rejected requests
- **Rationale**: Detect abuse patterns and legitimate use case growth

### 5. User Experience
- **Principle**: Clear error messages when limit exceeded
- **Implementation**: Structured error response with request_id
- **Rationale**: Help users understand and fix the issue

---

## Design Decisions

### Decision 1: Default Limit = 50 MB

**Options Considered**:
1. **10 MB**: Too small for vision models
2. **50 MB**: Industry standard (ds2api)
3. **100 MB**: Unnecessarily large, higher DoS risk
4. **Unlimited**: Unacceptable security risk

**Decision**: 50 MB

**Rationale**:
- ✅ Supports vision models (5-10 MB)
- ✅ Supports long-context (10-20 MB)
- ✅ Supports complex tools (5-10 MB)
- ✅ Matches ds2api (compatibility)
- ✅ Reasonable security boundary
- ❌ 100 MB would increase DoS risk without clear benefit

**Trade-offs**:
- **Pro**: Covers 99%+ of legitimate use cases
- **Pro**: Industry-standard value
- **Con**: May need to increase for future use cases (e.g., video input)
- **Mitigation**: Make limit configurable via environment variable

### Decision 2: Per-Endpoint Limits

**Options Considered**:
1. **Global limit only**: Simple but inflexible
2. **Per-endpoint limits**: More complex but better security
3. **Per-user limits**: Too complex, hard to enforce

**Decision**: Per-endpoint limits with global default

**Rationale**:
- Different endpoints have different needs
- Reduces attack surface (search endpoints don't need 50 MB)
- Allows future expansion (video endpoints might need 100 MB)

**Endpoint-Specific Limits**:

| Endpoint | Limit | Rationale |
|----------|-------|-----------|
| `/v1/chat/completions` | 50 MB | Vision + long-context + tools |
| `/v1/messages` | 50 MB | Anthropic long-context |
| `/v1/responses` | 50 MB | OpenAI Responses API |
| `/v1/search` | 5 MB | No images, short queries |
| `/v1/fetch` | 5 MB | No images, short queries |
| `/v1/music/generations` | 10 MB | Music generation parameters |
| `/v1/videos/generations` | 10 MB | Video generation parameters |
| `/v1/images/generations` | 10 MB | Image generation parameters |

### Decision 3: Middleware Layer Implementation

**Options Considered**:
1. **Application layer**: Check after parsing body
2. **Middleware layer**: Check before parsing body
3. **Reverse proxy layer**: Check at nginx/envoy

**Decision**: Middleware layer (tower_http)

**Rationale**:
- ✅ Prevents parsing of oversized bodies (saves CPU)
- ✅ Consistent with Rust/axum ecosystem
- ✅ Easy to configure per-route
- ✅ No external dependency (nginx/envoy)
- ❌ Reverse proxy would be more efficient but adds deployment complexity

**Implementation Strategy**:
```rust
// Use tower_http::limit::RequestBodyLimitLayer
// Applied at router level, before body parsing
```

### Decision 4: Error Response Format

**Options Considered**:
1. **Plain text**: Simple but not machine-readable
2. **OpenAI error format**: Compatible but limited
3. **Custom structured format**: Flexible but non-standard

**Decision**: OpenAI-compatible error format

**Rationale**:
- ✅ Compatible with OpenAI SDK clients
- ✅ Machine-readable (JSON)
- ✅ Includes request_id for debugging
- ✅ Standard error code

**Error Response**:
```json
{
  "error": {
    "code": "request_too_large",
    "message": "Request body exceeds 50 MB limit. Received: 52.3 MB",
    "type": "invalid_request_error",
    "request_id": "req-abc123"
  }
}
```

**HTTP Status**: `413 Payload Too Large`

### Decision 5: Configuration Strategy

**Options Considered**:
1. **Hardcoded**: Simple but inflexible
2. **Environment variables**: Flexible, standard
3. **Config file**: Most flexible but adds complexity

**Decision**: Environment variables with hardcoded defaults

**Rationale**:
- ✅ Standard practice for Rust services
- ✅ Easy to override in deployment
- ✅ No config file parsing overhead
- ✅ Fallback to sensible defaults

**Environment Variables**:
```bash
# Global default
GATEWAY_MAX_REQUEST_BODY_BYTES=52428800  # 50 MB in bytes

# Per-endpoint overrides (optional)
GATEWAY_MAX_BODY_CHAT_COMPLETIONS=52428800
GATEWAY_MAX_BODY_SEARCH=5242880  # 5 MB
GATEWAY_MAX_BODY_MUSIC=10485760  # 10 MB
```

---

## Implementation Strategy

### Phase 1: Global Limit (P0 - Implemented)

**Goal**: Add 50 MB global limit to all endpoints

**Tasks**:
1. Add `tower_http` dependency to `Cargo.toml`
2. Create middleware configuration in `Gateway/src/http/middleware.rs`
3. Apply middleware to router in `Gateway/src/http/router.rs`
4. Add environment variable parsing in `Gateway/src/config.rs`
5. Add error response formatting in `Gateway/src/http/extractors.rs`
6. Add unit tests for limit enforcement
7. Add integration tests for error response format

**Files to Modify**:
- `Gateway/Cargo.toml`: Add `tower-http = { version = "0.5", features = ["limit"] }`
- `Gateway/src/http/middleware.rs`: Create `body_limit_layer()` function
- `Gateway/src/http/router.rs`: Apply middleware to router
- `Gateway/src/config.rs`: Add `max_request_body_bytes` field
- `Gateway/src/http/extractors.rs`: Convert body-too-large rejections into gateway JSON errors

**Acceptance Criteria**:
- [x] Requests > 50 MB are rejected with 413 status
- [x] Error response uses OpenAI-compatible JSON with `request_too_large`
- [x] Environment variable `GATEWAY_MAX_REQUEST_BODY_BYTES` works
- [x] Default is 50 MB when env var not set
- [x] Unit/smoke tests pass
- [x] Integration-style smoke tests pass

### Phase 2: Per-Endpoint Limits (P1 - Implemented)

**Goal**: Configure different limits for different endpoints

**Tasks**:
1. Create endpoint-specific route groups in `Gateway/src/http/router.rs`
2. Apply different limits to different route groups
3. Add per-endpoint environment variables
4. Update configuration parsing
5. Add tests for per-endpoint limits

**Files to Modify**:
- `Gateway/src/http/middleware.rs`: Build reusable `body_limit_layer()`
- `Gateway/src/http/router.rs`: Apply different limits to route groups
- `Gateway/src/config.rs`: Add per-endpoint limit fields

**Acceptance Criteria**:
- [x] Chat endpoints use the chat/body default limit
- [x] Search endpoints reject bodies above the smaller search limit
- [x] Music endpoints use the media body limit
- [x] Per-endpoint env vars work
- [x] Tests pass for covered endpoint types

### Phase 3: Monitoring and Alerting (Pending observability enhancement)

**Goal**: Observe large requests and rejected requests

**Tasks**:
1. Add metrics for request body size distribution
2. Add metrics for rejected requests (413 responses)
3. Add logging for requests > 10 MB
4. Create Grafana dashboard
5. Create Prometheus alerts

**Files to Modify**:
- `Gateway/src/http/middleware.rs`: Add metrics collection
- `Gateway/src/metrics.rs`: Add new metric definitions

**Metrics to Add**:
```rust
// Request body size histogram
gateway_request_body_size_bytes{endpoint="/v1/chat/completions"}

// Rejected requests counter
gateway_request_body_rejected_total{endpoint="/v1/chat/completions", reason="too_large"}

// Large request counter (> 10 MB)
gateway_request_body_large_total{endpoint="/v1/chat/completions"}
```

**Acceptance Criteria**:
- [ ] Metrics are exported to Prometheus
- [ ] Grafana dashboard shows body size distribution
- [ ] Alerts fire when rejection rate > 1%
- [ ] Logs include body size for large requests

### Phase 4: Documentation (Pending wider docs enhancement)

**Goal**: Document the feature for users and operators

**Tasks**:
1. Update `gateway/ARCHITECTURE.md` with body limit section
2. Create user-facing documentation
3. Add troubleshooting guide
4. Update deployment guide with env vars

**Files to Create/Modify**:
- `Gateway/ARCHITECTURE.md`: Add "Request Body Limits" section
- a user-facing API limits guide under `docs/`
- a troubleshooting guide section for `413 Payload Too Large`

**Acceptance Criteria**:
- [x] This design/status doc is updated with current code anchors
   - [ ] Wider architecture doc updated
- [ ] Dedicated user-facing docs complete
- [ ] Dedicated troubleshooting guide includes examples
- [ ] Deployment guide includes env vars

---

## Technical Implementation Details

### Middleware Implementation

```rust
// Gateway/src/http/middleware.rs

use tower_http::limit::RequestBodyLimitLayer;

/// Build a request-body limit layer for a route group.
pub fn body_limit_layer(max_size: usize) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(max_size)
}
```

### Configuration Parsing

```rust
// Gateway/src/config.rs

pub struct Config {
    pub max_request_body_bytes: usize,
    pub max_body_chat_completions_bytes: usize,
    pub max_body_search_bytes: usize,
    pub max_body_music_bytes: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let max_request_body_bytes =
            parse_env_or("GATEWAY_MAX_REQUEST_BODY_BYTES", 50 * 1024 * 1024usize)?;

        Ok(Self {
            max_request_body_bytes,
            max_body_chat_completions_bytes: parse_env_or(
                "GATEWAY_MAX_BODY_CHAT_COMPLETIONS",
                max_request_body_bytes,
            )?,
            max_body_search_bytes: parse_env_or(
                "GATEWAY_MAX_BODY_SEARCH",
                5 * 1024 * 1024usize,
            )?,
            max_body_music_bytes: parse_env_or(
                "GATEWAY_MAX_BODY_MUSIC",
                10 * 1024 * 1024usize,
            )?,
        })
    }
}
```

The full current implementation also exposes endpoint-specific overrides for completions, messages, responses, embeddings, audio, fetch, research, image generation/editing, and video routes.

### Router Integration

```rust
// Gateway/src/http/router.rs

pub fn build_router(state: Arc<AppState>) -> Router {
    let chat_routes = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let search_routes = Router::new()
        .route("/v1/search", post(search))
        .route("/v1/fetch", post(fetch))
        .layer(body_limit_layer(state.config.max_body_search_bytes));

    let media_routes = Router::new()
        .route("/v1/music/generations", post(music_generations))
        .route("/v1/videos/generations", post(video_generations))
        .layer(body_limit_layer(state.config.max_body_music_bytes));

    Router::new()
        .merge(chat_routes)
        .merge(search_routes)
        .merge(media_routes)
}
```

### Error Handling

```rust
// Gateway/src/http/extractors.rs

if message.contains("length limit")
    || message.contains("payload too large")
    || message.contains("Content length limit")
{
    Err(GatewayError {
        kind: crate::error::ErrorKind::BadRequest,
        message: "Request body too large".to_string(),
        code: Some("request_too_large".to_string()),
        http_status: Some(413),
        retryable: false,
        fallback_hint: crate::error::FallbackHint::Abort {
            reason: "Reduce request body size.".to_string(),
        },
        provider_name: None,
    })
}
```

---

## Security Considerations

### 1. Slowloris Attack Prevention

**Threat**: Attacker sends request body very slowly to tie up connections

**Mitigation**: Combine body limit with timeout
```rust
let app = Router::new()
    .layer(body_limit_layer(50 * 1024 * 1024))
    .layer(TimeoutLayer::new(Duration::from_secs(30)));
```

### 2. Memory Exhaustion

**Threat**: Multiple concurrent large requests exhaust memory

**Mitigation**:
- Body limit prevents individual requests from being too large
- Connection limit prevents too many concurrent requests
- AIMD concurrency control limits upstream requests

**Configuration**:
```rust
// Limit concurrent connections
let app = Router::new()
    .layer(ConcurrencyLimitLayer::new(1000));
```

### 3. Disk Exhaustion

**Threat**: If bodies are spilled to disk, attacker fills disk

**Mitigation**:
- `tower_http` buffers in memory, doesn't spill to disk
- Body limit prevents individual requests from being too large
- Monitor disk usage and alert

### 4. Bypass Attempts

**Threat**: Attacker tries to bypass limit with chunked encoding

**Mitigation**:
- `tower_http` enforces limit on total body size, regardless of encoding
- Chunked transfer encoding is still subject to limit

---

## Monitoring and Alerting

### Metrics

```yaml
# Prometheus metrics

# Request body size distribution
gateway_request_body_size_bytes{endpoint="/v1/chat/completions"}
  type: histogram
  buckets: [1KB, 10KB, 100KB, 1MB, 10MB, 50MB]

# Rejected requests
gateway_request_body_rejected_total{endpoint="/v1/chat/completions", reason="too_large"}
  type: counter

# Large requests (> 10 MB)
gateway_request_body_large_total{endpoint="/v1/chat/completions"}
  type: counter
```

### Alerts

```yaml
# Prometheus alerts

- alert: HighBodyLimitRejectionRate
  expr: |
    rate(gateway_request_body_rejected_total[5m]) > 0.01
  for: 5m
  annotations:
    summary: "High rate of requests rejected due to body size limit"
    description: "{{ $value }} requests/sec are being rejected (> 1%)"

- alert: UnusuallyLargeRequests
  expr: |
    rate(gateway_request_body_large_total[5m]) > 10
  for: 10m
  annotations:
    summary: "Unusual number of large requests (> 10 MB)"
    description: "{{ $value }} large requests/sec detected"
```

### Logging

```rust
// Log large requests
if body_size > 10 * 1024 * 1024 {
    tracing::warn!(
        body_size = body_size,
        endpoint = %path,
        user_id = ?user_id,
        "Large request body detected"
    );
}

// Log rejected requests
if body_size > limit {
    tracing::error!(
        body_size = body_size,
        limit = limit,
        endpoint = %path,
        user_id = ?user_id,
        "Request body rejected: too large"
    );
}
```

---

## User-Facing Documentation

### Error Message

When a request exceeds the body limit, users receive:

```json
HTTP/1.1 413 Payload Too Large
Content-Type: application/json

{
  "error": {
    "code": "request_too_large",
    "message": "Request body exceeds 50 MB limit. Received: 52.3 MB",
    "type": "invalid_request_error",
    "request_id": "req-abc123"
  }
}
```

### Troubleshooting Guide

**Problem**: `413 Payload Too Large` error

**Causes**:
1. Sending too many images in a single request
2. Very long conversation history (> 200K tokens)
3. Large tool definitions

**Solutions**:

1. **For vision models**: Reduce image size or number of images
   ```python
   # Bad: Multiple high-res images
   messages = [
       {"role": "user", "content": [
           {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}  # 5 MB
           {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}  # 5 MB
           {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}  # 5 MB
       ]}
   ]

   # Good: Compress images or send separately
   messages = [
       {"role": "user", "content": [
           {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}  # 2 MB (compressed)
       ]}
   ]
   ```

2. **For long conversations**: Truncate old messages
   ```python
   # Bad: Send entire conversation history
   messages = conversation_history  # 100+ messages

   # Good: Keep only recent messages
   messages = conversation_history[-20:]  # Last 20 messages
   ```

3. **For complex tools**: Simplify tool definitions
   ```python
   # Bad: Very detailed schemas
   tools = [
       {
           "type": "function",
           "function": {
               "name": "search",
               "parameters": {
                   # 100+ properties with detailed descriptions
               }
           }
       }
   ]

   # Good: Simplified schemas
   tools = [
       {
           "type": "function",
           "function": {
               "name": "search",
               "parameters": {
                   # Only essential properties
               }
           }
       }
   ]
   ```

---

## Current Verification Strategy

Code-backed verification currently lives in:

- `Gateway/tests/smoke.rs::oversized_request_returns_413_with_request_too_large_code`: verifies oversized chat requests return HTTP 413 with the OpenAI-compatible `request_too_large` error code.
- `Gateway/tests/smoke.rs::search_uses_smaller_body_limit_than_chat`: verifies search routes use the smaller per-endpoint limit while chat routes use the larger chat limit.
- `Gateway/src/config.rs`: env parsing is exercised through route-level smoke configuration and the standard Rust test/build gates.
- `Gateway/tests/python/test_gateway_docs_consistency.py`: guards this document and its `Gateway/` duplicate against stale env var names, old error paths, old week-plan labels, and duplicate-copy drift.
- `scripts/verify-gateway-release-candidate.ps1`: includes the safe-by-default local RC gate. Live-provider canary execution remains explicit opt-in and is not part of default/local/CI gates.

---

## Current Rollout Status

Code-local Rust gateway rollout is complete for global and per-endpoint request body limit enforcement. The remaining scope is outside the code-local release-candidate gate and should be tracked as observability/documentation/product hardening:

- Prometheus/Grafana metrics for body-size distributions and rejected-request rates.
- Wider user/operator documentation and deployment guide examples for request-size env vars.
- Slowloris-specific timeout documentation and verification.
- Load/performance validation under realistic large-request concurrency.

Rollback remains operationally straightforward because limits are environment-configurable; operators can temporarily raise affected per-endpoint limits while investigating false positives, without disabling the global safety boundary permanently.

---

## Future Enhancements

### 1. Dynamic Limits Based on User Tier
```rust
// Premium users get higher limits
let limit = match user.tier {
    UserTier::Free => 10 * 1024 * 1024,      // 10 MB
    UserTier::Pro => 50 * 1024 * 1024,       // 50 MB
    UserTier::Enterprise => 100 * 1024 * 1024, // 100 MB
};
```

### 2. Streaming Body Parsing
```rust
// Parse body as it arrives, reject early if too large
// Saves memory and CPU
```

### 3. Compression Support
```rust
// Accept gzip/brotli compressed bodies
// Decompress and check size
```

### 4. Rate Limiting Based on Body Size
```rust
// Large requests count more towards rate limit
let cost = if body_size > 10 * 1024 * 1024 {
    5 // Large request costs 5x
} else {
    1 // Normal request costs 1x
};
```

---

## Acceptance Criteria

The body limit feature is complete when:

1. **Functionality**:
   - [x] Global 50 MB limit is enforced
   - [x] Per-endpoint limits work correctly
   - [x] Environment variables are respected
   - [x] Error responses use an OpenAI-compatible JSON error code

2. **Security**:
   - [x] Requests > limit are rejected before handler parsing
   - [x] Memory usage is bounded by explicit request body limits
   - [ ] Slowloris-specific timeout hardening is separately documented and verified

3. **Observability**:
   - [ ] Metrics are exported to Prometheus
   - [ ] Logs include body size for large/rejected requests
   - [ ] Grafana dashboard shows body size distribution
   - [ ] Alerts fire when rejection rate > 1%

4. **Testing**:
   - [x] Unit/smoke tests pass
   - [x] Integration-style smoke tests pass
   - [ ] Load tests show no performance regression

5. **Documentation**:
   - [x] This design/status doc is updated with current code anchors
   - [ ] Wider architecture doc updated
   - [ ] Dedicated user-facing docs complete
   - [ ] Dedicated troubleshooting guide includes examples
   - [ ] Deployment guide includes env vars

---

**Last Updated**: 2026-06-09
**Owner**: Rust Gateway Team
**Status**: Phase 1/2 implemented; monitoring and wider docs enhancements pending
