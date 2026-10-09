//! Native DashScope ingress shares authentication, entitlement and accounting pipeline.
use super::dashscope_error::DashScopeError;
use crate::{
    error::GatewayError,
    http::{
        extractors::{JsonBody, OptionalBearerToken},
        request_headers::apply_public_request_headers,
        sse::into_sse_response,
    },
    pipeline::{run_pipeline, PipelineContext, PipelineOutput},
    protocol::{canonical::EndpointKind, dashscope},
    state::AppState,
    upstream::stream::TrackedStream,
};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use std::{collections::HashMap, sync::Arc};

pub async fn text(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, DashScopeError> {
    handle(state, token, query, headers, body, false)
        .await
        .map_err(Into::into)
}
pub async fn multimodal(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, DashScopeError> {
    handle(state, token, query, headers, body, true)
        .await
        .map_err(Into::into)
}
async fn handle(
    state: Arc<AppState>,
    token: Option<String>,
    query: HashMap<String, String>,
    headers: HeaderMap,
    body: serde_json::Value,
    multimodal: bool,
) -> Result<Response, GatewayError> {
    let stream = headers
        .get("x-dashscope-sse")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("enable"));
    let message_format = body
        .pointer("/parameters/result_format")
        .and_then(serde_json::Value::as_str)
        == Some("message");
    let incremental = body
        .pointer("/parameters/incremental_output")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let canonical = dashscope::normalize(body, multimodal, stream)?;
    let model = canonical.requested_model.clone().unwrap_or_default();
    let mut ctx = PipelineContext::new(canonical, token);
    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &["x-client-name"],
    )?;
    ctx.query_params = query;
    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(value) => Ok(Json(dashscope::response::from_openai(
            &value,
            multimodal,
            message_format,
        )?)
        .into_response()),
        PipelineOutput::Sse(stream) => {
            let translated = dashscope::ingress_stream::native_or_translate(
                stream,
                model,
                multimodal,
                message_format,
                incremental,
            );
            Ok(into_sse_response(
                TrackedStream::new_with_error(translated, |_, _| {}),
                EndpointKind::ChatCompletions,
            )
            .into_response())
        }
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "Unexpected DashScope binary response",
        )),
    }
}
