//! Fixed SAP deployment transport, using the shared Bedrock request and frame codecs.
use super::*;
use crate::{
    protocol::{
        bedrock_converse,
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind},
        stream_error::{ProtocolStreamError, StreamError},
    },
    upstream::{common::RequestPlan, response_types::UpstreamStreamingResponse},
};
use futures::TryStreamExt;
use serde_json::{json, Value};

pub(super) const EVENTSTREAM: &str = "application/vnd.amazon.eventstream";

pub(super) fn selected(payload: &ProviderAccountPayload) -> bool {
    payload
        .extra_body
        .as_ref()
        .and_then(|b| b.get(BEDROCK_CONVERSE))
        == Some(&Value::Bool(true))
}

pub(super) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    let raw = req.raw_body.as_object().ok_or_else(invalid)?;
    const CONTROLS: &[&str] = &[
        "max_tokens",
        "max_completion_tokens",
        "temperature",
        "top_p",
        "stop",
        "n",
        "stream_options",
        "user",
    ];
    if req.endpoint_kind != EndpointKind::ChatCompletions
        || raw.keys().any(|k| {
            !matches!(
                k.as_str(),
                "model" | "messages" | "stream" | "tools" | "tool_choice"
            ) && !CONTROLS.contains(&k.as_str())
        })
        || raw
            .get("stream")
            .is_some_and(|v| v.as_bool() != Some(stream))
        || raw
            .get("stream_options")
            .is_some_and(|v| !stream || v != &json!({"include_usage":true}))
        || raw.get("n").is_some_and(|v| v.as_u64() != Some(1))
        || raw.get("user").is_some_and(|v| !v.is_string())
        || req.reasoning.is_some()
        || req.previous_response_id.is_some()
        || req.extra.iter().any(|(k, v)| {
            !CONTROLS.contains(&k.as_str())
                || k == "n" && v.as_u64() != Some(1)
                || k == "stream_options" && (!stream || v != &json!({"include_usage":true}))
        })
    {
        return Err(invalid());
    }
    let images_enabled = payload
        .extra_body
        .as_ref()
        .and_then(|settings| settings.get(BEDROCK_IMAGE_INPUT))
        == Some(&Value::Bool(true));
    let mut body = bedrock_tools::pack(req, model, stream, images_enabled)?;
    let fields = body.as_object_mut().ok_or_else(invalid)?;
    for local in ["n", "stream_options", "user"] {
        fields.remove(local);
    }
    let operation = if stream {
        "converse-stream"
    } else {
        "converse"
    };
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/{operation}", payload.base_url),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::ChatCompletions,
    })
}

pub(super) fn unpack(
    body: &Value,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Option<CanonicalRelayResponse> {
    let content = body.pointer("/output/message/content")?.as_array()?;
    if content.is_empty() || content.len() > 4096 {
        return None;
    }
    let mut tools = bedrock_tools::response_tools(req);
    for block in content {
        if block.get("text").is_some_and(Value::is_string) {
            continue;
        }
        let tool = block.get("toolUse")?;
        let id = tool["toolUseId"].as_str()?;
        let name = tool["name"].as_str()?;
        if tool.as_object()?.len() != 3 || !tool["input"].is_object() || !tools.accept(id, name) {
            return None;
        }
    }
    let mut result = bedrock_converse::unpack_bedrock_response(body).ok()?;
    if !tools.complete(result.finish_reason.as_deref()?) {
        return None;
    }
    let usage = crate::protocol::openai::chat_usage(result.usage.as_ref()?);
    crate::protocol::openai::chat_admission::validate_usage(&json!({"usage":usage})).ok()?;
    // Never reflect an internal model identifier or publish native continuation state.
    result.model = model.into();
    result.bedrock_turn = None;
    Some(result)
}

pub(super) fn stream(
    response: rquest::Response,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let correct_mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case(EVENTSTREAM));
    if !correct_mime {
        return Err(failure("sap_aicore_response_failed"));
    }
    let alias = req.requested_model.as_deref().unwrap_or(model).to_owned();
    let source = response.bytes_stream().map_err(StreamError::Transport);
    let translated =
        bedrock_converse::translate_public(source, alias, bedrock_tools::response_tools(req))
            .map_err(|error: StreamError<rquest::Error>| {
                StreamError::from(match error {
                    StreamError::Transport(_) => super::errors::interrupted(),
                    _ => stream_failure(),
                })
            });
    Ok(UpstreamStreamingResponse::NativeEvents(Box::pin(
        translated,
    )))
}

fn stream_failure() -> ProtocolStreamError {
    ProtocolStreamError::classified(
        "invalid_sap_aicore_bedrock_stream: incomplete or invalid Converse stream",
        failure("sap_aicore_response_failed"),
    )
}
