use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Multipart, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde_json::{Map, Value};

use crate::error::GatewayError;
use crate::http::extractors::{JsonBody, OptionalBearerToken};
use crate::http::request_headers::apply_public_request_headers;
use crate::pipeline::{run_pipeline, BinaryPipelineResponse, PipelineContext, PipelineOutput};
use crate::protocol::openai::{normalize_audio_speech, normalize_audio_transcriptions};
use crate::state::AppState;

pub async fn handle_audio_transcriptions(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, GatewayError> {
    let mut body = Map::new();

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        GatewayError::bad_request(format!(
            "Invalid multipart audio transcription body: {error}"
        ))
    })? {
        let Some(name) = field.name().map(str::to_string) else {
            continue;
        };

        let normalized_name = name.trim_end_matches("[]").to_string();
        if name == "file" || field.file_name().is_some() {
            let mime_type = field
                .content_type()
                .map(str::to_string)
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let file_name = field.file_name().map(str::to_string);
            let bytes = field.bytes().await.map_err(|error| {
                GatewayError::bad_request(format!("Invalid audio upload payload: {error}"))
            })?;
            body.insert(
                normalized_name,
                serde_json::json!({
                    "file_name": file_name,
                    "mime_type": mime_type,
                    "base64": base64::engine::general_purpose::STANDARD.encode(bytes),
                }),
            );
            continue;
        }

        let text = field.text().await.map_err(|error| {
            GatewayError::bad_request(format!("Invalid audio transcription field: {error}"))
        })?;
        insert_form_value(&mut body, &normalized_name, Value::String(text));
    }

    let canonical = normalize_audio_transcriptions(Value::Object(body))?;
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
            "Audio transcription endpoint unexpectedly produced a streaming response",
        )),
        PipelineOutput::Binary(binary) => Ok(binary_into_response(binary)),
    }
}

pub async fn handle_audio_speech(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    JsonBody(body): JsonBody,
) -> Result<Response, GatewayError> {
    let canonical = normalize_audio_speech(body)?;
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
            "Audio speech endpoint unexpectedly produced a streaming response",
        )),
        PipelineOutput::Binary(binary) => Ok(binary_into_response(binary)),
    }
}

fn insert_form_value(body: &mut Map<String, Value>, key: &str, value: Value) {
    if let Some(existing) = body.get_mut(key) {
        match existing {
            Value::Array(values) => values.push(value),
            other => {
                let previous = other.take();
                *other = Value::Array(vec![previous, value]);
            }
        }
    } else {
        body.insert(key.to_string(), value);
    }
}

fn binary_into_response(binary: BinaryPipelineResponse) -> Response {
    let mut response = Response::builder().status(StatusCode::OK);
    if let Some(content_type) = binary.content_type.as_deref() {
        response = response.header("content-type", content_type);
    }
    for (name, value) in binary.extra_headers {
        if let (Ok(header_name), Ok(header_value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            response = response.header(header_name, header_value);
        }
    }

    response
        .body(axum::body::Body::from(binary.body))
        .expect("valid binary response")
}
