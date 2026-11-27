//! Function/custom-text Responses admission and exact full-history continuation.
//! No tool output is ever converted into a new user prompt.
use super::request;
use super::tool_schema::{definitions, invalid, Definition, ToolKind};
use crate::{error::GatewayError, protocol::canonical::CanonicalRelayRequest};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub(crate) struct ToolRequest {
    pub prompt: String,
    pub images: super::inline_images::Images,
    pub tools: Vec<Definition>,
    pub history: Vec<Value>,
    pub signature: String,
    pub output: Option<ToolOutput>,
    pub chat: bool,
    pub family: crate::protocol::canonical::ProtocolFamily,
}
#[derive(Clone, Debug)]
pub(crate) struct ToolOutput {
    pub call_id: String,
    pub kind: ToolKind,
    pub text: String,
}
impl ToolOutput {
    pub(super) fn from_item(item: &Value) -> Result<Self, GatewayError> {
        let object = item.as_object().ok_or_else(invalid)?;
        if object
            .keys()
            .any(|k| !["type", "id", "call_id", "output"].contains(&k.as_str()))
            || item
                .get("id")
                .is_some_and(|v| v.as_str().is_none_or(|s| s.is_empty() || s.len() > 256))
        {
            return Err(invalid());
        }
        let kind = match item["type"].as_str() {
            Some("function_call_output") => ToolKind::Function,
            Some("custom_tool_call_output") => ToolKind::CustomText,
            _ => return Err(invalid()),
        };
        Ok(Self {
            call_id: call_id(&item["call_id"])?.into(),
            kind,
            text: super::text_output::parse(&item["output"], "input_text")?,
        })
    }
    pub(super) fn history_item(&self) -> Value {
        json!({"type":self.kind.output_type(),"call_id":self.call_id,"output":self.text})
    }
}
pub(crate) fn parse(req: &CanonicalRelayRequest) -> Result<ToolRequest, GatewayError> {
    parse_projected(req, false)
}
pub(super) fn parse_projected(
    req: &CanonicalRelayRequest,
    chat: bool,
) -> Result<ToolRequest, GatewayError> {
    let (req, images) = super::inline_images::extract(req, chat)?;
    // Share header/storage/media admission with the proven text lane.
    let mut header = req.clone();
    header.tools.clear();
    header.raw_body["tools"] = json!([]);
    header.raw_body["input"] = json!("tool header admission");
    request::prompt(&header)?;
    if req
        .raw_body
        .get("tool_choice")
        .is_some_and(|v| v != "auto" && v != "none")
    {
        return Err(invalid());
    }
    let disabled = req.raw_body.get("tool_choice").is_some_and(|v| v == "none");
    let tools = definitions(&req.raw_body["tools"])?;
    let items = match &req.raw_body["input"] {
        Value::String(text) => vec![json!({"role":"user","content":text})],
        Value::Array(items) if items.len() <= 256 => items.clone(),
        _ => return Err(invalid()),
    };
    let mut history = Vec::new();
    for item in &items {
        let object = item.as_object().ok_or_else(invalid)?;
        if item
            .get("id")
            .is_some_and(|v| v.as_str().is_none_or(|s| s.is_empty() || s.len() > 256))
        {
            return Err(invalid());
        }
        // Only absence implies a message; malformed explicit discriminators
        // must not be erased by the generic Responses normalizer.
        let item_type = match item.get("type") {
            None => "message",
            Some(value) => value.as_str().ok_or_else(invalid)?,
        };
        let normalized = match item_type {
            "message" => {
                if object.keys().any(|k| {
                    !["type", "role", "content", "id", "status", "phase"].contains(&k.as_str())
                }) {
                    return Err(invalid());
                }
                let role = item["role"].as_str().ok_or_else(invalid)?;
                if !["system", "developer", "user", "assistant"].contains(&role) {
                    return Err(invalid());
                }
                json!({"role":role,"content":request::content(&item["content"], role == "assistant")?})
            }
            call_type @ ("function_call" | "custom_tool_call") => {
                let kind = if call_type == "custom_tool_call" {
                    ToolKind::CustomText
                } else {
                    ToolKind::Function
                };
                if object.keys().any(|k| {
                    ![
                        "type",
                        "call_id",
                        "name",
                        kind.input_field(),
                        "id",
                        "status",
                    ]
                    .contains(&k.as_str())
                }) {
                    return Err(invalid());
                }
                let name = item["name"].as_str().ok_or_else(invalid)?;
                let input = text(&item[kind.input_field()], 65536)?;
                if !tools
                    .iter()
                    .any(|tool| tool.name == name && tool.kind == kind)
                    || (kind == ToolKind::Function
                        && !serde_json::from_str::<Value>(input).is_ok_and(|v| v.is_object()))
                {
                    return Err(invalid());
                }
                let mut call = json!({"type":kind.call_type(),"call_id":call_id(&item["call_id"])?,"name":name});
                call[kind.input_field()] = json!(input);
                call
            }
            "function_call_output" | "custom_tool_call_output" => {
                ToolOutput::from_item(item)?.history_item()
            }
            "reasoning" => super::thinking::history(item, chat)?,
            _ => return Err(invalid()),
        };
        history.push(normalized);
    }
    let last = history.last().ok_or_else(invalid)?;
    // Existing MCP capabilities cannot be revoked by a continuation IPC result.
    // Never rebuild a Run or replay historical results to fake tool_choice:none.
    if disabled && history.iter().any(|item| item.get("type").is_some()) {
        return Err(invalid());
    }
    let kind = match last["type"].as_str() {
        Some("function_call_output") => Some(ToolKind::Function),
        Some("custom_tool_call_output") => Some(ToolKind::CustomText),
        _ => None,
    };
    let output = kind.map(|kind| ToolOutput {
        call_id: last["call_id"].as_str().unwrap().into(),
        kind,
        text: last["output"].as_str().unwrap().into(),
    });
    if let Some(output) = &output {
        super::text_output::check_ipc(&output.call_id, &output.text)?;
    }
    let prompt = if output.is_some() {
        String::new()
    } else {
        // A new Run may not silently replay prior calls or unclaimed tool outputs.
        if last["role"] != "user" || history.iter().any(|i| i.get("type").is_some()) {
            return Err(invalid());
        }
        header.raw_body["input"] = json!(history);
        request::prompt_with_capabilities(&header, !images.is_empty(), !disabled)?
    };
    let signature = format!("{:x}", Sha256::digest(serde_json::to_vec(&json!({
        "tools":req.raw_body["tools"],"instructions":req.raw_body.get("instructions"),"tool_choice":req.raw_body.get("tool_choice"),
        "parallel_tool_calls":req.raw_body.get("parallel_tool_calls"), "images":images.binding()
    })).unwrap()));
    Ok(ToolRequest {
        prompt,
        images,
        tools,
        history,
        signature,
        output,
        chat,
        family: req.protocol_family,
    })
}
fn call_id(value: &Value) -> Result<&str, GatewayError> {
    let id = text(value, 256)?;
    if id.trim().is_empty() {
        return Err(invalid());
    }
    Ok(id)
}
fn text(value: &Value, max: usize) -> Result<&str, GatewayError> {
    value
        .as_str()
        .filter(|s| s.len() <= max)
        .ok_or_else(invalid)
}
