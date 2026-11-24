//! Fixed Token Plan Responses endpoint; exclusions are checked on the wire, not inferred from aliases.
use super::*;
use crate::protocol::responses::native_admission::valid_terminal;
use serde_json::Value;

pub(super) fn supports_model(model: &str) -> bool {
    matches!(
        model,
        "sensenova-6.8-flash-lite" | "deepseek-flash" | "kimi-k3"
    )
}

pub(crate) fn selected(payload: &ProviderAccountPayload, endpoint: EndpointKind) -> bool {
    owns_payload(payload) && endpoint == EndpointKind::Responses
}

pub(crate) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    validate_request(payload, req)?;
    // Raw IDs of the wrong type must not be silently removed by canonical packing.
    for key in [
        "previous_response_id",
        "conversation",
        "context_management",
        "prompt",
    ] {
        if !req.raw_body[key].is_null() {
            return Err(invalid());
        }
    }
    let body = crate::protocol::native_text_request::pack(req, model, stream, invalid)?;
    validate_wire(&body, model)?;
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{BASE}/responses"),
        query: vec![],
        body: Some(body),
        response_kind: EndpointKind::Responses,
    };
    validate_plan(payload, &plan)?;
    Ok(plan)
}

fn validate_wire(body: &Value, model: &str) -> Result<(), GatewayError> {
    if !supports_model(model) {
        return Err(GatewayError::bad_request(
            "This model is not supported by SenseNova Responses.",
        )
        .with_code("sensenova_responses_model_unsupported"));
    }
    if ["store", "background"]
        .iter()
        .any(|key| body.get(key).is_some_and(|v| v != false))
        || [
            "previous_response_id",
            "conversation",
            "context_management",
            "prompt",
            "mcp_servers",
        ]
        .iter()
        .any(|key| !body[key].is_null())
        || body.get("tools").is_some_and(|tools| {
            tools
                .as_array()
                .is_none_or(|tools| tools.iter().any(|tool| tool["type"] != "function"))
        })
        || !tool_choice_supported(&body["tool_choice"])
    {
        return Err(invalid());
    }
    if body["text"]["format"]["type"] == "json_schema" && model == "sensenova-6.8-flash-lite" {
        return Err(GatewayError::bad_request(
            "This SenseNova Responses model does not support json_schema.",
        )
        .with_code("sensenova_responses_format_unsupported"));
    }
    match &body["input"] {
        Value::String(_) => Ok(()),
        Value::Array(items) if items.iter().all(input_item_supported) => Ok(()),
        _ => Err(invalid()),
    }
}

fn tool_choice_supported(choice: &Value) -> bool {
    choice.is_null()
        || matches!(choice.as_str(), Some("auto" | "required" | "none"))
        || (choice["type"] == "function" && nonempty(&choice["name"]))
}

fn input_item_supported(item: &Value) -> bool {
    match item.get("type") {
        Some(kind) if kind == "function_call" => {
            nonempty(&item["call_id"]) && nonempty(&item["name"]) && item["arguments"].is_string()
        }
        Some(kind) if kind == "function_call_output" => {
            nonempty(&item["call_id"]) && content_supported(&item["output"])
        }
        Some(kind) if kind == "reasoning" => true,
        None | Some(Value::Null) => nonempty(&item["role"]) && content_supported(&item["content"]),
        Some(kind) if kind == "message" => {
            nonempty(&item["role"]) && content_supported(&item["content"])
        }
        _ => false,
    }
}

fn content_supported(content: &Value) -> bool {
    content.is_string()
        || content.as_array().is_some_and(|parts| {
            parts.iter().all(|part| {
                match part["type"].as_str() {
                    Some("input_text" | "output_text") => part["text"].is_string(),
                    Some("refusal") => part["refusal"].is_string(),
                    // Gateway sends only the reference to the fixed provider; it does not fetch it.
                    Some("input_image") => {
                        part["file_id"].is_null()
                            && part["image_url"].as_str().is_some_and(|url| {
                                [
                                    "data:image/png;base64,",
                                    "data:image/jpeg;base64,",
                                    "data:image/gif;base64,",
                                    "data:image/webp;base64,",
                                ]
                                .into_iter()
                                .any(|prefix| {
                                    url.strip_prefix(prefix)
                                        .is_some_and(|data| !data.is_empty())
                                }) || url::Url::parse(url).is_ok_and(|u| {
                                    matches!(u.scheme(), "https" | "http") && u.has_host()
                                })
                            })
                    }
                    _ => false,
                }
            })
        })
}

fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.trim().is_empty())
}

pub(crate) async fn read(response: rquest::Response) -> Result<Value, GatewayError> {
    let bytes = crate::protocol::upstream_body::collect_bounded_upstream_body(
        response,
        "SenseNova Responses",
    )
    .await
    .map_err(|_| response_error())?;
    let body = crate::protocol::unique_json::parse(&bytes).map_err(|_| response_error())?;
    if body["object"] != "response" || !body["output"].is_array() || !valid_terminal(&body) {
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
    E: From<crate::protocol::stream_error::ProtocolStreamError> + Send + 'static,
{
    let mut id = None::<String>;
    crate::protocol::responses::native_stream::guard(
        stream,
        model,
        false,
        move |value, kind, terminal| {
            let body = &value["response"];
            if kind == "response.created" {
                if id.is_some() {
                    return false;
                }
                id = body["id"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_owned);
                return id.is_some();
            }
            // The documented abbreviated terminal can omit object/model/output, but not identity/status.
            !terminal || (id.as_deref().is_some_and(|id| body["id"] == id) && valid_terminal(body))
        },
        || {
            crate::protocol::stream_error::ProtocolStreamError::classified(
                "invalid_sensenova_stream: incomplete or invalid Responses stream",
                response_error(),
            )
        },
    )
}
