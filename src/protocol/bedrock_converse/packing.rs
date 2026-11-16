//! Canonical request packing for the Bedrock Converse wire format.

use super::{generation, generation_validation, history, messages, public_history};
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::canonical::CanonicalTool;
use crate::protocol::tool_choice;
use serde_json::json;
use serde_json::Value;

/// Structural compatibility API; network admission uses the fallible packer below.
pub fn pack_bedrock_converse(req: &CanonicalRelayRequest, _model: &str, _stream: bool) -> Value {
    let translated = generation::for_target(req);
    // This legacy API is structural, not admission. On a history conflict it keeps
    // the canonical projection; it must never restore raw data over policy edits.
    let original = match history::original_if_unchanged(translated.as_ref()) {
        Ok(original) => original,
        Err(_) => None,
    };
    pack_prepared(translated.as_ref(), original)
}

pub fn try_pack_bedrock_converse(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<Value, GatewayError> {
    let mut body = try_pack_for_plan(req, model, stream)?;
    finish_plan(req, &mut body, model)?;
    Ok(body)
}

// Payload defaults and fingerprint transformations run before the public history check.
pub(crate) fn try_pack_for_plan(
    req: &CanonicalRelayRequest,
    _model: &str,
    _stream: bool,
) -> Result<Value, GatewayError> {
    // Check before translation so an alias coalescer cannot hide conflicting caller values.
    generation_validation::validate_request(req)?;
    let translated = generation::for_target(req);
    generation_validation::validate_request(translated.as_ref())?;
    let original = history::original_if_unchanged(translated.as_ref())?;
    let body = pack_prepared(translated.as_ref(), original);
    generation_validation::validate_body(&body)?;
    history::validate_body(&body)?;
    Ok(body)
}

pub(crate) fn finish_plan(
    req: &CanonicalRelayRequest,
    body: &mut Value,
    model: &str,
) -> Result<(), GatewayError> {
    public_history::apply(req, body, model)?;
    generation_validation::validate_body(body)?;
    history::validate_body(body)
}

fn pack_prepared(req: &CanonicalRelayRequest, original: Option<&Value>) -> Value {
    let mut body = json!({
        "messages": match original {
            Some(original) => original["messages"].clone(),
            None => json!(messages::build(req)),
        },
    });

    if let Some(original) = original {
        if let Some(system) = original.get("system") {
            body["system"] = system.clone();
        }
    } else if let Some(system) = req.system_message().filter(|value| !value.is_empty()) {
        body["system"] = json!([{ "text": system }]);
    }

    if !req.tools.is_empty() || req.tool_choice.is_some() {
        let mut tool_config = json!({});
        if !req.tools.is_empty() {
            tool_config["tools"] = json!(req.tools.iter().map(pack_tool).collect::<Vec<_>>());
        }
        if let Some(tool_choice) = pack_tool_choice(req.tool_choice.as_ref()) {
            tool_config["toolChoice"] = tool_choice;
        }
        body["toolConfig"] = tool_config;
    }

    // Model and stream mode belong to the HTTP path, not the AWS JSON document.
    for (key, value) in &req.extra {
        if matches!(
            key.as_str(),
            "modelId"
                | "stream"
                | "inferenceConfig"
                | "gateway_bedrock_history"
                | "gateway_bedrock_turn"
        ) || generation::is_alias(key)
        {
            continue;
        }
        body[key] = value.clone();
    }

    generation::apply(req, &mut body);
    body
}

fn pack_tool(tool: &CanonicalTool) -> Value {
    json!({
        "toolSpec": {
            "name": tool.name,
            "description": tool.description,
            "inputSchema": {
                "json": tool
                    .input_schema
                    .clone()
                    .unwrap_or_else(|| json!({"type":"object","properties":{}}))
            }
        }
    })
}

fn pack_tool_choice(tool_choice: Option<&Value>) -> Option<Value> {
    tool_choice::pack_bedrock_tool_choice(tool_choice)
}
