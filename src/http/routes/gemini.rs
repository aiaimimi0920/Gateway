use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};

use crate::error::GatewayError;
use crate::http::extractors::{JsonBody, OptionalBearerToken};
use crate::http::request_headers::apply_public_request_headers;
use crate::http::sse::into_sse_response;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::{gemini_api, openai};
use crate::state::AppState;
use crate::upstream::stream::TrackedStream;

pub async fn handle_v1_model_action(
    Path(action): Path<String>,
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    handle_model_action(action, state, token, query_params, headers, body).await
}

pub async fn handle_v1beta_model_action(
    Path(action): Path<String>,
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    handle_model_action(action, state, token, query_params, headers, body).await
}

async fn handle_model_action(
    action: String,
    state: Arc<AppState>,
    token: Option<String>,
    query_params: HashMap<String, String>,
    headers: HeaderMap,
    body: serde_json::Value,
) -> Result<Response, GatewayError> {
    let (model, stream) = parse_model_action(action.as_str())?;
    let canonical = gemini_api::normalize_generate_content(body, Some(model.clone()), stream)?;
    let mut ctx = PipelineContext::new(canonical, token);
    apply_public_request_headers(&mut ctx, &headers, &state.config, &["x-goog-api-key"]);
    ctx.query_params = query_params;

    match (stream, run_pipeline(ctx, &state).await?) {
        (false, PipelineOutput::Json(value)) => build_generate_content_response(value),
        (false, PipelineOutput::Sse(_)) => Err(GatewayError::server_error(
            "unexpected streaming response for Gemini generateContent",
        )),
        (false, PipelineOutput::Binary(_)) => Err(GatewayError::server_error(
            "unexpected binary response for Gemini generateContent",
        )),
        (true, PipelineOutput::Sse(stream)) => {
            build_stream_generate_content_response(stream, &model)
        }
        (true, PipelineOutput::Json(_)) => Err(GatewayError::server_error(
            "unexpected non-streaming response for Gemini streamGenerateContent",
        )),
        (true, PipelineOutput::Binary(_)) => Err(GatewayError::server_error(
            "unexpected binary response for Gemini streamGenerateContent",
        )),
    }
}

fn build_generate_content_response(value: serde_json::Value) -> Result<Response, GatewayError> {
    let canonical_resp = openai::unpack_openai_response(&value)?;
    Ok(Json(gemini_api::build_generate_content_success(
        &canonical_resp.model,
        &canonical_resp.text,
        canonical_resp.usage.as_ref(),
        &canonical_resp.tool_calls,
        canonical_resp.finish_reason.as_deref(),
    ))
    .into_response())
}

fn build_stream_generate_content_response(
    stream: TrackedStream,
    model: &str,
) -> Result<Response, GatewayError> {
    let translated =
        gemini_api::translate_openai_sse_to_gemini_stream(stream, model_for_stream_header(model));
    let wrapped = TrackedStream::new(translated, |_, _| {});
    Ok(into_sse_response(
        wrapped,
        crate::protocol::canonical::EndpointKind::ChatCompletions,
    )
    .into_response())
}

fn model_for_stream_header(model: &str) -> String {
    model.to_string()
}

fn parse_model_action(action: &str) -> Result<(String, bool), GatewayError> {
    let normalized = action.trim_start_matches('/');
    if let Some(model) = normalized.strip_suffix(":generateContent") {
        if model.is_empty() {
            return Err(GatewayError::bad_request(
                "Gemini model path is missing the model name",
            ));
        }
        return Ok((model.to_string(), false));
    }
    if let Some(model) = normalized.strip_suffix(":streamGenerateContent") {
        if model.is_empty() {
            return Err(GatewayError::bad_request(
                "Gemini model path is missing the model name",
            ));
        }
        return Ok((model.to_string(), true));
    }
    Err(GatewayError::not_found(format!(
        "unsupported Gemini model action path: {normalized}"
    )))
}
