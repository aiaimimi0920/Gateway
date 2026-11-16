//! 原生请求保留未知 wire 字段，但 canonical 修改始终优先于原始副本。
use super::{invalid, GatewayError};
use crate::protocol::{
    anthropic,
    canonical::{CanonicalRelayRequest, EndpointKind},
    responses,
};
use serde::Serialize;
use serde_json::{json, Value};

pub(super) fn equal<T: Serialize + ?Sized>(a: &T, b: &T) -> bool {
    serde_json::to_value(a).expect("canonical serialization")
        == serde_json::to_value(b).expect("canonical serialization")
}

pub(super) fn normalize(
    req: &CanonicalRelayRequest,
    body: Value,
) -> Result<CanonicalRelayRequest, GatewayError> {
    if req.endpoint_kind == EndpointKind::Responses {
        responses::normalize_responses(body)
    } else {
        anthropic::normalize_messages(body)
    }
}

pub(super) fn rebuilt(req: &CanonicalRelayRequest, model: &str, stream: bool) -> Value {
    let mut clean = req.clone();
    clean.raw_body = Value::Null;
    clean.extra.retain(|key, _| {
        !matches!(
            key.as_str(),
            "model"
                | "stream"
                | "messages"
                | "input"
                | "system"
                | "instructions"
                | "tools"
                | "tool_choice"
                | "thinking"
                | "reasoning"
                | "previous_response_id"
        )
    });
    clean
        .extra
        .insert("gateway_auto_cache".into(), json!(false));
    if req.endpoint_kind == EndpointKind::Responses {
        responses::pack_responses_preserving_system(&clean, model, stream)
    } else {
        anthropic::pack_anthropic(&clean, model, stream)
    }
}

fn replace(body: &mut Value, packed: &Value, key: &str) {
    let map = body.as_object_mut().expect("native object");
    if let Some(value) = packed.get(key) {
        map.insert(key.into(), value.clone());
    } else {
        map.remove(key);
    }
}

pub(super) fn pack(
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<Value, GatewayError> {
    let original = normalize(req, req.raw_body.clone())?;
    let mut body = req.raw_body.clone();
    if !body.is_object()
        || ["gateway_vertex_history", "gateway_bedrock_history"]
            .iter()
            .any(|k| body.get(k).is_some())
    {
        return Err(invalid());
    }
    let packed = rebuilt(req, model, stream);
    super::native_request_history::sync(req, &original, &mut body, &packed)?;
    if !equal(&req.tools, &original.tools) {
        body.as_object_mut().unwrap().remove("tools");
        if !req.tools.is_empty() {
            body["tools"] = Value::Array(
                req.tools
                    .iter()
                    .map(|tool| {
                        let mut value = Value::Object(
                            tool.raw
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect(),
                        );
                        let schema = if req.endpoint_kind == EndpointKind::Responses {
                            "parameters"
                        } else {
                            "input_schema"
                        };
                        for (key, field) in [
                            ("name", tool.name.as_ref().map(|v| json!(v))),
                            ("description", tool.description.as_ref().map(|v| json!(v))),
                            (schema, tool.input_schema.clone()),
                        ] {
                            if let Some(field) = field {
                                value[key] = field;
                            } else {
                                value.as_object_mut().unwrap().remove(key);
                            }
                        }
                        if req.endpoint_kind == EndpointKind::Responses {
                            value["type"] = json!(tool.tool_type);
                        }
                        value
                    })
                    .collect(),
            );
        }
    }
    if req.tool_choice != original.tool_choice {
        replace(&mut body, &packed, "tool_choice");
    }
    let responses = req.endpoint_kind == EndpointKind::Responses;
    if req.reasoning != original.reasoning {
        replace(
            &mut body,
            &packed,
            if responses { "reasoning" } else { "thinking" },
        );
    }
    if responses {
        // These fields have canonical owners, including removal by policy.
        for (key, value) in [
            (
                "previous_response_id",
                req.previous_response_id.as_ref().map(|v| json!(v)),
            ),
            ("metadata", req.metadata.as_ref().map(|v| json!(v))),
            ("user", req.explicit_session_key.as_ref().map(|v| json!(v))),
        ] {
            if let Some(value) = value {
                body[key] = value;
            } else {
                body.as_object_mut().unwrap().remove(key);
            }
        }
    }
    // Only fields owned by extra participate; unknown Responses extensions remain raw.
    for key in original.extra.keys().chain(req.extra.keys()) {
        if responses && matches!(key.as_str(), "metadata" | "previous_response_id" | "user") {
            continue;
        }
        if matches!(
            key.as_str(),
            "model"
                | "stream"
                | "messages"
                | "input"
                | "system"
                | "instructions"
                | "tools"
                | "tool_choice"
                | "thinking"
                | "reasoning"
        ) {
            continue;
        }
        if let Some(value) = req.extra.get(key) {
            body[key] = value.clone();
        } else {
            body.as_object_mut().unwrap().remove(key);
        }
    }
    // Product-local reserved top-level controls never become provider parameters.
    // Nested user data, tool arguments and metadata are deliberately untouched.
    body.as_object_mut()
        .unwrap()
        .retain(|key, _| !key.starts_with("gateway_") && !key.starts_with("neuro_"));
    body["model"] = json!(model);
    body["stream"] = json!(stream);
    if !responses && body.get("max_tokens").is_none() {
        body["max_tokens"] = packed["max_tokens"].clone();
    }
    // Stateful/background lifecycle is still an open development item, not silently ignored.
    if ["store", "background"]
        .iter()
        .any(|k| body.get(k) == Some(&json!(true)))
        || ["previous_response_id", "conversation"]
            .iter()
            .any(|k| body.get(k).is_some_and(|v| !v.is_null()))
        || body["tools"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|t| t["type"] == "mcp"))
    {
        return Err(invalid());
    }
    Ok(body)
}
