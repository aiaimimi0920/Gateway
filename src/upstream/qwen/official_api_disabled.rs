use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::qwen::official_api as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    surface::owns_payload(payload)
}

pub fn build_request_plan(
    _payload: &ProviderAccountPayload,
    _req: &CanonicalRelayRequest,
    _model: &str,
    _stream: bool,
) -> Result<RequestPlan, GatewayError> {
    Err(
        GatewayError::conflict("Qwen official API implementation line was compiled out.")
            .with_code("gateway_provider_line_compiled_out"),
    )
}

#[cfg(test)]
mod tests {
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole,
        ProtocolFamily,
    };
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::client::UpstreamClient;
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
    fn plan_qwen_dashscope_chat_returns_compiled_out_when_official_line_disabled() {
        let mut payload = make_payload(
            "openai_compatible",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        );
        payload.chat_completions_path = Some("/chat/completions".to_string());
        payload.responses_path = Some("/responses".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let error = UpstreamClient::build_request_plan(&payload, &req, "qwen3.5-flash", false)
            .expect_err("qwen official request plan should be compiled out");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
    }

    #[test]
    fn plan_qwen_dashscope_responses_return_compiled_out_when_official_line_disabled() {
        let mut payload = make_payload(
            "openai_compatible",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        );
        payload.chat_completions_path = Some("/chat/completions".to_string());
        payload.responses_path = Some("/responses".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Responses);
        let error = UpstreamClient::build_request_plan(&payload, &req, "qwen3.5-flash", false)
            .expect_err("qwen official request plan should be compiled out");
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
    }
}
