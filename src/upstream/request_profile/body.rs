//! Only fill protocol-valid client defaults; user content, tools and generation choices win.
use super::*;
use serde_json::{json, Value};

pub(crate) fn apply_body(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    body: &mut Value,
) {
    if applies(payload, req) && selected(payload) == RequestClientProfile::Workbuddy {
        super::workbuddy_codex::sanitize_body_metadata(body);
    }
    if applies(payload, req)
        && selected(payload) == RequestClientProfile::Workbuddy
        && payload.canonical_adapter() == "openai_compatible"
    {
        super::workbuddy::preserve_message_context(req, body);
    }
    if !applies(payload, req)
        || uses_claude(payload, req, model)
        || selected(payload) == RequestClientProfile::Workbuddy
    {
        return;
    }
    // Identity selection must not turn Chat/Messages/Gemini into Responses.
    if payload.canonical_adapter() != "openai_compatible" || body.get("input").is_none() {
        return;
    }
    let Some(map) = body.as_object_mut() else {
        return;
    };
    for (key, default) in [
        ("store", json!(false)),
        ("include", json!(["reasoning.encrypted_content"])),
        ("parallel_tool_calls", json!(true)),
        ("instructions", json!("")),
    ] {
        let value = payload.extra_body.as_ref().and_then(|extra| extra.get(key));
        // Native Codex metadata accompanies a complete caller-owned Responses
        // shape (including responses-lite). Do not invent omitted defaults;
        // configured provider defaults still retain their existing precedence.
        if req.endpoint_kind == EndpointKind::Responses
            && selected(payload) == RequestClientProfile::CodexCli
            && req.raw_body.get("client_metadata").is_some()
            && value.is_none()
        {
            continue;
        }
        map.entry(key)
            .or_insert_with(|| value.cloned().unwrap_or(default));
    }
    // Stateful continuation, stream mode, sampling and reasoning effort remain caller-owned.
}
