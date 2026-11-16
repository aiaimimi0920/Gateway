//! Move native text/tools into the public envelope; retain non-equivalent data as explicit metadata.
//! The extension is observation only, not an Anthropic thinking signature or a replay capability.
use super::*;
mod tools;
pub(in super::super) fn response(
    body: Value,
    req: &CanonicalRelayRequest,
    fallback_id: &str,
) -> Result<Value, BusinessError> {
    project(
        body,
        req.endpoint_kind,
        req.requested_model.as_deref().unwrap_or("unknown"),
        fallback_id,
    )
}
pub(in super::super) fn project(
    body: Value,
    kind: EndpointKind,
    model: &str,
    fallback_id: &str,
) -> Result<Value, BusinessError> {
    if kind == EndpointKind::ChatCompletions {
        return Ok(body);
    }
    let invalid = || BusinessError::InvalidResponse;
    let Value::Object(mut native) = body else {
        return Err(invalid());
    };
    let Value::Array(mut choices) = native.remove("choices").ok_or_else(invalid)? else {
        return Err(invalid());
    };
    if choices.len() != 1 {
        return Err(invalid());
    }
    let Value::Object(mut choice) = choices.pop().expect("single choice") else {
        return Err(invalid());
    };
    if choice.get("index").is_some_and(|v| v.as_u64() != Some(0)) {
        return Err(invalid());
    }
    let finish = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?
        .to_owned();
    let Value::Object(mut message) = choice.remove("message").ok_or_else(invalid)? else {
        return Err(invalid());
    };
    let text = match message.remove("content") {
        Some(Value::String(text)) => text,
        None | Some(Value::Null) => String::new(),
        _ => return Err(invalid()),
    };
    let calls = match message.remove("tool_calls") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(calls)) => calls,
        _ => return Err(invalid()),
    };
    if matches!(finish.as_str(), "tool_calls" | "function_call") && calls.is_empty() {
        return Err(invalid());
    }
    let is_messages = kind == EndpointKind::Messages;
    if !is_messages && kind != EndpointKind::Responses {
        return Err(invalid());
    }
    let id = native
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_id);
    // Reuse the framework's envelope/finish semantics, without permissive text/tool extraction.
    let mut output = if is_messages {
        crate::protocol::anthropic::build_messages_success(id, model, "", None, &[], Some(&finish))
    } else {
        crate::protocol::responses::build_responses_success(id, model, "", None, &[], Some(&finish))
    };
    let mut parts = Vec::new();
    if !text.is_empty() || calls.is_empty() {
        let mut part = if is_messages {
            json!({"type":"text"})
        } else {
            json!({"type":"message","role":"assistant","content":[{"type":"output_text"}]})
        };
        if is_messages {
            part["text"] = Value::String(text);
        } else {
            part["content"][0]["text"] = Value::String(text);
        }
        parts.push(part);
    }
    let mut extras = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for call in calls {
        let (part, extra) = tools::project(call, is_messages, &mut ids)?;
        parts.push(part);
        extras.push(extra);
    }
    output[if is_messages { "content" } else { "output" }] = Value::Array(parts);
    if let Some(usage) = native
        .get("usage")
        .and_then(|u| project_usage(u, is_messages))
    {
        output["usage"] = usage;
    }
    // No raw full-completion clone: native content and tool wire fields were moved into parts.
    output["gateway_freebuff"] = Value::Object(serde_json::Map::from_iter([
        ("version".into(), json!(1)),
        ("completion".into(), Value::Object(native)),
        ("choice".into(), Value::Object(choice)),
        ("message".into(), Value::Object(message)),
        ("tool_calls".into(), Value::Array(extras)),
    ]));
    if !within_limit(
        &output,
        crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
    ) {
        return Err(BusinessError::ResponseTooLarge);
    }
    Ok(output)
}
fn project_usage(raw: &Value, messages: bool) -> Option<Value> {
    let mut usage = serde_json::Map::new();
    for (source, target) in [
        ("prompt_tokens", "input_tokens"),
        ("completion_tokens", "output_tokens"),
        ("total_tokens", "total_tokens"),
    ] {
        if messages && source == "total_tokens" {
            continue;
        }
        if let Some(n) = raw.get(source).and_then(Value::as_u64) {
            usage.insert(target.into(), json!(n));
        }
    }
    // Counts remain partial if the provider report is partial; do not manufacture zero/cache data.
    if let Some(n) = raw
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .filter(|_| !messages)
    {
        usage.insert("input_tokens_details".into(), json!({"cached_tokens":n}));
    }
    if let Some(n) = raw
        .pointer("/completion_tokens_details/reasoning_tokens")
        .and_then(Value::as_u64)
        .filter(|_| !messages)
    {
        usage.insert(
            "output_tokens_details".into(),
            json!({"reasoning_tokens":n}),
        );
    }
    if usage.is_empty() {
        None
    } else {
        Some(Value::Object(usage))
    }
}
