//! Admit the lossless public Chat subset before the shared Converse packer runs.
use std::{borrow::Cow, collections::HashSet};

use serde_json::{json, Value};

use super::{bedrock_images, image_input::InlineImages, invalid, GatewayError};
use crate::protocol::{
    bedrock_converse::{
        self,
        public_tools::{arguments, tool_id, tool_name, PublicTools},
    },
    canonical::{CanonicalRelayRequest, ContentPart, MessageRole},
    openai::decode_provider_tool_argument_entities,
    tool_choice::{parse_tool_choice, CanonicalToolChoice},
};

pub(super) fn pack(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
    images_enabled: bool,
) -> Result<Value, GatewayError> {
    let strict = definitions(req)?;
    let choice = req.raw_body.get("tool_choice");
    if choice != req.tool_choice.as_ref() || !valid_choice(req, choice) {
        return Err(invalid());
    }
    let request = history(req, images_enabled)?;
    let mut body = bedrock_converse::try_pack_bedrock_converse(&request, model, stream)
        .map_err(|_| invalid())?;
    if !strict.is_empty() {
        let specs = body["toolConfig"]["tools"]
            .as_array_mut()
            .ok_or_else(invalid)?;
        for (spec, strict) in specs.iter_mut().zip(strict) {
            let spec = spec["toolSpec"].as_object_mut().ok_or_else(invalid)?;
            if spec.get("description").is_some_and(Value::is_null) {
                spec.remove("description");
            }
            if let Some(strict) = strict {
                spec.insert("strict".into(), json!(strict));
            }
        }
    }
    Ok(body)
}

pub(super) fn response_tools(req: &CanonicalRelayRequest) -> PublicTools {
    let choice = parse_tool_choice(req.tool_choice.as_ref());
    let specific = match &choice {
        Some(CanonicalToolChoice::Specific(name)) => Some(name.as_str()),
        _ => None,
    };
    let names = req
        .tools
        .iter()
        .filter_map(|tool| tool.name.as_deref())
        .filter(|name| specific.is_none_or(|selected| selected == *name))
        .map(str::to_owned);
    let ids = req
        .messages
        .iter()
        .flat_map(|message| &message.tool_calls)
        .filter_map(|call| call.id.clone());
    PublicTools::new(
        names,
        ids,
        matches!(
            choice,
            Some(CanonicalToolChoice::Required | CanonicalToolChoice::Specific(_))
        ),
    )
}

fn keys(value: &Value, allowed: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|m| m.keys().all(|key| allowed.contains(&key.as_str())))
}

fn definitions(req: &CanonicalRelayRequest) -> Result<Vec<Option<bool>>, GatewayError> {
    let Some(raw) = req.raw_body.get("tools") else {
        return if req.tools.is_empty() {
            Ok(vec![])
        } else {
            Err(invalid())
        };
    };
    let raw = raw
        .as_array()
        .filter(|tools| !tools.is_empty() && tools.len() == req.tools.len())
        .ok_or_else(invalid)?;
    let mut names = HashSet::new();
    raw.iter()
        .zip(&req.tools)
        .map(|(raw, tool)| {
            let f = &raw["function"];
            let name = tool
                .name
                .as_deref()
                .filter(|name| tool_name(name))
                .ok_or_else(invalid)?;
            let has_schema = |value: &Value| value.is_object() && value["type"] == "object";
            if !keys(raw, &["type", "function"])
                || raw["type"] != "function"
                || tool.tool_type != "function"
                || !keys(f, &["name", "description", "parameters", "strict"])
                || f["name"] != name
                || !names.insert(name)
                || f.get("description")
                    .is_some_and(|v| v.as_str().is_none_or(str::is_empty))
                || tool.description.as_deref().is_some_and(str::is_empty)
                || f.get("parameters").is_some_and(|v| !has_schema(v))
                || tool.input_schema.as_ref().is_some_and(|v| !has_schema(v))
                || tool.raw.get("function").and_then(|v| v.get("strict")) != f.get("strict")
            {
                return Err(invalid());
            }
            f.get("strict")
                .map(|v| v.as_bool().ok_or_else(invalid))
                .transpose()
        })
        .collect()
}

fn valid_choice(req: &CanonicalRelayRequest, choice: Option<&Value>) -> bool {
    let Some(choice) = choice else { return true };
    if req.tools.is_empty() {
        return false;
    }
    // Converse has no equivalent to `none`; the shared bridge otherwise maps it to auto.
    matches!(choice.as_str(), Some("auto" | "required"))
        || keys(choice, &["type", "function"])
            && choice["type"] == "function"
            && keys(&choice["function"], &["name"])
            && choice["function"]["name"].as_str().is_some_and(|name| {
                req.tools
                    .iter()
                    .any(|tool| tool.name.as_deref() == Some(name))
            })
}

