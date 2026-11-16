//! 非流式决策入口：沿用公开鉴权、过滤、路由、预扣及审计 pipeline。
use crate::{
    error::GatewayError,
    http::{
        extractors::{JsonBody, OptionalBearerToken},
        request_headers::apply_public_request_headers,
    },
    pipeline::{run_pipeline, PipelineContext, PipelineOutput},
    state::AppState,
};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use std::{collections::HashMap, sync::Arc};

pub async fn handle(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = crate::protocol::commandcode::systemone::normalize(body)?;
    let mut ctx = PipelineContext::new(canonical, token);
    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &["x-project-id"],
    )?;
    ctx.query_params = query;
    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(body) => Ok(Json(body).into_response()),
        _ => Err(GatewayError::server_error(
            "System One unexpectedly produced a non-JSON response.",
        )),
    }
}
