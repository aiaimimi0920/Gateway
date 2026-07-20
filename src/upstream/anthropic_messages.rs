use crate::error::GatewayError;
use crate::protocol::anthropic;
use crate::protocol::anthropic_messages as surface;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::routing::candidate::ProviderAccountPayload;
#[cfg(feature = "family-anthropic-compatible-official-api")]
use crate::upstream::anthropic_compatible_official_api_common as anthropic_common;
use crate::upstream::common::RequestPlan;
use serde_json::Value;

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
            "Anthropic Messages request planner received a payload that does not belong to the Anthropic Messages line.",
        )
        .with_code("unsupported_anthropic_messages_payload"));
    }
    if !surface::supports_endpoint(req.endpoint_kind) {
        return Err(GatewayError::bad_request(
            "Anthropic Messages currently supports only text chat endpoints.",
        )
        .with_code("unsupported_anthropic_messages_endpoint"));
    }
    #[cfg(not(feature = "family-anthropic-compatible-official-api"))]
    {
        return Err(GatewayError::conflict(
            "Anthropic-compatible official API common family was compiled out.",
        )
        .with_code("gateway_provider_line_compiled_out"));
    }
    #[cfg(feature = "family-anthropic-compatible-official-api")]
    anthropic_common::build_request_plan(payload, req, model, stream)
}

pub async fn accumulate_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    anthropic::accumulate_anthropic_stream(response, model).await
}

pub fn unpack_response(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    anthropic::unpack_anthropic_response(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use std::collections::HashMap;

    fn make_payload() -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "anthropic_compatible".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "tok".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: Some("x-api-key".to_string()),
            anthropic_version: Some("2023-06-01".to_string()),
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: Some("/v1/messages".to_string()),
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
            protocol_family: ProtocolFamily::Anthropic,
            endpoint_kind,
            requested_model: Some("claude-sonnet-4-6".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello anthropic".to_string(),
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

    fn make_request_with_protocol(
        protocol_family: ProtocolFamily,
        endpoint_kind: EndpointKind,
    ) -> CanonicalRelayRequest {
        let mut request = make_request(endpoint_kind);
        request.protocol_family = protocol_family;
        request
    }

    fn assert_anthropic_messages_plan(plan: &RequestPlan, model: &str) {
        assert_eq!(plan.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(plan.response_kind, EndpointKind::Messages);
        assert_eq!(plan.body.as_ref().unwrap()["model"], model);
        assert!(plan.body.as_ref().unwrap().get("max_tokens").is_some());
    }

    #[test]
    fn build_request_plan_uses_messages_path() {
        let plan = build_request_plan(
            &make_payload(),
            &make_request(EndpointKind::Messages),
            "claude-sonnet-4-6",
            false,
        )
        .expect("anthropic messages plan");
        assert_eq!(plan.url, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn plan_anthropic_messages_url() {
        let plan = build_request_plan(
            &make_payload(),
            &make_request(EndpointKind::Messages),
            "claude-3-5-sonnet-20241022",
            false,
        )
        .unwrap();
        assert_anthropic_messages_plan(&plan, "claude-3-5-sonnet-20241022");
    }

    #[test]
    fn plan_anthropic_completions_can_bridge_to_messages() {
        let plan = build_request_plan(
            &make_payload(),
            &make_request_with_protocol(ProtocolFamily::OpenAi, EndpointKind::Completions),
            "claude-3-5-sonnet-20241022",
            false,
        )
        .unwrap();
        assert_anthropic_messages_plan(&plan, "claude-3-5-sonnet-20241022");
    }
}
