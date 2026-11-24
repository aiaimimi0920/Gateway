//! SAP Orchestration v2 Chat. The configured model, not the public alias, selects execution.
use super::*;
use crate::{
    protocol::canonical::{CanonicalRelayRequest, EndpointKind},
    upstream::common::RequestPlan,
};
use serde_json::{json, Value};

#[path = "orchestration/reasoning.rs"]
pub(super) mod reasoning;
#[path = "orchestration/tools.rs"]
pub(super) mod tools;
pub(crate) use reasoning::{decode as decode_native, execute_native, native_selected};

pub(super) fn valid_model(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

pub(super) fn model(payload: &ProviderAccountPayload) -> Option<&str> {
    payload
        .extra_body
        .as_ref()?
        .get(ORCHESTRATION_MODEL)?
        .as_str()
}

pub(super) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    let raw = req.raw_body.as_object().ok_or_else(invalid)?;
    if req.endpoint_kind != EndpointKind::ChatCompletions
        || raw.keys().any(|key| {
            !matches!(
                key.as_str(),
                "model"
                    | "messages"
                    | "stream"
                    | "stream_options"
                    | "reasoning_effort"
                    | "tools"
                    | "max_tokens"
                    | "max_completion_tokens"
                    | "n"
                    | "user"
            )
        })
        || raw
            .get("stream")
            .is_some_and(|value| value.as_bool() != Some(stream))
        || raw
            .get("stream_options")
            .is_some_and(|options| !stream || options != &json!({"include_usage":true}))
        || req.tool_choice.is_some()
        || raw.get("n").is_some_and(|value| value.as_u64() != Some(1))
        || req.extra.keys().any(|key| {
            !matches!(
                key.as_str(),
                "max_tokens"
                    | "max_completion_tokens"
                    | "n"
                    | "stream_options"
                    | "reasoning_effort"
            )
        })
    {
        return Err(invalid());
    }
    let definitions = tools::definitions(req)?;
    let images = payload
        .extra_body
        .as_ref()
        .and_then(|settings| settings.get(ORCHESTRATION_IMAGE_INPUT))
        == Some(&Value::Bool(true));
    if images {
        image_input::validate(req, true)?;
    }
    let enabled = reasoning::enabled(payload);
    let mut history = tools::history(req, images, enabled)?;
    reasoning::restore(req, &mut history)?;
    let mut selection = json!({"name":model});
    match (
        req.extra.get("max_tokens"),
        req.extra.get("max_completion_tokens"),
    ) {
        (Some(_), Some(_)) => return Err(invalid()),
        (Some(value), None) | (None, Some(value)) => {
            let limit = value
                .as_u64()
                .filter(|n| *n > 0 && *n <= u32::MAX.into())
                .ok_or_else(invalid)?;
            selection["params"] = json!({"max_completion_tokens":limit});
        }
        (None, None) => {}
    }
    if raw.contains_key("stream_options") {
        selection["params"]["stream_options"] = json!({"include_usage":true});
    }
    if let Some(effort) = reasoning::effort(req, enabled)? {
        selection["params"]["reasoning_effort"] = json!(effort);
    }
    let body = tools::request_body(selection, history, definitions, stream)?;
    Ok(RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/v2/completion", payload.base_url),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::ChatCompletions,
    })
}

pub(super) fn final_result(body: &Value, allow_tools: bool) -> Option<&Value> {
    if !body
        .get("request_id")?
        .as_str()
        .is_some_and(|id| !id.is_empty())
        || body.get("error").is_some_and(|v| !v.is_null())
    {
        return None;
    }
    let result = body.get("final_result")?;
    let [choice] = result.get("choices")?.as_array()?.as_slice() else {
        return None;
    };
    let message = choice.get("message")?;
    tools::valid_final_calls(message, &choice["finish_reason"], allow_tools).then_some(result)
}
