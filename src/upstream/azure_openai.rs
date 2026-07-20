use crate::error::GatewayError;
use crate::protocol::azure_openai as surface;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
#[cfg(feature = "family-openai-compatible-official-api")]
use crate::upstream::openai_compatible_official_api_common as openai_compatible_common;

pub fn owns_payload(payload: &ProviderAccountPayload) -> bool {
    surface::owns_payload(payload)
}

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    if !owns_payload(payload) {
        return Err(GatewayError::bad_request(
            "Azure OpenAI request planner received a payload that does not belong to the Azure OpenAI line.",
        )
        .with_code("unsupported_azure_openai_payload"));
    }
    if !surface::supports_endpoint(req.endpoint_kind) {
        return Err(GatewayError::bad_request(
            "Azure OpenAI currently supports only OpenAI-family text, embeddings, audio, and image endpoints.",
        )
        .with_code("unsupported_azure_openai_endpoint"));
    }
    #[cfg(not(feature = "family-openai-compatible-official-api"))]
    {
        return Err(
            GatewayError::conflict("Azure OpenAI common family was compiled out.")
                .with_code("gateway_provider_line_compiled_out"),
        );
    }
    #[cfg(feature = "family-openai-compatible-official-api")]
    openai_compatible_common::build_request_plan(payload, req, model, stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use rquest::Method;
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "openai_compatible".to_string(),
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
            auth_mode: Some("api-key".to_string()),
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: Some("/responses".to_string()),
            chat_completions_path: Some("/chat/completions".to_string()),
            completions_path: Some("/deployments/gpt/legacy/completions".to_string()),
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
            requested_model: Some("gpt-4.1".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello azure".to_string(),
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
    fn build_request_plan_supports_v1_style_paths() {
        let payload = make_payload("https://example.openai.azure.com/openai/v1");
        let plan = build_request_plan(
            &payload,
            &make_request(EndpointKind::ChatCompletions),
            "gpt-4.1",
            false,
        )
        .expect("azure v1 plan");
        assert_eq!(plan.method, Method::POST);
        assert_eq!(
            plan.url,
            "https://example.openai.azure.com/openai/v1/chat/completions"
        );
    }

    #[test]
    fn build_request_plan_preserves_legacy_deployment_path_override() {
        let payload = make_payload("https://example.openai.azure.com/openai");
        let plan = build_request_plan(
            &payload,
            &make_request(EndpointKind::Completions),
            "gpt-4.1",
            false,
        )
        .expect("azure legacy plan");
        assert_eq!(
            plan.url,
            "https://example.openai.azure.com/openai/deployments/gpt/legacy/completions"
        );
    }
}
