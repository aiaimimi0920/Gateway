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
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::openai::normalize_embeddings;
use crate::state::AppState;

/// POST /v1/embeddings
///
/// This is treated as an OpenAI-compatible passthrough endpoint. The request is
/// still authenticated, routed, audited, and quota-checked by the unified
/// gateway pipeline before the selected provider is invoked.
pub async fn handle_embeddings(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = normalize_embeddings(body)?;
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
        PipelineOutput::Sse(_) => Err(GatewayError::server_error(
            "Embeddings endpoint unexpectedly produced a streaming response",
        )),
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "Embeddings endpoint unexpectedly produced a binary response",
        )),
    }
}
