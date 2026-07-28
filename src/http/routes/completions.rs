// ---------------------------------------------------------------------------
// http/routes/completions.rs — POST /v1/chat/completions and /v1/completions
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
use crate::protocol::canonical::EndpointKind;
use crate::protocol::openai::{normalize_chat_completions, normalize_legacy_completions};
use crate::state::AppState;

/// POST /v1/chat/completions
///
/// Accepts an OpenAI-compatible chat completions request, routes it through
/// the gateway pipeline, and returns either a JSON response or an SSE stream.
pub async fn handle_chat_completions(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = normalize_chat_completions(body)?;
    let mut ctx = PipelineContext::new(canonical, token);

    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &[],
    )?;
    ctx.query_params = query_params;

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(v) => Ok(Json(v).into_response()),
        PipelineOutput::Sse(stream) => {
            Ok(into_sse_response(stream, EndpointKind::ChatCompletions).into_response())
        }
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for chat completions endpoint",
        )),
    }
}

/// POST /v1/completions
///
/// Accepts the legacy OpenAI completions schema and forwards it through the
/// same unified gateway pipeline. Non-streaming responses are returned as JSON;
/// streaming responses are forwarded as raw SSE bytes.
pub async fn handle_legacy_completions(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = normalize_legacy_completions(body)?;
    let mut ctx = PipelineContext::new(canonical, token);
    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &[],
    )?;
    ctx.query_params = query_params;

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(v) => Ok(Json(v).into_response()),
        PipelineOutput::Sse(stream) => {
            Ok(into_sse_response(stream, EndpointKind::Completions).into_response())
        }
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for legacy completions endpoint",
        )),
    }
}
