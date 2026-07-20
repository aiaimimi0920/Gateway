# Error Handling Strategy for Rust Gateway

## Purpose

This document records the current Rust gateway error contract. The implementation is code-backed by `Gateway/src/error.rs` and the HTTP JSON extractor path in `Gateway/src/http/extractors.rs`.

The current contract is not the older enum-only `GatewayError` design. The gateway now uses:

- `ErrorKind`: coarse error classification for routing, response status, and provider-failure interpretation.
- `FallbackHint`: structured operational guidance for retry, provider fallback, model downgrade, or abort.
- `GatewayError`: a structured error record carrying kind, message, optional public code, optional HTTP status, retryability, fallback hint, and optional provider name.

## Current Code Anchors

- `Gateway/src/error.rs`: defines `ErrorKind`, `FallbackHint`, `GatewayError`, `GatewayError::bad_request`, `GatewayError::rate_limited`, upstream classification, fallback hint construction, and Axum `IntoResponse` conversion.
- `Gateway/src/http/extractors.rs`: converts JSON extraction failures, including `RequestBodyLimitLayer` body-too-large rejections, into gateway JSON errors. Oversized request bodies use public code `request_too_large` and HTTP status `413`.
- `Gateway/tests/smoke.rs`: verifies oversized requests return `413` with `request_too_large`.
- `Gateway/tests/python/test_gateway_docs_consistency.py`: guards this document and its `Gateway/` duplicate against stale enum-only error documentation and duplicate-copy drift.

## Structured Error Types

### `ErrorKind`

`Gateway/src/error.rs` defines the current coarse error categories:

```rust
pub enum ErrorKind {
    Authentication,
    RateLimit,
    ContextLength,
    ContentFilter,
    ModelNotFound,
    ServerError,
    Network,
    BadRequest,
    InsufficientQuota,
    ServiceUnavailable,
    Timeout,
    Unknown,
}
```

The gateway derives an `ErrorKind` from explicit constructors, HTTP status, provider body keywords, and upstream classification. Important helpers include:

- `kind_from_http_status`
- `kind_from_body_keywords`
- `classify_upstream_error`
- `classify_upstream_error_with_retry_after`

### `FallbackHint`

`FallbackHint` is serialized with an `action` tag and communicates what downstream routing or operators should do:

```rust
pub enum FallbackHint {
    Retry { delay_ms: u64, reason: String },
    FallbackProvider { reason: String },
    DowngradeModel { suggested_model: Option<String>, reason: String },
    Abort { reason: String },
}
```

Current defaults are created by `build_fallback_hint`:

- Rate limits and transient service failures usually produce `FallbackHint::Retry`.
- quota, server, network, and unknown provider failures generally produce `FallbackHint::FallbackProvider`.
- context-length and model-not-found failures can produce `FallbackHint::DowngradeModel`.
- authentication, bad request, and content-filter failures produce `FallbackHint::Abort`.

### `GatewayError`

The current `GatewayError` is a struct, not an enum:

```rust
pub struct GatewayError {
    pub kind: ErrorKind,
    pub message: String,
    pub code: Option<String>,
    pub http_status: Option<u16>,
    pub retryable: bool,
    pub fallback_hint: FallbackHint,
    pub provider_name: Option<String>,
}
```

Constructor helpers keep common cases consistent:

- `GatewayError::bad_request(...)`
- `GatewayError::unauthorized(...)`
- `GatewayError::not_found(...)`
- `GatewayError::conflict(...)`
- `GatewayError::rate_limited(...)`
- `GatewayError::service_unavailable(...)`
- `GatewayError::with_provider(...)`

## HTTP Response Contract

`GatewayError` implements Axum `IntoResponse` in `Gateway/src/error.rs` through `impl axum::response::IntoResponse for GatewayError`. The response body shape is:

```json
{
  "error": {
    "message": "...",
    "type": "BadRequest",
    "code": "optional_public_code"
  }
}
```

HTTP status is selected from `GatewayError.http_status` when present. Otherwise the gateway falls back to an `ErrorKind`-based status mapping:

- `Authentication` -> `401`
- `BadRequest` -> `400`
- `RateLimit` -> `429`
- `ModelNotFound` -> `404`
- otherwise -> `500`

## Request Body Limit Error Path

`Gateway/src/http/extractors.rs` wraps Axum JSON extraction through `JsonBody`. It recognizes body-limit rejection messages from `RequestBodyLimitLayer` and emits a structured gateway error:

```rust
GatewayError {
    kind: crate::error::ErrorKind::BadRequest,
    message: "Request body too large".to_string(),
    code: Some("request_too_large".to_string()),
    http_status: Some(413),
    retryable: false,
    fallback_hint: crate::error::FallbackHint::Abort {
        reason: "Reduce request body size.".to_string(),
    },
    provider_name: None,
}
```

This path is covered by `Gateway/tests/smoke.rs::oversized_request_returns_413_with_request_too_large_code`.

## Upstream Provider Error Classification

Provider errors are classified by HTTP status, response body keywords, and optional retry-after metadata.

Current important behavior:

- `429` -> `ErrorKind::RateLimit`, retryable, usually `FallbackHint::Retry`.
- quota/credit body text -> `ErrorKind::InsufficientQuota`, usually provider fallback.
- context/token length body text -> `ErrorKind::ContextLength`, usually model downgrade guidance.
- content filter/safety/moderation body text -> `ErrorKind::ContentFilter`, abort.
- authentication status/body text -> `ErrorKind::Authentication`, abort.
- server/network failures -> provider fallback or retry depending on classification.

This classification is used by the pipeline to distinguish retryable/fallbackable provider failures from request-local errors that should abort.

## Current Verification Strategy

Code-backed verification currently lives in:

- `Gateway/src/error.rs` unit tests for HTTP status mapping, upstream keyword/status classification, retry-after handling, and fallback hint serialization.
- `Gateway/src/http/extractors.rs` implementation for body-too-large JSON conversion.
- `Gateway/tests/smoke.rs::oversized_request_returns_413_with_request_too_large_code` for the externally visible request-size error contract.
- `Gateway/tests/python/test_gateway_docs_consistency.py` for this document's current-code anchors and duplicate-copy synchronization.
- `scripts/verify-gateway-release-candidate.ps1` for the safe-by-default local RC gate. Live-provider canary execution remains explicit opt-in and is not part of default/local/CI gates.

## Current Status and Remaining Scope

Code-local Rust gateway error handling is implemented around `ErrorKind`, `FallbackHint`, and structured `GatewayError`.

Remaining non-blocking hardening areas:

- richer operator-facing error-rate dashboards by `ErrorKind` and provider;
- broader database/Redis-specific mapping docs if those layers need public runbooks;
- deeper end-to-end tests for provider classification across live upstreams, kept explicit opt-in because they depend on credentials, quota, network, and provider behavior.

Default/local/CI verification must continue to avoid live third-party calls. Credential, quota, upstream-provider, and network failures must remain classified separately from code-local RC gate failures.

---

**Last Updated**: 2026-06-09
**Owner**: Rust Gateway Team
**Status**: Structured Rust error contract implemented; richer operator reporting remains future observability work