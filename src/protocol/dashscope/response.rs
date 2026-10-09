//! Lossless native responses bypass this bridge; cross-family output is checked.
use crate::error::GatewayError;
use crate::protocol::{canonical::CanonicalRelayResponse, openai};
use serde_json::{json, Value};

pub fn to_openai(body: &Value, model: &str, delta: bool) -> Result<Value, GatewayError> {
    if body
        .get("code")
        .and_then(Value::as_str)
        .is_some_and(|v| !v.is_empty())
    {
        return Err(
            GatewayError::server_error("DashScope returned a protocol error")
                .with_code("dashscope_upstream_error"),
        );
    }
    let output = body
        .get("output")
        .ok_or_else(|| GatewayError::server_error("Missing DashScope output"))?;
    let choices = if let Some(choices) = output.get("choices").and_then(Value::as_array) {
        let mut result = Vec::new();
        for (index, choice) in choices.iter().enumerate() {
            let mut message = choice.get("message").cloned().unwrap_or_else(|| json!({}));
            if let Some(parts) = message.get("content").and_then(Value::as_array) {
                if parts.iter().any(|p| {
                    p.as_object()
                        .is_none_or(|m| m.len() != 1 || !m.contains_key("text"))
                }) {
                    return Err(GatewayError::bad_request(
                        "DashScope structured output requires a native DashScope client",
                    )
                    .with_code("unsupported_dashscope_output"));
                }
                message["content"] = json!(parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join(""));
            }
            let reason = match choice.get("finish_reason").and_then(Value::as_str) {
                Some("null") | None => Value::Null,
                _ => choice["finish_reason"].clone(),
            };
            let mut item = json!({"index":index,"finish_reason":reason});
            item[if delta { "delta" } else { "message" }] = message;
            result.push(item);
        }
        result
    } else if let Some(text) = output.get("text").and_then(Value::as_str) {
        let mut item = json!({"index":0,"finish_reason":output.get("finish_reason").filter(|v| v.as_str() != Some("null")).cloned().unwrap_or(Value::Null)});
        item[if delta { "delta" } else { "message" }] = json!({"role":"assistant","content":text});
        vec![item]
    } else {
        return Err(GatewayError::server_error(
            "Invalid DashScope generation output",
        ));
    };
    let mut result = json!({"id":body.get("request_id").cloned().unwrap_or(json!("dashscope-gateway")),"object":if delta {"chat.completion.chunk"} else {"chat.completion"},"model":model,"choices":choices});
    if let Some(usage) = body.get("usage") {
        let input = usage["input_tokens"].as_u64().unwrap_or(0);
        let output = usage["output_tokens"].as_u64().unwrap_or(0);
        result["usage"] = json!({"prompt_tokens":input,"completion_tokens":output,"total_tokens":usage["total_tokens"].as_u64().unwrap_or(input.saturating_add(output))});
    }
    Ok(result)
}

pub fn unpack(body: &Value, model: &str) -> Result<CanonicalRelayResponse, GatewayError> {
    openai::unpack_openai_response(&to_openai(body, model, false)?)
}

pub fn observe_native(body: &Value, model: &str) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut observation = body.clone();
    if let Some(choices) = observation
        .pointer_mut("/output/choices")
        .and_then(Value::as_array_mut)
    {
        for choice in choices {
            // Only accounting uses this projection; the native payload stays untouched.
            if choice
                .pointer("/message/content")
                .is_some_and(Value::is_array)
            {
                choice["message"]["content"] = json!("");
            }
        }
    }
    unpack(&observation, model)
}

pub fn from_openai(
    body: &Value,
    multimodal: bool,
    message_format: bool,
) -> Result<Value, GatewayError> {
    if body.get("output").is_some() {
        return Ok(body.clone());
    }
    if !multimodal
        && !message_format
        && body["choices"].as_array().is_some_and(|choices| {
            choices.len() > 1
                || choices.iter().any(|c| {
                    c.get("message")
                        .or_else(|| c.get("delta"))
                        .and_then(Value::as_object)
                        .is_some_and(|m| {
                            m.iter().any(|(k, v)| {
                                !["role", "content"].contains(&k.as_str()) && !v.is_null()
                            })
                        })
                })
        })
    {
        return Err(GatewayError::bad_request(
            "Structured completion requires DashScope result_format=message",
        )
        .with_code("unsupported_dashscope_output"));
    }
    let choices = body["choices"].as_array().map(|items| items.iter().map(|choice| {
        let mut message = choice.get("message").or_else(|| choice.get("delta")).cloned().unwrap_or(json!({}));
        if multimodal {
            if let Some(text) = message.get("content").and_then(Value::as_str) {
                message["content"] = json!([{"text":text}]);
            }
        }
        json!({"message":message,"finish_reason":choice.get("finish_reason").filter(|v| !v.is_null()).cloned().unwrap_or(json!("null"))})
    }).collect::<Vec<_>>()).unwrap_or_default();
    let output = if message_format || multimodal {
        json!({"choices":choices})
    } else {
        json!({"text":choices.first().and_then(|c| c.pointer("/message/content")).cloned().unwrap_or(json!("")),"finish_reason":choices.first().map(|c| c["finish_reason"].clone()).unwrap_or(json!("null"))})
    };
    let mut result = json!({"request_id":body.get("id").cloned().unwrap_or(json!("dashscope-gateway")),"output":output});
    if let Some(usage) = body.get("usage").filter(|v| v.is_object()) {
        let mut mapped = serde_json::Map::new();
        for (source, target) in [
            ("prompt_tokens", "input_tokens"),
            ("completion_tokens", "output_tokens"),
            ("total_tokens", "total_tokens"),
        ] {
            if let Some(value) = usage.get(source).and_then(Value::as_u64) {
                mapped.insert(target.into(), json!(value));
            }
        }
        result["usage"] = Value::Object(mapped);
    }
    Ok(result)
}
