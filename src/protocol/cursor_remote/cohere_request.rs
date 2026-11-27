//! Cohere v2 text/function subset, checked before the shared Chat history parser.
//! Optional usage and tool_plan remain absent, not synthesized from Cursor thinking.
use super::{gemini_tools::keys, tool_schema::invalid};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, ProtocolFamily},
};
use serde_json::{json, Value};

pub(super) fn project(req: &CanonicalRelayRequest) -> Result<CanonicalRelayRequest, GatewayError> {
    let mut internal = req.clone();
    let body = &mut internal.raw_body;
    keys(
        body,
        &["model", "stream", "messages", "tools", "tool_choice"],
    )?;
    if let Some(choice) = body.get("tool_choice") {
        body["tool_choice"] = json!(match choice.as_str() {
            Some("NONE") => "none",
            _ => return Err(invalid()),
        });
    }
    if let Some(tools) = body.get("tools") {
        for tool in tools
            .as_array()
            .filter(|a| a.len() <= 64)
            .ok_or_else(invalid)?
        {
            keys(tool, &["type", "function"])?;
            keys(&tool["function"], &["name", "description", "parameters"])?;
            if tool["type"] != "function" {
                return Err(invalid());
            }
        }
    }
    let messages = body["messages"]
        .as_array_mut()
        .filter(|a| a.len() <= 256)
        .ok_or_else(invalid)?;
    for message in messages {
        let role = message["role"].as_str().ok_or_else(invalid)?;
        match role {
            "tool" => {
                keys(message, &["role", "content", "tool_call_id"])?;
                if let Some(text) = message["content"].as_str() {
                    // Cohere string tool results are JSON objects; text blocks
                    // are the explicit lane for arbitrary text. Preserve bytes.
                    if !serde_json::from_str::<Value>(text).is_ok_and(|v| v.is_object()) {
                        return Err(invalid());
                    }
                }
            }
            "assistant" => {
                keys(message, &["role", "content", "tool_calls"])?;
                if let Some(calls) = message.get("tool_calls") {
                    for call in calls.as_array().ok_or_else(invalid)? {
                        if call["type"] != "function" {
                            return Err(invalid());
                        }
                    }
                }
            }
            "system" | "user" => keys(message, &["role", "content"])?,
            _ => return Err(invalid()),
        }
        if let Some(content) = message.get("content").filter(|v| !v.is_null()) {
            // Reject image/document blocks even when Chat would accept images.
            super::text_output::parse(content, "text")?;
        }
    }
    internal.protocol_family = ProtocolFamily::OpenAi;
    super::chat_request::project(&internal)
}
