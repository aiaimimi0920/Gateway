//! Transcript/header contract after inline media extraction; no silent field loss.
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
use serde_json::{json, Value};

const MAX_PROMPT: usize = 64 * 1024;
pub(super) fn invalid() -> GatewayError {
    crate::routing::config::cursor::stop(
        GatewayError::bad_request("Cursor remote request contains unsupported fields or exceeds its bounded input contract")
            .with_code("cursor_remote_unsupported_request"),
    )
}

pub(crate) fn prompt(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    prompt_with_images(req, false)
}

pub(super) fn prompt_with_images(
    req: &CanonicalRelayRequest,
    images: bool,
) -> Result<String, GatewayError> {
    prompt_with_capabilities(req, images, false)
}

pub(super) fn prompt_with_capabilities(
    req: &CanonicalRelayRequest,
    images: bool,
    tools: bool,
) -> Result<String, GatewayError> {
    if req.protocol_family != ProtocolFamily::OpenAi
        || req.endpoint_kind != EndpointKind::Responses
        || req.requested_model.as_deref() != Some(crate::routing::config::cursor::MODEL)
        || !req.tools.is_empty()
        || req.previous_response_id.is_some()
    {
        return Err(invalid());
    }
    let body = req.raw_body.as_object().ok_or_else(invalid)?;
    for (key, value) in body {
        let allowed = match key.as_str() {
            "model" | "input" | "instructions" => true,
            "stream" => value.as_bool() == Some(req.stream),
            "store" | "background" => value == false,
            "tools" => value.as_array().is_some_and(Vec::is_empty),
            // Codex asks for an optional encrypted field even with reasoning disabled.
            // Cursor supplies no encrypted state; this optional projection stays absent.
            "include" => value.as_array().is_some_and(|items| {
                items.is_empty()
                    || (items.as_slice() == [json!("reasoning.encrypted_content")]
                        && body.get("reasoning") == Some(&json!({"effort":"none"})))
            }),
            "tool_choice" => value == "none" || value == "auto",
            "parallel_tool_calls" => value.is_boolean(), // no tools; no scheduling effect
            "reasoning" => value.is_null() || value == &json!({"effort":"none"}),
            "text" => value == &json!({"format":{"type":"text"}}),
            // These identify cache/session metadata, not stored continuation.
            "prompt_cache_key" => value.as_str().is_some_and(|s| s.len() <= 256),
            // Opaque caller telemetry stays at Gateway; never sent as Cursor identity.
            "client_metadata" => value.as_object().is_some_and(|m| {
                m.len() <= 32
                    && m.iter()
                        .all(|(k, v)| k.len() <= 128 && v.as_str().is_some_and(|s| s.len() <= 4096))
            }),
            _ => false,
        };
        if !allowed {
            return Err(invalid());
        }
    }
    let mut messages = Vec::new();
    if let Some(instructions) = body.get("instructions") {
        let text = instructions.as_str().ok_or_else(invalid)?;
        if !text.is_empty() {
            messages.push(json!({"role":"system","content":text}));
        }
    }
    match body.get("input") {
        Some(Value::String(text)) => messages.push(json!({"role":"user","content":text})),
        Some(Value::Array(items)) if items.len() <= 128 => {
            for item in items {
                let object = item.as_object().ok_or_else(invalid)?;
                if object
                    .keys()
                    .any(|key| !["type", "id", "role", "content"].contains(&key.as_str()))
                    || item.get("type").is_some_and(|v| v != "message")
                    || item
                        .get("id")
                        .is_some_and(|v| v.as_str().is_none_or(|s| s.len() > 256))
                {
                    return Err(invalid());
                }
                let role = item["role"].as_str().ok_or_else(invalid)?;
                if !["system", "developer", "user", "assistant"].contains(&role) {
                    return Err(invalid());
                }
                let text = content(&item["content"], role == "assistant")?;
                messages.push(json!({"role":role,"content":text}));
            }
        }
        _ => return Err(invalid()),
    }
    if messages.is_empty()
        || messages.last().is_none_or(|m| m["role"] != "user")
        || (!images
            && messages
                .iter()
                .all(|m| m["content"].as_str().is_none_or(|s| s.trim().is_empty())))
    {
        return Err(invalid());
    }
    // ASK has one user-text field, not role-native history. Preserve every message
    // and instruction as a clearly labelled transcript; do not claim native role priority.
    let media = if images {
        "The attached images belong to the final user message."
    } else if tools {
        ""
    } else {
        "This is a text-only exchange;"
    };
    let policy = if tools {
        "The client owns the workspace. Use only the declared gateway MCP functions for all operations; do not use server-side shell, file or web tools."
    } else {
        "do not call tools."
    };
    let prompt = format!(
        "Answer the final user message in this JSON conversation. Respect system and developer instructions. {media} {policy}\n{}",
        serde_json::to_string(&messages).map_err(|_| invalid())?
    );
    if prompt.len() > MAX_PROMPT {
        return Err(invalid());
    }
    Ok(prompt)
}

