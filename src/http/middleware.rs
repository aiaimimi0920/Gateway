// ---------------------------------------------------------------------------
// http/middleware.rs — HTTP request logging middleware
//
// Generates a unique X-Request-Id for each request, logs method/path/status/
// latency on completion, and provides utilities for masking sensitive header
// values (API keys, bearer tokens) in log output.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use std::time::Instant;
use tower_http::limit::RequestBodyLimitLayer;
use uuid::Uuid;

use crate::http::route_proof;
use crate::metrics::request::global_gateway_metrics;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request logging middleware
// ---------------------------------------------------------------------------

/// Axum middleware that:
/// 1. Generates a `X-Request-Id` UUID and attaches it to the response.
/// 2. Rejects new work while the process is draining for rolling deployment,
///    while still allowing health/metrics endpoints.
/// 3. Tracks in-flight requests for shutdown observability.
/// 4. Logs every completed request with request_id, method, path, status,
///    and latency_ms.
/// 5. Sanitises bearer tokens / API keys found in headers before logging.
pub async fn request_logging(
    State(state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Response<Body> {
    let request_id = resolve_request_id(request.headers());
    if let Ok(header_value) = request_id.parse() {
        request.headers_mut().insert("x-request-id", header_value);
    }
    let traceparent = request.headers().get("traceparent").cloned();
    let tracestate = request.headers().get("tracestate").cloned();
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let sanitised_headers =
        tracing::enabled!(tracing::Level::DEBUG).then(|| sanitised_auth_headers(request.headers()));

    let start = Instant::now();
    let metrics = global_gateway_metrics();

    let capture_route_proof = route_proof::should_capture_route_proof(
        route_proof::server_route_proof_enabled(),
        request.headers(),
    );
    let ((mut response, drain_rejected), captured_route_proof) = route_proof::capture_route_proof(
        capture_route_proof,
        async {
            let draining = state.lifecycle.is_draining();
            let drain_rejected = draining && !is_drain_exempt_path(path.as_str());
            let response = if drain_rejected {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "error": {
                            "message": "Gateway instance is draining for a rolling deployment. Retry on another instance.",
                            "code": "gateway_draining",
                        }
                    })),
                )
                    .into_response()
            } else {
                state.lifecycle.begin_request();
                metrics.begin_request();
                let response = next.run(request).await;
                metrics.end_request();
                state.lifecycle.end_request();
                response
            };
            (response, drain_rejected)
        },
    )
    .await;
    let latency = start.elapsed().as_millis();
    let status = response.status().as_u16();
    metrics.observe_request(status, latency as u64, drain_rejected);

    // Inject X-Request-Id into response headers.
    if let Ok(header_value) = request_id.parse() {
        response.headers_mut().insert("x-request-id", header_value);
    }
    if let Some(value) = traceparent {
        response.headers_mut().insert("traceparent", value);
    }
    if let Some(value) = tracestate {
        response.headers_mut().insert("tracestate", value);
    }
    route_proof::clear_route_proof_headers(response.headers_mut());
    if response.status().is_success() {
        if let Some(proof) = captured_route_proof.as_ref() {
            route_proof::apply_route_proof_headers(response.headers_mut(), proof);
        }
    }

    // Log at info level for every request.
    tracing::info!(
        request_id = %request_id,
        method = %method,
        path = %path,
        status = status,
        latency_ms = latency as u64,
        "request completed"
    );

    // Log sanitised auth headers at debug level (only when RUST_LOG includes
    // debug; avoids string allocation in production).
    if let Some(sanitised_headers) = sanitised_headers {
        for (name, masked) in sanitised_headers {
            tracing::debug!(
                request_id = %request_id,
                header = %name,
                value = %masked,
                "request auth header (masked)"
            );
        }
    }

    response
}

fn sanitised_auth_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "authorization" | "x-api-key" | "x-goog-api-key" | "anthropic-beta"
            )
        })
        .map(|(name, value)| {
            let value = value.to_str().unwrap_or("<binary>");
            (name.to_string(), mask_sensitive(value))
        })
        .collect()
}

