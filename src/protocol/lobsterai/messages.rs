//! Selected Anthropic models share canonical packing, with bounded product-owned response admission.
use super::{invalid, response_error};
use crate::{
    error::GatewayError,
    protocol::{
        anthropic,
        canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind},
    },
    routing::candidate::ProviderAccountPayload,
    upstream::common::RequestPlan,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;
const LIMIT: usize = 16 * 1024 * 1024;
pub(crate) fn selected(payload: &ProviderAccountPayload) -> bool {
    super::owns_payload(payload)
        && super::metadata::read(payload)
            .ok()
            .and_then(|m| m.selection)
            .is_some_and(|s| s.model.api_format.as_deref() == Some("anthropic"))
}
pub(super) fn plan(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    // These Chat controls have no lossless mapping to Messages.
    if [
        "frequency_penalty",
        "presence_penalty",
        "logit_bias",
        "logprobs",
        "top_logprobs",
        "seed",
        "response_format",
    ]
    .iter()
    .any(|k| req.raw_body.get(k).is_some_and(|v| !v.is_null()))
    {
        return Err(invalid());
    }
    // Normalize even same-family requests: cash admission also accepts these explicit aliases.
    let mut normalized = req.clone();
    if !normalized.extra.contains_key("max_tokens") {
        if let Some(limit) = normalized
            .extra
            .get("max_output_tokens")
            .or_else(|| normalized.extra.get("max_completion_tokens"))
            .cloned()
        {
            normalized.extra.insert("max_tokens".into(), limit);
        }
    }
    normalized.extra.remove("max_output_tokens");
    normalized.extra.remove("max_completion_tokens");
    let mut body = anthropic::pack_anthropic(&normalized, model, stream);
    for key in [
        "store",
        "background",
        "conversation",
        "previous_response_id",
        "stream_options",
    ] {
        body.as_object_mut().unwrap().remove(key);
    }
    body["model"] = model.into();
    if let Some(choice) = body.get("tool_choice").and_then(Value::as_str) {
        body["tool_choice"] =
            serde_json::json!({"type":match choice {"required" => "any", other => other}});
    }
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/messages", super::management_boundary::BASE),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::Messages,
    })
}
pub(crate) fn translate<S, E>(
    stream: S,
    model: String,
) -> std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, E>> + Send>>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: From<crate::protocol::stream_error::ProtocolStreamError> + Send + 'static,
{
    Box::pin(super::messages_stream::translate(stream, model))
}
pub(super) async fn read(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    read_inner(response, model, LIMIT).await
}
pub(super) async fn read_probe(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    read_inner(response, model, super::probe_admission::LIMIT).await
}
async fn read_inner(
    response: rquest::Response,
    model: &str,
    limit: usize,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let sse = super::response::is_sse(&response)?;
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(),
            "LobsterAI Messages",
            limit,
            None,
        )
        .await
        .map_err(|_| response_error())?;
    if sse {
        let input = futures::stream::once(async {
            Ok::<_, crate::protocol::stream_error::StreamError<rquest::Error>>(bytes.into())
        });
        let mut stream = translate(input, model.into());
        let mut output = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| response_error())?;
            if output.len().saturating_add(chunk.len()) > limit {
                return Err(response_error());
            }
            output.extend_from_slice(&chunk);
        }
        let value = super::response::aggregate_messages(output.into()).await?;
        crate::protocol::openai::chat_admission::validate(&value).map_err(|_| response_error())?;
        let mut response = crate::protocol::openai::unpack_native_chat_response(&value)
            .map_err(|_| response_error())?;
        // These canonical fields survive aggregation, but the generic Chat JSON reader omits them.
        if let Some(usage) = &mut response.usage {
            usage.cache_read_input_tokens = value["usage"]["cache_read_input_tokens"].as_u64();
            usage.cache_creation_input_tokens =
                value["usage"]["cache_creation_input_tokens"].as_u64();
        }
        return Ok(response);
    }
    let value = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    if value["type"] != "message"
        || value["role"] != "assistant"
        || !value["error"].is_null()
        || !matches!(
            value["stop_reason"].as_str(),
            Some("end_turn" | "stop_sequence" | "max_tokens" | "tool_use")
        )
    {
        return Err(response_error());
    }
    let blocks = value["content"].as_array().ok_or_else(response_error)?;
    if value["stop_reason"] == "tool_use" && !blocks.iter().any(|b| b["type"] == "tool_use") {
        return Err(response_error());
    }
    for block in blocks {
        match block["type"].as_str() {
            Some("text") if block["text"].is_string() => {}
            Some("tool_use")
                if block["id"].as_str().is_some_and(|s| !s.is_empty())
                    && block["name"].as_str().is_some_and(|s| !s.is_empty())
                    && block["input"].is_object() => {}
            _ => return Err(response_error()),
        }
    }
    for key in [
        "input_tokens",
        "output_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ] {
        if value["usage"]
            .get(key)
            .is_some_and(|n| n.as_u64().is_none())
        {
            return Err(response_error());
        }
    }
    anthropic::unpack_anthropic_response(&value).map_err(|_| response_error())
}