pub(super) fn content(value: &Value, assistant: bool) -> Result<String, GatewayError> {
    content_with_separator(value, assistant, "\n")
}

pub(super) fn content_with_separator(
    value: &Value,
    assistant: bool,
    separator: &str,
) -> Result<String, GatewayError> {
    if let Some(text) = value.as_str() {
        return Ok(text.into());
    }
    let parts = value
        .as_array()
        .filter(|p| p.len() <= 128)
        .ok_or_else(invalid)?;
    let mut text = Vec::new();
    for part in parts {
        let map = part.as_object().ok_or_else(invalid)?;
        if map.len() != 2
            || !map.contains_key("type")
            || !map.contains_key("text")
            || part["type"]
                != if assistant {
                    "output_text"
                } else {
                    "input_text"
                }
        {
            return Err(invalid());
        }
        text.push(part["text"].as_str().ok_or_else(invalid)?);
    }
    Ok(text.join(separator))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(value: Value) -> Result<String, GatewayError> {
        prompt(&crate::protocol::responses::normalize_responses(value).unwrap())
    }
    #[test]
    fn cursor_remote_request_retains_instructions_and_history() {
        let p = parse(
            json!({"model":"cursor-personal","instructions":"system note","store":false,
            "input":[{"role":"developer","content":[{"type":"input_text","text":"developer note"}]},
            {"role":"assistant","content":[{"type":"output_text","text":"previous reply"}]},
            {"role":"user","content":"final question"}]}),
        )
        .unwrap();
        for value in [
            "system note",
            "developer note",
            "previous reply",
            "final question",
        ] {
            assert!(p.contains(value));
        }
    }
    #[test]
    fn cursor_remote_request_accepts_codex_text_envelope_without_tools() {
        let value = json!({"model":"cursor-personal","instructions":"system","store":false,"stream":true,
            "tools":[],"tool_choice":"auto","parallel_tool_calls":true,"reasoning":{"effort":"none"},
            "include":["reasoning.encrypted_content"],"prompt_cache_key":"fixture",
            "client_metadata":{"thread_id":"fixture"},
            "input":[{"type":"message","id":"msg_fixture","role":"user",
                "content":[{"type":"input_text","text":"hello"}]}]});
        assert!(parse(value).unwrap().contains("hello"));
    }
    #[test]
    fn cursor_remote_request_rejects_unmapped_semantics() {
        let base = json!({"model":"cursor-personal","input":"hello","store":false});
        assert!(parse(base.clone()).is_ok());
        for (key, value) in [
            ("store", json!(true)),
            ("tools", json!([{"type":"function","name":"x"}])),
            ("reasoning", json!({"effort":"high"})),
            ("max_output_tokens", json!(10)),
            ("previous_response_id", json!("resp_gw_old")),
            ("temperature", json!(1)),
            ("include", json!(["reasoning.encrypted_content"])),
            ("unknown", json!(true)),
        ] {
            let mut b = base.clone();
            b[key] = value;
            assert!(parse(b).is_err(), "{key}");
        }
        for input in [
            json!([{"role":"user","content":[{"type":"input_image","image_url":"x"}]}]),
            json!([{"type":"function_call_output","call_id":"x","output":"x"}]),
            json!([{"role":"user","content":[{"type":"input_text","text":"x","unknown":true}]}]),
            json!("x".repeat(MAX_PROMPT)),
        ] {
            let mut b = base.clone();
            b["input"] = input;
            assert!(parse(b).is_err());
        }
    }
}
