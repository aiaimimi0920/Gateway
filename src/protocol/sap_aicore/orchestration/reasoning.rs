//! Preserve SAP harmonized reasoning blocks without exposing intermediate module results.
use super::{final_result, owns_payload, runtime, ORCHESTRATION_MODEL, ORCHESTRATION_REASONING};
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, EndpointKind, MessageRole},
        openai,
    },
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::{json, Value};

pub(in crate::protocol::sap_aicore) fn enabled(payload: &ProviderAccountPayload) -> bool {
    payload.extra_body.as_ref().is_some_and(|settings| {
        settings
            .get(ORCHESTRATION_MODEL)
            .is_some_and(Value::is_string)
            && settings.get(ORCHESTRATION_REASONING) == Some(&Value::Bool(true))
    })
}

pub(crate) fn native_selected(payload: &ProviderAccountPayload, endpoint: EndpointKind) -> bool {
    endpoint == EndpointKind::ChatCompletions && owns_payload(payload) && enabled(payload)
}

pub(super) fn effort(
    req: &CanonicalRelayRequest,
    enabled: bool,
) -> Result<Option<&str>, GatewayError> {
    let invalid = super::invalid;
    if req.reasoning.is_some() {
        return Err(invalid());
    }
    let Some(raw) = req.raw_body.get("reasoning_effort") else {
        return if req.extra.contains_key("reasoning_effort") {
            Err(invalid())
        } else {
            Ok(None)
        };
    };
    if !enabled || req.extra.get("reasoning_effort") != Some(raw) {
        return Err(invalid());
    }
    raw.as_str()
        .filter(|path| {
            matches!(
                *path,
                "none" | "minimal" | "low" | "medium" | "high" | "xhigh"
            )
        })
        .map(Some)
        .ok_or_else(invalid)
}

pub(in crate::protocol::sap_aicore) fn valid_blocks(value: &Value) -> bool {
    value.as_array().is_some_and(|items| {
        items.len() <= 128
            && items.iter().all(|item| {
                item.as_object().is_some_and(|fields| {
                    !fields.is_empty()
                        && fields.iter().all(|(key, text)| {
                            matches!(key.as_str(), "content" | "signature") && text.is_string()
                        })
                })
            })
    })
}

pub(super) fn restore(
    req: &CanonicalRelayRequest,
    history: &mut [Value],
) -> Result<(), GatewayError> {
    let invalid = super::invalid;
    let raw = req.raw_body["messages"].as_array().ok_or_else(invalid)?;
    if !raw
        .iter()
        .any(|message| message.get("reasoning_content").is_some())
    {
        return Ok(());
    }
    let normalized = openai::normalize_chat_completions(req.raw_body.clone())?;
    let mut original = normalized
        .messages
        .iter()
        .filter(|m| m.role != MessageRole::System);
    let mut actual = req
        .messages
        .iter()
        .zip(history)
        .filter(|(m, _)| m.role != MessageRole::System);
    // Refuse to reattach opaque signatures after a non-system history rewrite.
    for message in raw.iter().filter(|m| m["role"] != "system") {
        let expected = original.next().ok_or_else(invalid)?;
        let (current, wire) = actual.next().ok_or_else(invalid)?;
        if openai::pack_openai_message(expected) != openai::pack_openai_message(current) {
            return Err(invalid());
        }
        if let Some(blocks) = message.get("reasoning_content") {
            wire["reasoning_content"] = blocks.clone();
        }
    }
    if original.next().is_some() || actual.next().is_some() {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) async fn execute_native(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    let (response, _) = runtime::send(client, payload, req, model, false).await?;
    decode(response, req).await
}

pub(crate) async fn decode(
    response: rquest::Response,
    req: &CanonicalRelayRequest,
) -> Result<Value, GatewayError> {
    let failed = || super::failure("sap_aicore_response_failed");
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
        response,
        "SAP Orchestration",
    )
    .await
    .map_err(|_| super::failure("sap_aicore_body_incomplete"))?;
    let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| failed())?;
    let result = final_result(&body, !req.tools.is_empty()).ok_or_else(failed)?;
    openai::chat_admission::validate(result).map_err(|_| failed())?;
    let choice = &result["choices"][0];
    let message = &choice["message"];
    let mut reply = json!({"id":result["id"],"object":"chat.completion","created":result["created"],
        "model":result["model"],"choices":[{"index":0,"finish_reason":choice["finish_reason"],
            "message":{"role":"assistant","content":message["content"]}}]});
    if let Some(blocks) = message.get("reasoning_content").filter(|v| !v.is_null()) {
        if !valid_blocks(blocks) {
            return Err(failed());
        }
        reply["choices"][0]["message"]["reasoning_content"] = blocks.clone();
    }
    for key in ["tool_calls", "refusal"] {
        if let Some(value) = message.get(key) {
            reply["choices"][0]["message"][key] = value.clone();
        }
    }
    if let Some(usage) = result.get("usage") {
        reply["usage"] = usage.clone();
    }
    Ok(reply)
}
