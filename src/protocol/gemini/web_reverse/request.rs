use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use super::{GeminiWebBootstrap, GeminiWebRequest};
use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
};

pub fn pack_request(
    req: &CanonicalRelayRequest,
    _model: &str,
    bootstrap: &GeminiWebBootstrap,
) -> Result<GeminiWebRequest, GatewayError> {
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Messages
            | EndpointKind::Responses
            | EndpointKind::Completions
    ) {
        return Err(GatewayError::bad_request(
            "Gemini Web adapters currently support only text chat endpoints.",
        )
        .with_code("unsupported_gemini_web_endpoint"));
    }
    let combined_text = combine_messages_for_gemini_web(&req.messages)?;
    if combined_text.trim().is_empty() {
        return Err(GatewayError::bad_request(
            "Gemini Web requests require at least one text message part.",
        )
        .with_code("gemini_web_missing_text"));
    }

    let request_id = current_reqid();
    let request_uuid = uuid::Uuid::new_v4().to_string();
    let mut inner_req_list = vec![Value::Null; 69];
    inner_req_list[0] = json!([combined_text, 0, null, null, null, null, 0]);
    inner_req_list[1] = json!([bootstrap.language]);
    inner_req_list[2] = json!(["", "", "", null, null, null, null, null, null, ""]);
    inner_req_list[6] = json!([1]);
    inner_req_list[7] = Value::from(1);
    inner_req_list[10] = Value::from(1);
    inner_req_list[11] = Value::from(0);
    inner_req_list[17] = json!([[0]]);
    inner_req_list[18] = Value::from(0);
    inner_req_list[27] = Value::from(1);
    inner_req_list[30] = json!([4]);
    inner_req_list[41] = json!([1]);
    inner_req_list[53] = Value::from(0);
    inner_req_list[59] = Value::String(request_uuid);
    inner_req_list[61] = Value::Array(Vec::new());
    inner_req_list[68] = Value::from(2);

    let inner_payload = serde_json::to_string(&inner_req_list).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Web inner request payload: {error}"
        ))
        .with_provider("gemini_web_compatible")
        .with_code("gemini_web_request_serialize_failed")
    })?;
    let f_req = serde_json::to_string(&vec![Value::Null, Value::String(inner_payload)]).map_err(
        |error| {
            GatewayError::server_error(format!(
                "serialize Gemini Web outer request payload: {error}"
            ))
            .with_provider("gemini_web_compatible")
            .with_code("gemini_web_request_serialize_failed")
        },
    )?;

    let mut query = vec![
        ("hl".to_string(), bootstrap.language.clone()),
        ("_reqid".to_string(), request_id.to_string()),
        ("rt".to_string(), "c".to_string()),
    ];
    if let Some(build_label) = bootstrap
        .build_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("bl".to_string(), build_label.to_string()));
    }
    if let Some(session_id) = bootstrap
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("f.sid".to_string(), session_id.to_string()));
    }

    let mut form = vec![("f.req".to_string(), f_req)];
    if let Some(access_token) = bootstrap
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        form.insert(0, ("at".to_string(), access_token.to_string()));
    }

    Ok(GeminiWebRequest { query, form })
}

fn combine_messages_for_gemini_web(messages: &[CanonicalMessage]) -> Result<String, GatewayError> {
    let mut rendered = Vec::new();
    for message in messages {
        let mut parts = Vec::new();
        for part in &message.content {
            match part {
                ContentPart::Text { text } => {
                    if !text.trim().is_empty() {
                        parts.push(text.trim().to_string());
                    }
                }
                ContentPart::ImageUrl { .. } => {
                    return Err(GatewayError::bad_request(
                        "Gemini Web direct replay does not currently support multimodal content.",
                    )
                    .with_code("unsupported_gemini_web_image"));
                }
                ContentPart::Json { .. } | ContentPart::Raw { .. } => {
                    return Err(GatewayError::bad_request(
                        "Gemini Web direct replay does not currently support structured non-text content.",
                    )
                    .with_code("unsupported_gemini_web_non_text"));
                }
            }
        }
        if parts.is_empty() {
            continue;
        }
        let text = parts.join("\n");
        if messages.len() == 1 && matches!(message.role, MessageRole::User) {
            rendered.push(text);
        } else {
            rendered.push(format!("{}: {}", gemini_web_role_label(message.role), text));
        }
    }
    Ok(rendered.join("\n\n"))
}

fn gemini_web_role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn current_reqid() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    10_000 + (millis % 89_999)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;
    use crate::protocol::canonical::ProtocolFamily;

    fn make_text_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            requested_model: Some("gemini-web-fixture".to_string()),
            endpoint_kind: EndpointKind::ChatCompletions,
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "reply with exactly: gemini web fixture ok".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn packs_text_request_into_f_req_form() {
        let bootstrap = GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "en".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };
        let request = pack_request(&make_text_request(), "gemini-web-fixture", &bootstrap).unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "bl" && value == "bl-1"));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "f.sid" && value == "sid-1"));
        assert!(request
            .form
            .iter()
            .any(|(key, value)| key == "at" && value == "at-1"));
        let form_payload = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value.as_str())
            .unwrap();
        assert!(form_payload.contains("reply with exactly"));
        assert!(form_payload.contains("[\\\"reply with exactly: gemini web fixture ok\\\",0"));
    }

    #[test]
    fn legacy_pack_gemini_web_delegates_to_modular_owner() {
        let bootstrap = GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "en".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };
        let request = crate::protocol::gemini_web::pack_gemini_web(
            &make_text_request(),
            "gemini-web-fixture",
            &bootstrap,
        )
        .unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "bl" && value == "bl-1"));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "f.sid" && value == "sid-1"));
        assert!(request
            .form
            .iter()
            .any(|(key, value)| key == "at" && value == "at-1"));
    }
}
