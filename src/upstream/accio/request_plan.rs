use crate::error::GatewayError;
use crate::protocol::accio;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::Method;
use serde_json::json;

/// These fields belong to credential material, never to caller or generic body extras.
pub fn is_reserved_payload_extra_key(key: &str) -> bool {
    matches!(
        key,
        "token"
            | "accessToken"
            | "access_token"
            | "apiKey"
            | "api_key"
            | "refreshToken"
            | "refresh_token"
            | "sgcookie"
            | "utdid"
            | "x-utdid"
    )
}

fn body_token(payload: &ProviderAccountPayload) -> Result<&str, GatewayError> {
    if !payload.api_key.trim().is_empty() {
        // Runtime refresh overrides only api_key; old extra_body copies may be stale or malformed.
        return Ok(&payload.api_key);
    }
    let mut selected = None;
    for key in ["token", "accessToken"] {
        let Some(value) = payload.extra_body.as_ref().and_then(|extra| extra.get(key)) else {
            continue;
        };
        let value = value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                GatewayError::bad_request("Accio credential body token must be a nonempty string")
                    .with_code("accio_invalid_body_token")
            })?;
        if selected.is_some_and(|previous| previous != value) {
            return Err(
                GatewayError::bad_request("Accio credential body token aliases disagree")
                    .with_code("accio_ambiguous_body_token"),
            );
        }
        selected = Some(value);
    }
    selected.ok_or_else(|| {
        GatewayError::bad_request("Accio credential body token is required")
            .with_code("accio_missing_body_token")
    })
}

fn bind_credentials(
    payload: &ProviderAccountPayload,
    body: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), GatewayError> {
    let token = body_token(payload)?;
    // Match the existing header owner's source and precedence; never invent a device identifier.
    let utdid = payload
        .headers
        .get("utdid")
        .or_else(|| payload.headers.get("x-utdid"))
        .filter(|value| {
            !value.trim().is_empty() && rquest::header::HeaderValue::from_str(value).is_ok()
        })
        .ok_or_else(|| {
            GatewayError::bad_request("Accio credential requires a valid utdid header")
                .with_code("accio_invalid_utdid")
        })?;
    body.retain(|key, _| !is_reserved_payload_extra_key(key));
    body.insert("token".into(), json!(token));
    body.insert("utdid".into(), json!(utdid));
    Ok(())
}

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Accio adapters use the dedicated web_reverse_api replay line and are not supported by the generic planner when that line is unavailable.",
    )
    .with_code("unsupported_accio_request_plan")
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    let path = payload
        .responses_path
        .as_deref()
        .unwrap_or("/api/adk/llm/generateContent");
    let mut plan = RequestPlan {
        method: Method::POST,
        url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
        query: Vec::new(),
        body: Some(accio::pack_accio(req, model, stream)),
        response_kind: req.endpoint_kind,
    };
    let body = plan
        .body
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
        .expect("Accio packer always returns an object");
    bind_credentials(payload, body)?;
    let version = super::headers::effective_version(payload);
    body.entry("version".to_string())
        .or_insert_with(|| json!(version));
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            call_flow: None,
            request_client_profile: None,
            discovered_protocols: Vec::new(),
            adapter: "accio_compatible".to_string(),
            base_url: base_url.to_string(),
            api_key: "tok".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::from([("utdid".into(), "synthetic-device".into())]),
            auth_mode: Some("bearer".to_string()),
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("claude-sonnet-4-6".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello accio".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn plan_accio_compatible_url_and_body() {
        let mut payload = make_payload("https://phoenix-gw.alibaba.com");
        payload
            .headers
            .insert("utdid".to_string(), "utd-accio-live".to_string());
        payload
            .headers
            .insert("version".to_string(), "0.5.6".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "claude-sonnet-4-6", true).unwrap();
        assert_eq!(
            plan.url,
            "https://phoenix-gw.alibaba.com/api/adk/llm/generateContent"
        );
        assert_eq!(plan.body.as_ref().unwrap()["model"], "claude-sonnet-4-6");
        assert!(plan.body.as_ref().unwrap()["contents"].is_array());
        assert!(plan.body.as_ref().unwrap().get("request_id").is_some());
        assert!(plan.body.as_ref().unwrap().get("message_id").is_some());
        assert_eq!(plan.body.as_ref().unwrap()["utdid"], "utd-accio-live");
        assert_eq!(plan.body.as_ref().unwrap()["version"], "0.5.9");
    }

    #[test]
    fn plan_accio_custom_path() {
        let mut payload = make_payload("https://phoenix-gw.alibaba.com");
        payload.responses_path = Some("/custom/endpoint".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "claude-sonnet-4-6", false).unwrap();
        assert_eq!(plan.url, "https://phoenix-gw.alibaba.com/custom/endpoint");
    }
}
