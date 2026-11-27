//! Strict text/image subset for contracts requiring request-level token usage.
//! Tool phases and generation controls remain rejected until their semantics are proved.
use super::{gemini_tools::keys, request::invalid};
use crate::{
    error::GatewayError,
    protocol::canonical::{CanonicalRelayRequest, ProtocolFamily},
};
use serde_json::{json, Value};

pub(super) fn project(req: &CanonicalRelayRequest) -> Result<CanonicalRelayRequest, GatewayError> {
    let messages = if req.protocol_family == ProtocolFamily::BedrockConverse {
        bedrock(&req.raw_body)?
    } else {
        dashscope(req)?
    };
    let mut internal = req.clone();
    internal.protocol_family = ProtocolFamily::OpenAi;
    internal.raw_body =
        json!({"model":req.requested_model,"stream":req.stream,"messages":messages});
    super::chat_request::project(&internal)
}

fn bedrock(body: &Value) -> Result<Vec<Value>, GatewayError> {
    keys(body, &["messages", "system", "inferenceConfig"])?;
    if body.get("inferenceConfig").is_some_and(|v| v != &json!({})) {
        return Err(invalid());
    }
    let mut messages = Vec::new();
    if let Some(system) = body.get("system") {
        let mut text = String::new();
        for part in array(system)? {
            keys(part, &["text"])?;
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(part["text"].as_str().ok_or_else(invalid)?);
        }
        messages.push(json!({"role":"system","content":text}));
    }
    for message in array(&body["messages"])? {
        keys(message, &["role", "content"])?;
        let role = message["role"]
            .as_str()
            .filter(|r| matches!(*r, "user" | "assistant"))
            .ok_or_else(invalid)?;
        let mut parts = Vec::new();
        for part in array(&message["content"])? {
            if let Some(text) = part.get("text") {
                keys(part, &["text"])?;
                parts.push(json!({"type":"text","text":text.as_str().ok_or_else(invalid)?}));
            } else {
                keys(part, &["image"])?;
                let image = &part["image"];
                keys(image, &["format", "source"])?;
                keys(&image["source"], &["bytes"])?;
                let format = image["format"]
                    .as_str()
                    .filter(|f| matches!(*f, "png" | "jpeg"))
                    .ok_or_else(invalid)?;
                let bytes = image["source"]["bytes"].as_str().ok_or_else(invalid)?;
                if role != "user" {
                    return Err(invalid());
                }
                parts.push(json!({"type":"image_url","image_url":{"url":format!("data:image/{format};base64,{bytes}")}}));
            }
        }
        messages.push(json!({"role":role,"content":parts}));
    }
    Ok(messages)
}

fn dashscope(req: &CanonicalRelayRequest) -> Result<Vec<Value>, GatewayError> {
    let body = &req.raw_body;
    keys(body, &["model", "input", "parameters"])?;
    let multimodal = req.protocol_family == ProtocolFamily::DashScopeMultimodal;
    if let Some(parameters) = body.get("parameters") {
        keys(parameters, &["result_format", "incremental_output"])?;
        if parameters
            .get("result_format")
            .is_some_and(|v| v != "message" && (multimodal || v != "text"))
            || parameters
                .get("incremental_output")
                .is_some_and(|v| !req.stream || !v.is_boolean())
        {
            return Err(invalid());
        }
    }
    let input = &body["input"];
    keys(input, &["prompt", "messages"])?;
    if let Some(prompt) = input.get("prompt") {
        if multimodal || input.get("messages").is_some() {
            return Err(invalid());
        }
        return Ok(vec![
            json!({"role":"user","content":prompt.as_str().ok_or_else(invalid)?}),
        ]);
    }
    let mut messages = Vec::new();
    for message in array(&input["messages"])? {
        keys(message, &["role", "content"])?;
        let role = message["role"]
            .as_str()
            .filter(|r| matches!(*r, "system" | "user" | "assistant"))
            .ok_or_else(invalid)?;
        let content = if multimodal {
            let mut parts = Vec::new();
            for part in array(&message["content"])? {
                if let Some(text) = part.get("text") {
                    keys(part, &["text"])?;
                    parts.push(json!({"type":"text","text":text.as_str().ok_or_else(invalid)?}));
                } else {
                    keys(part, &["image"])?;
                    let url = part["image"].as_str().ok_or_else(invalid)?;
                    if role != "user" || !url.starts_with("data:image/") {
                        return Err(invalid());
                    }
                    parts.push(json!({"type":"image_url","image_url":{"url":url}}));
                }
            }
            json!(parts)
        } else {
            json!(message["content"].as_str().ok_or_else(invalid)?)
        };
        messages.push(json!({"role":role,"content":content}));
    }
    Ok(messages)
}

fn array(value: &Value) -> Result<&Vec<Value>, GatewayError> {
    value
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 128)
        .ok_or_else(invalid)
}
