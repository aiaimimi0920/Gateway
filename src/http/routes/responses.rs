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
use crate::http::extractors::OptionalBearerToken;
use crate::http::native_json::NativeJson;
use crate::http::request_headers::apply_public_request_headers;
use crate::http::sse::into_sse_response;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::canonical::EndpointKind;
use crate::protocol::responses::normalize_responses;
use crate::state::AppState;

mod websocket;
pub(crate) use websocket::handle_websocket;

/// POST /v1/responses
///
/// Accepts an OpenAI Responses API request, routes it through the gateway
/// pipeline, and returns either a JSON response or an SSE stream.
pub async fn handle_responses(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    mut headers: HeaderMap,
    NativeJson {
        value: body,
        mut wire,
    }: NativeJson,
) -> Result<Response, GatewayError> {
    crate::http::native_session::normalize_headers(&mut headers)?;
    crate::http::native_session::attach_sdk_session(
        &state,
        token.as_deref(),
        &headers,
        &body,
        &mut wire,
    )
    .await?;
    let mut ctx = prepare_context(body, token, query_params, &headers, &state)?;
    ctx.native_wire = Some(Arc::new(wire));
    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(v) => Ok(Json(v).into_response()),
        PipelineOutput::Sse(mut stream) => {
            let metadata = std::mem::take(&mut stream.native_response_metadata);
            let mut response = into_sse_response(stream, EndpointKind::Responses).into_response();
            metadata.apply(response.headers_mut());
            Ok(response)
        }
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for responses endpoint",
        )),
    }
}

fn prepare_context(
    body: serde_json::Value,
    token: Option<String>,
    query_params: HashMap<String, String>,
    headers: &HeaderMap,
    state: &Arc<AppState>,
) -> Result<PipelineContext, GatewayError> {
    let canonical = normalize_responses(body)?;
    let mut ctx = PipelineContext::new(canonical, token);

    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &[],
    )?;
    ctx.query_params = query_params;
    for name in crate::protocol::codex_wire::FORWARDED_HEADERS {
        if let Some(value) = headers.get(*name).and_then(|v| v.to_str().ok()) {
            if crate::protocol::codex_wire::valid_header(name, value) {
                ctx.request_headers
                    .insert((*name).to_owned(), value.to_owned());
            }
        }
    }
    Ok(ctx)
}

/// Compact is a separately admitted, buffered Responses operation. Keep the
/// operation out of caller-controlled JSON so it cannot redirect normal creates.
pub async fn handle_compact(
    state: State<Arc<AppState>>,
    token: OptionalBearerToken,
    query: Query<HashMap<String, String>>,
    headers: HeaderMap,
    mut body: NativeJson,
) -> Result<Response, GatewayError> {
    if body.value.get("stream").is_some() {
        return Err(GatewayError::bad_request("Compact does not accept stream"));
    }
    body.wire.compact = true;
    handle_responses(state, token, query, headers, body).await
}
