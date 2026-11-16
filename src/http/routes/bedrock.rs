use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures::StreamExt;

use crate::error::GatewayError;
use crate::http::extractors::{JsonBody, OptionalBearerToken};
use crate::http::request_headers::apply_public_request_headers;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::{bedrock_converse, openai};
use crate::state::AppState;

pub async fn handle_converse(
    Path(model): Path<String>,
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = bedrock_converse::normalize_converse(body, Some(model), false)?;
    let mut ctx = PipelineContext::new(canonical, token);
    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &[
            "x-amzn-bedrock-accept",
            "x-amzn-bedrock-content-type",
            "x-amzn-bedrock-guardrailidentifier",
            "x-amzn-bedrock-guardrailversion",
        ],
    )?;
    ctx.query_params = query_params;

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Json(value) => {
            let canonical_resp = openai::unpack_openai_response(&value)?;
            Ok(Json(bedrock_converse::build_converse_response(&canonical_resp)).into_response())
        }
        PipelineOutput::Sse(_) => Err(GatewayError::server_error(
            "unexpected streaming response for Bedrock converse",
        )),
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for Bedrock converse",
        )),
    }
}

pub async fn handle_converse_stream(
    Path(model): Path<String>,
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = bedrock_converse::normalize_converse(body, Some(model.clone()), true)?;
    let mut ctx = PipelineContext::new(canonical, token);
    apply_public_request_headers(
        &mut ctx,
        &headers,
        &state.config,
        state.console_auth.as_ref(),
        &[
            "x-amzn-bedrock-accept",
            "x-amzn-bedrock-content-type",
            "x-amzn-bedrock-guardrailidentifier",
            "x-amzn-bedrock-guardrailversion",
        ],
    )?;
    ctx.query_params = query_params;

    match run_pipeline(ctx, &state).await? {
        PipelineOutput::Sse(stream) => {
            // Native encoding already ran inside pipeline tracking; never encode a second time here.
            let mapped = stream.map(|item| {
                // Err has already crossed the sole tracker and fired its failure/refund callback.
                // Encoding a terminal exception here cannot turn that outcome into a success.
                if let Err(crate::protocol::stream_error::StreamError::Protocol(error)) = &item {
                    match bedrock_converse::encode_native_failure(error) {
                        Ok(Some(bytes)) => return Ok(bytes),
                        Err(error) => {
                            return Err(Box::new(error) as Box<dyn std::error::Error + Send + Sync>)
                        }
                        Ok(None) => {}
                    }
                }
                item.map_err(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>)
            });
            let body = Body::from_stream(mapped);
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.amazon.eventstream")
                .header(header::CACHE_CONTROL, "no-cache")
                .body(body)
                .expect("valid bedrock eventstream response"))
        }
        PipelineOutput::Json(_) => Err(GatewayError::server_error(
            "unexpected non-streaming response for Bedrock converse-stream",
        )),
        PipelineOutput::Binary(_) => Err(GatewayError::server_error(
            "unexpected binary response for Bedrock converse-stream",
        )),
    }
}
