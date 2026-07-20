// ---------------------------------------------------------------------------
// http/routes/messages.rs — POST /v1/messages (Anthropic protocol)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};

use crate::error::GatewayError;
use crate::http::extractors::{JsonBody, OptionalBearerToken};
use crate::http::request_headers::apply_public_request_headers;
use crate::http::sse::into_sse_response;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::anthropic::normalize_messages;
use crate::protocol::canonical::EndpointKind;
use crate::state::AppState;

/// POST /v1/messages
///
/// Accepts an Anthropic-compatible messages request, routes it through the
/// gateway pipeline, and returns either a JSON response or an SSE stream.
///
/// Supports `?beta=prompt-caching-2024-07-31` query parameter — when present,
/// the value is forwarded as the `anthropic-beta` header to the upstream API.
pub async fn handle_messages(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = normalize_messages(body)?;
    let mut ctx = PipelineContext::new(canonical, token);

    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        &["anthropic-version", "anthropic-beta"],
    );

    // Forward ?beta=<value> as the `anthropic-beta` request header so that
    // the upstream header builder can pick it up.  If the header was already
    // set from the original request, the query param value is appended
    // (comma-separated) rather than replaced.
    ctx.query_params = query_params;
    if let Some(beta_value) = ctx.query_params.get("beta") {
        let existing = ctx.request_headers.get("anthropic-beta").cloned();
        let merged = match existing {
            Some(prev) if !prev.is_empty() => format!("{},{}", prev, beta_value),
            _ => beta_value.clone(),
        };
        ctx.request_headers
            .insert("anthropic-beta".to_string(), merged);
    }

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(v) => Ok(Json(v).into_response()),
        PipelineOutput::Sse(stream) => {
            Ok(into_sse_response(stream, EndpointKind::Messages).into_response())
        }
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for messages endpoint",
        )),
    }
}
