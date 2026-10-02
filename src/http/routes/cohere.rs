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
use crate::protocol::{cohere, openai};
use crate::state::AppState;
use crate::upstream::stream::TrackedStream;

pub async fn handle_chat_v2(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = cohere::normalize_chat_v2(body)?;
    let mut ctx = PipelineContext::new(canonical.clone(), token);
    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &["x-client-name"],
    )?;
    ctx.query_params = query_params;

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(value) => {
            let canonical_resp = openai::unpack_openai_response(&value)?;
            Ok(Json(cohere::build_chat_v2_success(
                "chatcmpl-gateway",
                &canonical_resp.model,
                &canonical_resp.text,
                canonical_resp.usage.as_ref(),
                &canonical_resp.tool_calls,
                canonical_resp.finish_reason.as_deref(),
            ))
            .into_response())
        }
        PipelineOutput::Sse(stream) => {
            let translated = cohere::translate_openai_sse_to_cohere_stream_with_error(
                stream,
                canonical.requested_model.unwrap_or_default(),
            );
            let wrapped = TrackedStream::new_with_error(translated, |_, _| {});
            Ok(into_sse_response(
                wrapped,
                crate::protocol::canonical::EndpointKind::ChatCompletions,
            )
            .into_response())
        }
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for Cohere chat",
        )),
    }
}
