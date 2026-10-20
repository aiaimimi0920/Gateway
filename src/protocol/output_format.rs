//! Lossless layout conversion for the shared Chat/Responses structured-output contract.
//! Never rewrite JSON Schema or infer `strict`: false, null and absence are distinct.
use super::canonical::{CanonicalRelayRequest, EndpointKind};
use serde_json::{json, Value};

fn chat_format(format: &Value) -> Option<Value> {
    match format.get("type")?.as_str()? {
        "text" | "json_object" => Some(format.clone()),
        "json_schema" => {
            let mut config = format.as_object()?.clone();
            config.remove("type");
            Some(json!({"type":"json_schema", "json_schema": config}))
        }
        _ => None,
    }
}

fn responses_format(format: &Value) -> Option<Value> {
    match format.get("type")?.as_str()? {
        "text" | "json_object" => Some(format.clone()),
        "json_schema" => {
            let mut config = format.get("json_schema")?.as_object()?.clone();
            config.insert("type".into(), json!("json_schema"));
            Some(Value::Object(config))
        }
        _ => None,
    }
}

pub(super) fn apply_chat(req: &CanonicalRelayRequest, body: &mut Value) {
    if req.endpoint_kind != EndpointKind::Responses {
        return;
    }
    if let Some(format) = req
        .extra
        .get("text")
        .and_then(|text| text.get("format"))
        .and_then(chat_format)
    {
        body["response_format"] = format;
    }
    if let Some(verbosity) = req.extra.get("text").and_then(|text| text.get("verbosity")) {
        body["verbosity"] = verbosity.clone();
    }
    // These belong to Responses, not Chat; never send them as vendor extensions.
    let map = body.as_object_mut().unwrap();
    map.remove("text");
    map.remove("include");
}

pub(super) fn apply_responses(
    req: &CanonicalRelayRequest,
    body: &mut serde_json::Map<String, Value>,
) {
    if req.endpoint_kind == EndpointKind::Responses {
        return;
    }
    let mut text = serde_json::Map::new();
    if let Some(format) = req.extra.get("response_format").and_then(responses_format) {
        text.insert("format".into(), format);
        body.remove("response_format");
    }
    if let Some(verbosity) = req.extra.get("verbosity") {
        text.insert("verbosity".into(), verbosity.clone());
        body.remove("verbosity");
    }
    if !text.is_empty() {
        body.insert("text".into(), Value::Object(text));
    }
}
