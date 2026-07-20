// ---------------------------------------------------------------------------
// http/routes/responses.rs — POST /v1/responses (OpenAI Responses API)
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
use crate::protocol::responses::normalize_responses;
use crate::state::AppState;

/// POST /v1/responses
///
/// Accepts an OpenAI Responses API request, routes it through the gateway
/// pipeline, and returns either a JSON response or an SSE stream.
pub async fn handle_responses(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = normalize_responses(body)?;
    let mut ctx = PipelineContext::new(canonical, token);

    apply_public_request_headers(&mut ctx, &headers, &state.config, &[]);
    ctx.query_params = query_params;

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(v) => Ok(Json(v).into_response()),
        PipelineOutput::Sse(stream) => {
            Ok(into_sse_response(stream, EndpointKind::Responses).into_response())
        }
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for responses endpoint",
        )),
    }
}
