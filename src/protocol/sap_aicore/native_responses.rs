//! Native SAP Responses uses the fixed deployment, never a model-derived address or remote state.
use super::*;
use crate::{
    protocol::{
        canonical::{CanonicalRelayRequest, EndpointKind},
        responses::{native_admission::valid_terminal, native_stream},
        stream_error::ProtocolStreamError,
    },
    upstream::{client::UpstreamClient, common::RequestPlan},
};
use serde_json::Value;

pub(crate) fn selected(payload: &ProviderAccountPayload, endpoint: EndpointKind) -> bool {
    endpoint == EndpointKind::Responses
        && owns_payload(payload)
        && payload
            .extra_body
            .as_ref()
            .and_then(|v| v.get(NATIVE_RESPONSES))
            == Some(&Value::Bool(true))
}

pub(super) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    // Check ingress and final wire: a canonical removal must not hide an unsupported state request.
    if !stateless(&req.raw_body) || req.previous_response_id.is_some() {
        return Err(invalid());
    }
    let mut body = crate::protocol::native_text_request::pack(req, model, stream, invalid)?;
    let images_enabled = payload
        .extra_body
        .as_ref()
        .and_then(|settings| settings.get(NATIVE_RESPONSES_IMAGE_INPUT))
        == Some(&Value::Bool(true));
    if !stateless(&body) || !supported(&body, images_enabled) {
        return Err(invalid());
    }
    let fields = body.as_object_mut().ok_or_else(invalid)?;
    // SAP defaults to persistence. Always opt out, even if the caller omitted `store`.
    fields.insert("store".into(), false.into());
    for field in ["model", "previous_response_id", "conversation"] {
        fields.remove(field);
    }
    Ok(responses_plan(
        payload,
        rquest::Method::POST,
        "",
        Some(body),
    ))
}

pub(super) fn responses_plan(
    payload: &ProviderAccountPayload,
    method: rquest::Method,
    suffix: &str,
    body: Option<Value>,
) -> RequestPlan {
    RequestPlan {
        method,
        url: format!("{}/responses{suffix}", payload.base_url),
        // Native Responses uses the deployment URL, not Chat's date-based query.
        query: vec![],
        body,
        response_kind: EndpointKind::Responses,
    }
}

fn stateless(body: &Value) -> bool {
    ["store", "background"]
        .into_iter()
        .all(|k| body.get(k).is_none_or(|v| v == false))
        && ["previous_response_id", "conversation"]
            .into_iter()
            .all(|k| body[k].is_null())
}

pub(super) fn supported(body: &Value, images_enabled: bool) -> bool {
    let mut images = images_enabled.then(image_input::InlineImages::new);
    body.as_object().is_some_and(|fields| {
        fields.keys().all(|k| {
            matches!(
                k.as_str(),
                "model"
                    | "input"
                    | "instructions"
                    | "stream"
                    | "store"
                    | "background"
                    | "previous_response_id"
                    | "conversation"
                    | "tools"
                    | "tool_choice"
                    | "parallel_tool_calls"
                    | "reasoning"
                    | "include"
                    | "text"
                    | "max_output_tokens"
                    | "temperature"
                    | "top_p"
                    | "metadata"
                    | "user"
                    | "truncation"
            )
        })
    }) && body.get("include").is_none_or(|v| {
        v.as_array().is_some_and(|items| {
            items
                .iter()
                .all(|item| item == "reasoning.encrypted_content")
        })
    }) && body.get("tools").is_none_or(|v| {
        v.as_array().is_some_and(|items| {
            items
                .iter()
                .all(|tool| tool["type"] == "function" && nonempty(&tool["name"]))
        })
    }) && tool_choice(&body["tool_choice"])
        && (body["input"].is_string()
            || body["input"].as_array().is_some_and(|items| {
                !items.is_empty() && items.iter().all(|item| input_item(item, &mut images))
            }))
}

fn nonempty(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
}

fn tool_choice(v: &Value) -> bool {
    v.is_null()
        || matches!(v.as_str(), Some("auto" | "none" | "required"))
        || (v["type"] == "function" && nonempty(&v["name"]))
}

fn input_item(item: &Value, images: &mut Option<image_input::InlineImages>) -> bool {
    match item["type"].as_str() {
        Some("function_call") => {
            nonempty(&item["call_id"]) && nonempty(&item["name"]) && item["arguments"].is_string()
        }
        Some("function_call_output") => {
            nonempty(&item["call_id"]) && input_content(&item["output"], None)
        }
        Some("reasoning") => nonempty(&item["encrypted_content"]),
        kind if kind == Some("message") || item["type"].is_null() => {
            matches!(
                item["role"].as_str(),
                Some("user" | "assistant" | "system" | "developer")
            ) && input_content(
                &item["content"],
                if item["role"] == "user" {
                    images.as_mut()
                } else {
                    None
                },
            )
        }
        _ => false,
    }
}

fn input_content(value: &Value, mut images: Option<&mut image_input::InlineImages>) -> bool {
    value.is_string()
        || value.as_array().is_some_and(|parts| {
            parts.iter().all(|part| match part["type"].as_str() {
                Some("input_text" | "output_text") => part["text"].is_string(),
                Some("refusal") => part["refusal"].is_string(),
                // Reject remote URL/file references and over-budget requests before OAuth.
                Some("input_image") => images
                    .as_deref_mut()
                    .is_some_and(|budget| budget.admit_responses_part(part)),
                _ => false,
            })
        })
}

fn response_error() -> GatewayError {
    failure("sap_aicore_response_failed")
}

fn valid_response(body: &Value) -> bool {
    body["object"] == "response"
        && body["output"].is_array()
        && stateless(body)
        && valid_terminal(body)
}

pub(crate) async fn execute(
    client: &UpstreamClient,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    if !selected(payload, req.endpoint_kind) {
        return Err(invalid());
    }
    let (response, _) = runtime::send(client, payload, req, model, false).await?;
    decode(response).await
}

pub(crate) async fn decode(response: rquest::Response) -> Result<Value, GatewayError> {
    let bytes =
        crate::protocol::upstream_body::collect_bounded_upstream_body(response, "SAP Responses")
            .await
            .map_err(|_| failure("sap_aicore_body_incomplete"))?;
    let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    if !valid_response(&body) {
        return Err(response_error());
    }
    Ok(body)
}

pub(crate) fn guard<S, E>(
    stream: S,
    model: String,
) -> impl futures::Stream<Item = Result<bytes::Bytes, E>> + Send
where
    S: futures::Stream<Item = Result<bytes::Bytes, E>> + Send + 'static,
    E: From<ProtocolStreamError> + Send + 'static,
{
    let mut id: Option<String> = None;
    native_stream::guard(
        stream,
        model,
        false,
        move |value, kind, terminal| {
            let body = &value["response"];
            if kind == "response.created" {
                if id.is_some()
                    || !nonempty(&body["id"])
                    || body["object"] != "response"
                    || !stateless(body)
                {
                    return false;
                }
                id = body["id"].as_str().map(str::to_owned);
                return true;
            }
            id.as_deref().is_some_and(|id| {
                if terminal {
                    body["id"] == id && valid_response(body)
                } else if matches!(kind, "response.in_progress" | "response.queued") {
                    body["id"] == id && stateless(body)
                } else {
                    true
                }
            })
        },
        || {
            ProtocolStreamError::classified(
                "invalid_sap_aicore_stream: incomplete or invalid Responses stream",
                response_error(),
            )
        },
    )
}
