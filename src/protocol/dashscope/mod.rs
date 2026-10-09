//! DashScope text/multimodal generation wire contracts; native fields stay native.
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, ProtocolFamily};
use crate::protocol::openai;
use serde_json::{json, Value};

pub mod ingress_stream;
pub mod response;
pub mod stream;
mod stream_accumulator;
#[cfg(test)]
mod tests;

pub const TEXT_PATH: &str = "/services/aigc/text-generation/generation";
pub const MULTIMODAL_PATH: &str = "/services/aigc/multimodal-generation/generation";

pub fn is_dashscope(req: &CanonicalRelayRequest) -> bool {
    matches!(
        req.protocol_family,
        ProtocolFamily::DashScope | ProtocolFamily::DashScopeMultimodal
    )
}

pub fn normalize(
    body: Value,
    multimodal: bool,
    stream: bool,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let input = body
        .get("input")
        .and_then(Value::as_object)
        .ok_or_else(|| GatewayError::bad_request("DashScope requires an input object"))?;
    if body
        .get("model")
        .and_then(Value::as_str)
        .is_none_or(|m| m.trim().is_empty())
    {
        return Err(GatewayError::bad_request("DashScope requires a model"));
    }
    let mut flat = match body.get("parameters") {
        None => json!({}),
        Some(Value::Object(map)) => Value::Object(map.clone()),
        _ => {
            return Err(GatewayError::bad_request(
                "DashScope parameters must be an object",
            ))
        }
    };
    let mut messages = if let Some(messages) = input.get("messages") {
        messages
            .as_array()
            .cloned()
            .ok_or_else(|| GatewayError::bad_request("DashScope input.messages must be an array"))?
    } else if let Some(prompt) = input.get("prompt").and_then(Value::as_str) {
        vec![json!({"role":"user","content":prompt})]
    } else {
        return Err(GatewayError::bad_request(
            "DashScope requires input.messages or input.prompt",
        ));
    };
    for message in &mut messages {
        if let Some(parts) = message.get_mut("content").and_then(Value::as_array_mut) {
            for part in parts.iter_mut() {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    *part = json!({"type":"text","text":text});
                } else if let Some(image) = part.get("image").and_then(Value::as_str) {
                    *part = json!({"type":"image_url","image_url":{"url":image}});
                }
            }
        }
    }
    flat["messages"] = Value::Array(messages);
    flat["model"] = body["model"].clone();
    flat["stream"] = json!(stream);
    for key in ["result_format", "incremental_output"] {
        flat.as_object_mut().unwrap().remove(key);
    }
    let mut req = openai::normalize_chat_completions(flat)?;
    req.protocol_family = if multimodal {
        ProtocolFamily::DashScopeMultimodal
    } else {
        ProtocolFamily::DashScope
    };
    req.raw_body = body;
    Ok(req)
}

/// Unknown native fields may control OCR/search/other provider-owned features.
/// Reject a lossy bridge rather than silently stripping them or leaking them to another vendor.
pub fn validate_bridge(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    if !is_dashscope(req) {
        return Ok(());
    }
    const PARAMETERS: &[&str] = &[
        "temperature",
        "top_p",
        "max_tokens",
        "stop",
        "seed",
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "result_format",
        "incremental_output",
    ];
    let body = &req.raw_body;
    let unknown = body.as_object().is_some_and(|m| {
        m.keys()
            .any(|k| !["model", "input", "parameters"].contains(&k.as_str()))
    }) || body
        .get("parameters")
        .and_then(Value::as_object)
        .is_some_and(|m| m.keys().any(|k| !PARAMETERS.contains(&k.as_str())))
        || body
            .get("input")
            .and_then(Value::as_object)
            .is_some_and(|m| {
                m.keys()
                    .any(|k| !["messages", "prompt"].contains(&k.as_str()))
            })
        || body
            .pointer("/input/messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages.iter().any(|m| {
                    m.as_object().is_none_or(|obj| {
                        obj.keys().any(|k| {
                            !["role", "content", "name", "tool_call_id", "tool_calls"]
                                .contains(&k.as_str())
                        })
                    }) || m
                        .get("content")
                        .and_then(Value::as_array)
                        .is_some_and(|parts| {
                            parts.iter().any(|p| {
                                p.as_object().is_none_or(|obj| {
                                    obj.len() != 1
                                        || !(obj.contains_key("text") || obj.contains_key("image"))
                                })
                            })
                        })
                })
            });
    if unknown {
        return Err(GatewayError::bad_request(
            "DashScope native fields require a matching DashScope upstream",
        )
        .with_code("unsupported_dashscope_conversion"));
    }
    if !req.tools.is_empty()
        && req.protocol_family == ProtocolFamily::DashScope
        && body
            .pointer("/parameters/result_format")
            .and_then(Value::as_str)
            != Some("message")
    {
        return Err(GatewayError::bad_request(
            "DashScope tool conversion requires result_format=message",
        )
        .with_code("unsupported_dashscope_conversion"));
    }
    Ok(())
}

pub fn pack(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
    multimodal: bool,
) -> Result<Value, GatewayError> {
    if is_dashscope(req) {
        let mut body = req.raw_body.clone();
        body["model"] = json!(model);
        return Ok(body);
    }
    if req.previous_response_id.is_some()
        || req
            .raw_body
            .get("conversation")
            .is_some_and(|v| !v.is_null())
        || req.raw_body.get("background").and_then(Value::as_bool) == Some(true)
        || req.tools.iter().any(|tool| tool.tool_type != "function")
    {
        return Err(GatewayError::bad_request(
            "Provider-owned conversation state or tools cannot be converted to DashScope",
        )
        .with_code("unsupported_dashscope_conversion"));
    }
    let chat = openai::pack_openai(req, model, stream);
    let mut messages = chat["messages"].clone();
    if let Some(items) = messages.as_array_mut() {
        for message in items {
            if let Some(text) = message.get("content").and_then(Value::as_str) {
                if multimodal {
                    message["content"] = json!([{"text":text}]);
                }
            } else if let Some(parts) = message.get_mut("content").and_then(Value::as_array_mut) {
                for part in parts.iter_mut() {
                    *part = match part.get("type").and_then(Value::as_str) {
                        Some("text") => json!({"text":part["text"]}),
                        Some("image_url") if multimodal => {
                            json!({"image":part["image_url"]["url"]})
                        }
                        _ => {
                            return Err(GatewayError::bad_request(
                                "Content requires a compatible DashScope multimodal surface",
                            )
                            .with_code("unsupported_dashscope_content"))
                        }
                    };
                }
                if !multimodal {
                    message["content"] = json!(parts
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n"));
                }
            }
        }
    }
    let mut parameters = chat.as_object().cloned().unwrap_or_default();
    for key in ["model", "messages", "stream", "stream_options"] {
        parameters.remove(key);
    }
    if let Some(limit) = parameters
        .remove("max_completion_tokens")
        .or_else(|| parameters.remove("max_output_tokens"))
    {
        parameters.entry("max_tokens").or_insert(limit);
    }
    parameters.insert("result_format".into(), json!("message"));
    parameters.insert("incremental_output".into(), json!(stream));
    Ok(json!({"model":model,"input":{"messages":messages},"parameters":parameters}))
}
