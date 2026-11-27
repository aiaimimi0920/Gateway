//! Strict GenerateContent projection. Published IDs, names, argument objects and
//! complete history must match; no inferred calls, remote media or thought signatures.
use super::{
    gemini_tools::{declarations, keys},
    tool_schema::invalid,
};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily},
};
use serde_json::{json, Value};
use std::collections::HashSet;

pub(super) fn project(req: &CanonicalRelayRequest) -> Result<CanonicalRelayRequest, GatewayError> {
    let body = &req.raw_body;
    keys(
        body,
        &[
            "contents",
            "systemInstruction",
            "tools",
            "toolConfig",
            "generationConfig",
        ],
    )?;
    if body
        .get("generationConfig")
        .is_some_and(|v| v != &json!({}))
    {
        return Err(super::request::invalid());
    }
    let mut projected = json!({"model":req.requested_model,"stream":req.stream});
    if let Some(system) = body.get("systemInstruction") {
        keys(system, &["parts"])?;
        projected["instructions"] = json!(text_parts(&system["parts"])?);
    }
    if let Some(tools) = body.get("tools") {
        projected["tools"] = declarations(tools)?;
    }
    if let Some(config) = body.get("toolConfig") {
        keys(config, &["functionCallingConfig"])?;
        let call = &config["functionCallingConfig"];
        keys(call, &["mode"])?;
        projected["tool_choice"] = json!(match call["mode"].as_str() {
            Some("AUTO") => "auto",
            Some("NONE") => "none",
            _ => return Err(invalid()),
        });
    }
    let contents = body["contents"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 128)
        .ok_or_else(invalid)?;
    let mut history = Vec::new();
    let mut pending: Option<(String, String)> = None;
    let mut seen = HashSet::new();
    for content in contents {
        keys(content, &["role", "parts"])?;
        let role = match content.get("role") {
            None => "user",
            Some(value) => match value.as_str() {
                Some("user") => "user",
                Some("model") => "assistant",
                _ => return Err(invalid()),
            },
        };
        let parts = content["parts"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 128)
            .ok_or_else(invalid)?;
        if let Some((id, name)) = pending.take() {
            if role != "user" || parts.len() != 1 {
                return Err(invalid());
            }
            keys(&parts[0], &["functionResponse"])?;
            let output = &parts[0]["functionResponse"];
            keys(output, &["id", "name", "response"])?;
            if output["id"] != id || output["name"] != name || !output["response"].is_object() {
                return Err(invalid());
            }
            history.push(json!({"type":"function_call_output","call_id":id,
                "output":object_text(&output["response"])?}));
            continue;
        }
        let mut message_parts = Vec::new();
        let mut call_item = None;
        for part in parts {
            if call_item.is_some() {
                return Err(invalid()); // One call, after any text prelude.
            }
            if let Some(text) = part.get("text") {
                keys(part, &["text"])?;
                message_parts.push(
                    json!({"type":if role == "user" {"input_text"} else {"output_text"},
                    "text":text.as_str().ok_or_else(invalid)?}),
                );
            } else if let Some(image) = part.get("inlineData") {
                keys(part, &["inlineData"])?;
                keys(image, &["mimeType", "data"])?;
                let mime = image["mimeType"].as_str().ok_or_else(invalid)?;
                if role != "user" || !["image/png", "image/jpeg"].contains(&mime) {
                    return Err(invalid());
                }
                let data = image["data"].as_str().ok_or_else(invalid)?;
                message_parts.push(
                    json!({"type":"input_image","image_url":format!("data:{mime};base64,{data}")}),
                );
            } else if let Some(call) = part.get("functionCall") {
                keys(part, &["functionCall"])?;
                keys(call, &["id", "name", "args"])?;
                if role != "assistant" {
                    return Err(invalid());
                }
                let id = call["id"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty() && s.len() <= 256)
                    .ok_or_else(invalid)?;
                let name = call["name"].as_str().ok_or_else(invalid)?;
                if !seen.insert(id.to_owned()) {
                    return Err(invalid());
                }
                let args = object_text(call.get("args").unwrap_or(&json!({})))?;
                call_item =
                    Some(json!({"type":"function_call","call_id":id,"name":name,"arguments":args}));
                pending = Some((id.into(), name.into()));
            } else {
                return Err(invalid());
            }
        }
        if !message_parts.is_empty() {
            let has_image = message_parts.iter().any(|p| p["type"] == "input_image");
            let text = if has_image {
                json!(message_parts)
            } else {
                json!(super::request::content_with_separator(
                    &json!(message_parts),
                    role == "assistant",
                    ""
                )?)
            };
            if call_item.is_none() || text != "" {
                history.push(json!({"role":role,"content":text}));
            }
        }
        if let Some(call) = call_item {
            history.push(call);
        }
    }
    if pending.is_some() || history.len() > 256 {
        return Err(invalid());
    }
    projected["input"] = json!(history);
    let mut internal = req.clone();
    internal.protocol_family = ProtocolFamily::OpenAi;
    internal.endpoint_kind = EndpointKind::Responses;
    internal.raw_body = projected;
    Ok(internal)
}

fn text_parts(value: &Value) -> Result<String, GatewayError> {
    let parts = value
        .as_array()
        .filter(|a| a.len() <= 128)
        .ok_or_else(invalid)?;
    let mut text = String::new();
    for part in parts {
        keys(part, &["text"])?;
        text.push_str(part["text"].as_str().ok_or_else(invalid)?);
    }
    Ok(text)
}

/// Gemini carries objects rather than argument bytes. Canonicalize only this
/// protocol, recursively, so object key order never invalidates a legitimate echo.
pub(super) fn object_text(value: &Value) -> Result<String, GatewayError> {
    if !value.is_object() || value.to_string().len() > 65536 {
        return Err(invalid());
    }
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), sorted(v)))
                    .collect::<std::collections::BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(array) => Value::Array(array.iter().map(sorted).collect()),
            _ => value.clone(),
        }
    }
    serde_json::to_string(&sorted(value)).map_err(|_| invalid())
}