fn resolve_request_id(headers: &HeaderMap) -> String {
    let inbound = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .filter(|value| {
            value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        });

    inbound
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn is_drain_exempt_path(path: &str) -> bool {
    matches!(
        path,
        "/healthz"
            | "/readyz"
            | "/metrics"
            | "/v1/internal/gateway/readiness"
            | "/v1/internal/gateway/operations/summary"
    )
}

/// Build a request-body limit layer for a route group.
pub fn body_limit_layer(max_size: usize) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(max_size)
}

// ---------------------------------------------------------------------------
// Sensitive value masking
// ---------------------------------------------------------------------------

/// Mask a potentially sensitive string value.
///
/// - Values of 16 bytes or fewer are replaced entirely with `"***"`.
/// - Longer values show the first 8 bytes and last 4 bytes with
///   `***` in between: `sk-abc12345***wxyz`.
/// - A preview that would split a UTF-8 character is replaced entirely.
///
/// This heuristic keeps enough context to identify a key during debugging
/// without exposing the full secret.
pub fn mask_sensitive(value: &str) -> String {
    if value.len() <= 16 {
        return "***".to_string();
    }
    let (Some(prefix), Some(suffix)) = (value.get(..8), value.get(value.len() - 4..)) else {
        return "***".to_string();
    };
    format!("{prefix}***{suffix}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn mask_short_value_is_fully_hidden() {
        assert_eq!(mask_sensitive("sk-short"), "***");
        assert_eq!(mask_sensitive(""), "***");
        assert_eq!(mask_sensitive("1234567890123456"), "***"); // exactly 16
    }

    #[test]
    fn mask_long_value_shows_first_8_and_last_4() {
        let key = "sk-1234567890abcdefghijklmn";
        let masked = mask_sensitive(key);
        assert_eq!(masked, "sk-12345***klmn");
        // Ensure the full key is NOT present.
        assert!(!masked.contains(key));
    }

    #[test]
    fn mask_bearer_prefix_preserved() {
        let bearer = "Bearer sk-abcdefghijklmnopqrstuvwxyz";
        let masked = mask_sensitive(bearer);
        assert_eq!(masked, "Bearer s***wxyz");
    }

    #[test]
    fn mask_boundary_17_chars() {
        let val = "12345678901234567"; // 17 chars
        let masked = mask_sensitive(val);
        assert_eq!(masked, "12345678***4567");
    }

    #[test]
    fn sanitised_auth_headers_filters_and_masks_without_normalising_names() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            "Bearer test-secret-that-is-long".parse().unwrap(),
        );
        headers.insert("content-type", "application/json".parse().unwrap());

        let sanitised = sanitised_auth_headers(&headers);

        assert_eq!(sanitised.len(), 1);
        assert_eq!(sanitised[0].0, "authorization");
        assert!(!sanitised[0].1.contains("test-secret-that-is-long"));
    }

    #[test]
    fn request_id_reuses_safe_inbound_value() {
        let mut headers = HeaderMap::new();
        headers.insert("x-request-id", "client-request-42".parse().unwrap());
        assert_eq!(resolve_request_id(&headers), "client-request-42");
    }

    #[test]
    fn request_id_rejects_unsafe_or_oversized_inbound_value() {
        let mut headers = HeaderMap::new();
        let oversized = "a".repeat(256);
        headers.insert("x-request-id", oversized.parse().unwrap());
        let generated = resolve_request_id(&headers);
        assert_ne!(generated, oversized);
        assert!(generated.len() >= 16);
    }

    #[test]
    fn secret_preview_rejects_partial_unicode_boundaries() {
        for value in ["1234567\u{00e9}abcdefghij", "1234567890123\u{00e9}xyz"] {
            assert_eq!(mask_sensitive(value), "***");
        }
    }

    #[test]
    fn secret_preview_preserves_complete_unicode_boundaries() {
        let value = "\u{00e9}".repeat(9);
        assert_eq!(
            mask_sensitive(&value),
            format!("{}***{}", "\u{00e9}".repeat(4), "\u{00e9}".repeat(2))
        );
        assert_eq!(mask_sensitive(&"\u{00e9}".repeat(8)), "***");
    }
}