fn history(
    req: &CanonicalRelayRequest,
    images_enabled: bool,
) -> Result<Cow<'_, CanonicalRelayRequest>, GatewayError> {
    let raw = req.raw_body["messages"]
        .as_array()
        .filter(|raw| raw.len() == req.messages.len())
        .ok_or_else(invalid)?;
    let names: HashSet<_> = req
        .tools
        .iter()
        .filter_map(|tool| tool.name.as_deref())
        .collect();
    let mut pending = HashSet::new();
    let mut seen = HashSet::new();
    let mut out = Cow::Borrowed(req);
    let mut images = InlineImages::new();
    for (message_index, (raw, msg)) in raw.iter().zip(&req.messages).enumerate() {
        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        if raw["role"] != role || msg.name.is_some() {
            return Err(invalid());
        }
        let text =
            matches!(msg.content.as_slice(), [ContentPart::Text { text }] if !text.is_empty());
        if msg.role == MessageRole::Tool {
            let id = raw["tool_call_id"].as_str().ok_or_else(invalid)?;
            if !keys(raw, &["role", "content", "tool_call_id"])
                || !raw["content"].is_string()
                || !text
                || msg.tool_call_id.as_deref() != Some(id)
                || !pending.remove(id)
                || !msg.tool_calls.is_empty()
            {
                return Err(invalid());
            }
            continue;
        }
        if !pending.is_empty() || msg.tool_call_id.is_some() {
            return Err(invalid());
        }
        if images_enabled && msg.role == MessageRole::User {
            if let Some(parts) = raw["content"].as_array() {
                if !keys(raw, &["role", "content"]) || !msg.tool_calls.is_empty() {
                    return Err(invalid());
                }
                let parts = bedrock_images::project_user_content(parts, &msg.content, &mut images)?;
                out.to_mut().messages[message_index].content = parts;
                continue;
            }
        }
        if msg.role != MessageRole::Assistant || raw.get("tool_calls").is_none() {
            if !keys(raw, &["role", "content"])
                || !raw["content"].is_string()
                || !text
                || !msg.tool_calls.is_empty()
            {
                return Err(invalid());
            }
            continue;
        }
        if !keys(raw, &["role", "content", "tool_calls"])
            || !(raw["content"].is_null() || raw["content"].is_string())
        {
            return Err(invalid());
        }
        match msg.content.as_slice() {
            [] => {}
            [ContentPart::Text { text }] if text.is_empty() => {
                out.to_mut().messages[message_index].content.clear()
            }
            [ContentPart::Text { .. }] => {}
            _ => return Err(invalid()),
        }
        let raw_calls = raw["tool_calls"]
            .as_array()
            .filter(|calls| !calls.is_empty() && calls.len() == msg.tool_calls.len())
            .ok_or_else(invalid)?;
        for (call_index, (raw, call)) in raw_calls.iter().zip(&msg.tool_calls).enumerate() {
            let f = &raw["function"];
            let id = raw["id"]
                .as_str()
                .filter(|id| tool_id(id))
                .ok_or_else(invalid)?;
            let name = f["name"]
                .as_str()
                .filter(|name| names.contains(name))
                .ok_or_else(invalid)?;
            let encoded = f["arguments"].as_str().ok_or_else(invalid)?;
            if !keys(raw, &["id", "type", "function"])
                || raw["type"] != "function"
                || call.call_type != "function"
                || !keys(f, &["name", "arguments"])
                || call.id.as_deref() != Some(id)
                || call.name.as_deref() != Some(name)
                || !seen.insert(id)
                || arguments(encoded).is_none()
            {
                return Err(invalid());
            }
            pending.insert(id);
            if call.arguments.as_deref() != Some(encoded) {
                // Undo only the legacy HTML repair, not a policy edit. Validate raw JSON first.
                if call.arguments.as_deref()
                    != Some(decode_provider_tool_argument_entities(encoded).as_str())
                {
                    return Err(invalid());
                }
                out.to_mut().messages[message_index].tool_calls[call_index].arguments =
                    Some(encoded.into());
            }
        }
    }
    if !pending.is_empty() || !req.messages.iter().any(|msg| msg.role == MessageRole::User) {
        return Err(invalid());
    }
    Ok(out)
}
