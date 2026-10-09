use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;

/// Resolve the effective model name: use the requested model when present,
/// otherwise fall back to the provider default, then to the hardcoded default.
pub(crate) fn resolve_model<'a>(
    payload: &'a ProviderAccountPayload,
    req: &'a CanonicalRelayRequest,
) -> &'a str {
    req.requested_model
        .as_deref()
        .or(payload.default_model.as_deref())
        .unwrap_or("gpt-4o")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use serde_json::json;
    use std::collections::HashMap;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
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
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
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
    fn resolve_model_prefers_requested_over_default() {
        let mut payload = make_payload("openai_compatible", "https://api.openai.com");
        payload.default_model = Some("gpt-3.5-turbo".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        assert_eq!(resolve_model(&payload, &req), "gpt-4o");
    }

    #[test]
    fn resolve_model_falls_back_to_payload_default() {
        let mut payload = make_payload("openai_compatible", "https://api.openai.com");
        payload.default_model = Some("gpt-3.5-turbo".to_string());
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        req.requested_model = None;
        assert_eq!(resolve_model(&payload, &req), "gpt-3.5-turbo");
    }

    #[test]
    fn resolve_model_falls_back_to_hardcoded_default() {
        let payload = make_payload("openai_compatible", "https://api.openai.com");
        let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        req.requested_model = None;
        assert_eq!(resolve_model(&payload, &req), "gpt-4o");
    }
}
