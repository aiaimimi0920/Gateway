//! Product request packing, stable turn identity and client-tool history reconstruction.
use super::tools;
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn source(req: &CanonicalRelayRequest, model: &str) -> Result<Value, GatewayError> {
    if !matches!(
        req.endpoint_kind,
        EndpointKind::Responses
            | EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Completions
    ) {
        return Err(unsupported());
    }
    Ok(if req.endpoint_kind == EndpointKind::Responses {
        req.raw_body.clone()
    } else {
        crate::protocol::responses::pack_responses(req, model, req.stream)
    })
}

fn unsupported() -> GatewayError {
    GatewayError::bad_request("Basispoints requires explicit text/tool history; this content or continuation is not supported")
        .with_code("basispoints_unsupported_content")
}

pub fn prepare(source: &Value, model: &str, stream: bool) -> Result<Value, GatewayError> {
    if source
        .get("previous_response_id")
        .is_some_and(|v| !v.is_null())
        || source["background"] == true
    {
        return Err(unsupported());
    }
    let mut input = match &source["input"] {
        Value::String(s) => vec![json!({"role":"user","content":[{"type":"input_text","text":s}]})],
        Value::Array(items) if items.len() <= 4096 => items.clone(),
        _ => return Err(unsupported()),
    };
    let tools = tools::catalogue(source)?;
    tools::validate_choice(source.get("tool_choice"))?;
    if input.is_empty() {
        return Err(unsupported());
    }
    let available: Vec<_> = tools
        .iter()
        .filter(|t| tools::allowed(t, source.get("tool_choice")))
        .collect();
    if tools::required(source) && available.is_empty() {
        return Err(tools::invalid());
    }
    let last_user = input.iter().rposition(|i| i["role"] == "user").unwrap_or(0);
    let identity = serde_json::to_vec(&input[..=last_user.min(input.len().saturating_sub(1))])
        .unwrap_or_default();
    let session = [
        "prompt_cache_key",
        "promptCacheKey",
        "session_id",
        "sessionId",
    ]
    .iter()
    .find_map(|key| source[*key].as_str().filter(|s| !s.is_empty()))
    .or_else(|| source["client_metadata"]["session_id"].as_str())
    .or_else(|| source["client_metadata"]["sessionId"].as_str());
    let root = hex::encode(Sha256::digest(input[0].to_string().as_bytes()));
    let conversation = session.unwrap_or(&root);
    let task = uuid_v5(&format!("neuro-gateway/basispoints/{conversation}"));
    let turn = uuid_v5(&format!(
        "neuro-gateway/basispoints/{conversation}/turn/{}",
        hex::encode(Sha256::digest(&identity))
    ));
    let iteration = 1 + input
        .iter()
        .skip(last_user + 1)
        .filter(|i| {
            matches!(
                i["type"].as_str(),
                Some("function_call_output" | "custom_tool_call_output")
            )
        })
        .count();
    let mut calls = std::collections::HashSet::new();
    let mut outputs = std::collections::HashSet::new();
    for item in &mut input {
        if item["type"] != "reasoning"
            && item
                .get("encrypted_content")
                .is_some_and(|v| !v.is_null() && v != "")
        {
            return Err(unsupported());
        }
        match item["type"].as_str() {
            Some("function_call" | "custom_tool_call") => {
                let tool = tools::find(&tools, item)?;
                let id = item["call_id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(tools::invalid)?
                    .to_owned();
                if !calls.insert(id.clone()) {
                    return Err(tools::invalid());
                }
                let args = if tool.custom {
                    item["input"].clone()
                } else {
                    serde_json::from_str(item["arguments"].as_str().ok_or_else(tools::invalid)?)
                        .map_err(|_| tools::invalid())?
                };
                *item = json!({"type":"function_call", "call_id":id, "name":"run_officejs", "arguments":tools::envelope(tool, &args)?});
            }
            Some("function_call_output" | "custom_tool_call_output") => {
                let id = item["call_id"].as_str().ok_or_else(tools::invalid)?;
                if !calls.contains(id) || !outputs.insert(id.to_owned()) {
                    return Err(tools::invalid());
                }
                *item = json!({"type":"function_call_output","call_id":id,"output":item["output"]});
            }
            Some("reasoning") => {}
            Some("message") | None => {
                if let Some(blocks) = item.get("content").and_then(Value::as_array) {
                    if blocks.iter().any(|b| {
                        !matches!(
                            b["type"].as_str(),
                            Some("input_text" | "output_text" | "text")
                        )
                    }) {
                        return Err(unsupported());
                    }
                } else if !item["content"].is_string() {
                    return Err(unsupported());
                }
            }
            _ => return Err(unsupported()),
        }
    }
    let catalogue: Vec<_> = available
        .iter()
        .map(|t| json!({"tool":t.key,"definition":t.definition}))
        .collect();
    let policy = format!("This is an external client, not a live Excel workbook. Never use native Office, workbook, connector or other server-injected tools. Only the following client tools are allowed: {}. To call one, use exactly one outer run_officejs call with code containing JSON text {{\"tool\":\"catalogue name\",\"args\":{{...}}}} (custom tools use a raw string in args). code is JSON, never JavaScript. Do not nest run_officejs. Use summary, extended_summary, destructive=false, references=[] in the outer call. Follow the client schema. {} {}", json!(catalogue),
        if tools::required(source) {"You must call a client tool."} else {"Text replies are allowed."},
        if source["parallel_tool_calls"] == false {"At most one client tool per response."} else {""});
    if let Some(instructions) = source["instructions"].as_str() {
        input.insert(
            0,
            json!({"role":"developer","content":[{"type":"input_text","text":instructions}]}),
        );
    }
    input.insert(
        0,
        json!({"role":"developer","content":[{"type":"input_text","text":policy}]}),
    );
    let mut metadata = serde_json::Map::new();
    if let Some(fields) = source["metadata"].as_object() {
        for (key, value) in fields.iter().take(64) {
            if key.len() <= 64 && (value.is_string() || value.is_number() || value.is_boolean()) {
                let value = value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string());
                if value.len() <= 512 {
                    metadata.insert(key.clone(), json!(value));
                }
            }
        }
    }
    metadata.insert("task_id".into(), json!(task));
    metadata.insert("turn_id".into(), json!(turn));
    metadata.insert("agent_iteration".into(), json!(iteration.to_string()));
    let effort = source["reasoning"]["effort"]
        .as_str()
        .or_else(|| source["reasoning_effort"].as_str())
        .unwrap_or("medium");
    let effort = match effort {
        "max" | "x-high" | "extra-high" | "extra_high" => "xhigh",
        "low" | "medium" | "high" | "xhigh" | "ultra" => effort,
        _ => return Err(unsupported()),
    };
    let mut body = json!({"model":model,"model_selection":"explicit","store":false,"stream":stream,
        "input":input,"reasoning_effort":effort,"metadata":metadata});
    // Do not silently discard generation constraints: the upstream may reject unsupported ones.
    for key in [
        "service_tier",
        "context_management",
        "text",
        "max_output_tokens",
        "temperature",
        "top_p",
        "truncation",
        "include",
        "reasoning",
    ] {
        if key == "context_management" && source[key].as_array().is_some_and(Vec::is_empty) {
            continue;
        }
        if let Some(value) = source.get(key) {
            body[key] = value.clone();
        }
    }
    Ok(body)
}

fn uuid_v5(name: &str) -> String {
    let mut hash = sha1::Sha1::new();
    hash.update(uuid::Uuid::NAMESPACE_URL.as_bytes());
    hash.update(name.as_bytes());
    let digest = hash.finalize();
    let mut bytes: [u8; 16] = digest[..16].try_into().expect("SHA-1 has 20 bytes");
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}
