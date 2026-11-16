//! 拒绝通用 canonical 转换无法无损保留的输入；本切片只承诺文本及客户端函数工具。
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, ContentPart, EndpointKind};
use serde_json::Value;

pub(crate) fn validate(
    req: &CanonicalRelayRequest,
    invalid: fn() -> GatewayError,
) -> Result<(), GatewayError> {
    let raw = &req.raw_body;
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Responses
    ) || req.previous_response_id.is_some()
        || req.reasoning.as_ref().is_some_and(|v| !v.is_null())
        || [
            "conversation",
            "previous_response_id",
            "reasoning_effort",
            "thinking",
            "functions",
            "function_call",
            "mcp_servers",
            "context_management",
            "container",
            "audio",
            "web_search_options",
        ]
        .iter()
        .any(|key| raw.get(key).is_some_and(|v| !v.is_null()))
        || raw
            .get("modalities")
            .is_some_and(|v| v != &serde_json::json!(["text"]))
        || raw.get("n").is_some_and(|v| v != 1)
        || ["store", "background"]
            .iter()
            .any(|key| raw.get(key).is_some_and(|v| v == true))
        || ["gateway_vertex_history", "gateway_bedrock_history"]
            .iter()
            .any(|key| raw.get(key).is_some())
        || req.tools.iter().any(|t| t.tool_type != "function")
        || req.messages.iter().any(|m| {
            m.content
                .iter()
                .any(|p| !matches!(p, ContentPart::Text { .. }))
                || m.tool_calls
                    .iter()
                    .any(|call| !matches!(call.call_type.as_str(), "function" | "function_call"))
        })
    {
        return Err(invalid());
    }
    if req.endpoint_kind == EndpointKind::Responses
        && raw
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    !matches!(
                        item.get("type").and_then(Value::as_str),
                        None | Some("message" | "function_call" | "function_call_output")
                    ) || item.get("role").is_some_and(|role| {
                        !matches!(role.as_str(), Some("system" | "user" | "assistant"))
                    }) || ["content", "output"].iter().any(|key| {
                        item.get(key)
                            .and_then(Value::as_array)
                            .is_some_and(|parts| {
                                parts.iter().any(|part| !plain_response_text(part))
                            })
                    })
                })
            })
    {
        return Err(invalid());
    }
    // Anthropic's generic normalizer labels server tools as function and drops tool-result is_error.
    if req.endpoint_kind == EndpointKind::Messages
        && (raw
            .get("system")
            .and_then(Value::as_array)
            .is_some_and(|parts| {
                parts
                    .iter()
                    .any(|part| part["type"] != "text" || !plain_response_text(part))
            })
            || raw
                .get("tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| {
                    tools
                        .iter()
                        .any(|tool| tool.get("type").is_some_and(|kind| kind != "custom"))
                })
            || raw
                .get("messages")
                .and_then(Value::as_array)
                .is_some_and(|messages| {
                    messages.iter().any(|message| {
                        message
                            .get("content")
                            .and_then(Value::as_array)
                            .is_some_and(|parts| {
                                parts.iter().any(|part| {
                                    part["type"] == "tool_result"
                                        && part.get("is_error").is_some_and(|v| v != false)
                                })
                            })
                    })
                }))
    {
        return Err(invalid());
    }
    if raw
        .get("messages")
        .and_then(Value::as_array)
        .is_some_and(|messages| {
            messages.iter().any(|message| {
                message["role"] == "function"
                    || [
                        "reasoning",
                        "reasoning_content",
                        "reasoning_details",
                        "thinking",
                        "function_call",
                        "audio",
                    ]
                    .iter()
                    .any(|key| message.get(key).is_some_and(|v| !v.is_null()))
            })
        })
    {
        return Err(invalid());
    }
    // Legacy Chat normalization repairs HTML entities. Do not silently change modern arguments.
    if req.endpoint_kind == EndpointKind::ChatCompletions {
        let original = raw["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|m| m["tool_calls"].as_array().into_iter().flatten());
        let calls = req.messages.iter().flat_map(|m| &m.tool_calls);
        if original.zip(calls).any(|(raw, call)| {
            raw["function"]["arguments"]
                .as_str()
                .is_some_and(|args| call.arguments.as_deref() != Some(args))
        }) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn plain_response_text(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        object
            .keys()
            .all(|key| matches!(key.as_str(), "type" | "text" | "annotations"))
            // Responses text replies contain an empty annotations array by default.
            && value.get("annotations").is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
            && matches!(
                value["type"].as_str(),
                Some("input_text" | "output_text" | "text")
            )
            && value["text"].is_string()
    })
}
