use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::chatgpt::official_api as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
#[cfg(feature = "family-openai-compatible-official-api")]
use crate::upstream::openai_compatible_official_api_common as openai_compatible_common;

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    surface::is_chatgpt_official_api_base_url(&payload.base_url)
        || surface::is_chatgpt_codex_backend_base_url(&payload.base_url)
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    if !owns_payload(payload) {
        return Err(GatewayError::bad_request(
            "ChatGPT official API request planner only owns ChatGPT official API and Codex backend lines.",
        )
        .with_code("unsupported_chatgpt_official_api_payload"));
    }
    if !supports_request_plan_endpoint(req.endpoint_kind) {
        return Err(GatewayError::bad_request(
            "ChatGPT official API adapters currently support only OpenAI-family text, embeddings, audio, and image endpoints.",
        )
        .with_code("unsupported_chatgpt_official_api_endpoint"));
    }
    #[cfg(not(feature = "family-openai-compatible-official-api"))]
    {
        return Err(
            GatewayError::conflict("ChatGPT official API common family was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    #[cfg(feature = "family-openai-compatible-official-api")]
    openai_compatible_common::build_request_plan(payload, req, model, stream)
}

fn supports_request_plan_endpoint(endpoint_kind: EndpointKind) -> bool {
    surface::supports_endpoint(endpoint_kind) || endpoint_kind == EndpointKind::Messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole, ProtocolFamily};
    use rquest::Method;
    use serde_json::Value;
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "openai_compatible".to_string(),
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

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello chatgpt official".to_string(),
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
    fn owns_payload_covers_official_and_codex_base_urls() {
        assert!(owns_payload(&make_payload("https://api.openai.com/v1")));
        assert!(owns_payload(&make_payload(
            "https://chatgpt.com/backend-api/codex"
        )));
        assert!(!owns_payload(&make_payload("https://api.deepseek.com")));
    }

    #[test]
    fn build_request_plan_uses_official_chat_completions_path() {
        let payload = make_payload("https://api.openai.com");
        let req = make_request(EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "gpt-5.4", false).expect("plan");
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
    }

    #[test]
    fn build_request_plan_keeps_codex_backend_responses_bridge() {
        let mut payload = make_payload("https://chatgpt.com/backend-api/codex");
        payload.responses_path = Some("/responses".to_string());
        let req = make_request(EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "gpt-5.4", false).expect("plan");
        assert_eq!(plan.url, "https://chatgpt.com/backend-api/codex/responses");
        assert!(plan
            .body
            .as_ref()
            .and_then(|body| body.get("input"))
            .is_some());
    }

    #[test]
    fn build_request_plan_allows_messages_ingress_bridge() {
        let mut payload = make_payload("https://api.openai.com");
        payload.responses_path = Some("/v1/responses".to_string());
        let req = make_request(EndpointKind::Messages);
        let plan = build_request_plan(&payload, &req, "gpt-5.4", true).expect("plan");
        assert_eq!(plan.url, "https://api.openai.com/v1/responses");
        assert_eq!(
            plan.body.as_ref().and_then(|body| body.get("stream")),
            Some(&Value::Bool(true))
        );
    }
}
