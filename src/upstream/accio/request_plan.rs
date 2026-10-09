use crate::error::GatewayError;
use crate::protocol::accio;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::Method;
use serde_json::json;

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
    if let Some(serde_json::Value::Object(body)) = plan.body.as_mut() {
        if let Some(utdid) = payload
            .headers
            .get("utdid")
            .or_else(|| payload.headers.get("x-utdid"))
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            body.entry("utdid".to_string())
                .or_insert_with(|| json!(utdid));
        }
        let version = super::headers::effective_version(payload);
        body.entry("version".to_string())
            .or_insert_with(|| json!(version));
    }
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
            headers: HashMap::new(),
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
