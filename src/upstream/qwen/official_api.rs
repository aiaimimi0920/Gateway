use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::qwen::official_api as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::anthropic_compatible_official_api_common as anthropic_compatible_common;
use crate::upstream::common::RequestPlan;
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
            "Qwen official API request plan received a payload that does not belong to the Qwen official line.",
        )
        .with_code("unsupported_qwen_official_request_plan"));
    }

    match payload.canonical_adapter() {
        "openai_compatible" => {
            if !surface::supports_endpoint(req.endpoint_kind) {
                return Err(GatewayError::bad_request(
                    "Qwen official OpenAI-compatible surfaces currently support only text chat endpoints.",
                )
                .with_code("unsupported_qwen_official_openai_endpoint"));
            }
            openai_compatible_common::build_request_plan(payload, req, model, stream)
        }
        "anthropic_compatible" => anthropic_compatible_common::build_request_plan(
            payload, req, model, stream,
        )
        .map_err(|error| {
            if error.code.as_deref() == Some("unsupported_anthropic_official_endpoint") {
                GatewayError::bad_request(
                    "Qwen Coding Plan Anthropic-compatible surface supports only text chat endpoints.",
                )
                .with_code("unsupported_qwen_official_anthropic_endpoint")
            } else {
                error
            }
        }),
        _ => Err(GatewayError::bad_request(
            "Qwen official request planner supports only OpenAI-compatible and Anthropic-compatible adapters.",
        )
        .with_code("unsupported_qwen_official_adapter")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some("qwen3-coder-plus".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello qwen".to_string(),
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
            extra: std::collections::HashMap::new(),
        }
    }

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "tok".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: std::collections::HashMap::new(),
            auth_mode: None,
            anthropic_version: Some("2023-06-01".to_string()),
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: Some("/responses".to_string()),
            chat_completions_path: Some("/chat/completions".to_string()),
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

    fn assert_qwen_dashscope_openai_path(
        plan: &RequestPlan,
        expected_url: &str,
        expected_response_kind: EndpointKind,
    ) {
        assert_eq!(plan.url, expected_url);
        assert_eq!(plan.response_kind, expected_response_kind);
    }

    #[test]
    fn build_qwen_coding_plan_anthropic_request_plan_uses_messages_path() {
        let plan = build_request_plan(
            &make_payload(
                "anthropic_compatible",
                "https://coding.dashscope.aliyuncs.com/apps/anthropic",
            ),
            &make_request(EndpointKind::Messages),
            "qwen3-coder-plus",
            false,
        )
        .expect("qwen anthropic plan");

        assert_eq!(
            plan.url,
            "https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages"
        );
        assert_eq!(plan.response_kind, EndpointKind::Messages);
    }

    #[cfg(feature = "line-qwen-official-api")]
    #[test]
    fn plan_qwen_dashscope_chat_uses_native_chat_path_without_double_v1() {
        let plan = build_request_plan(
            &make_payload(
                "openai_compatible",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ),
            &make_request(EndpointKind::ChatCompletions),
            "qwen3.5-flash",
            false,
        )
        .unwrap();
        assert_qwen_dashscope_openai_path(
            &plan,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            EndpointKind::ChatCompletions,
        );
    }

    #[cfg(feature = "line-qwen-official-api")]
    #[test]
    fn plan_qwen_dashscope_responses_use_native_responses_path() {
        let plan = build_request_plan(
            &make_payload(
                "openai_compatible",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ),
            &make_request(EndpointKind::Responses),
            "qwen3.5-flash",
            false,
        )
        .unwrap();
        assert_qwen_dashscope_openai_path(
            &plan,
            "https://dashscope.aliyuncs.com/compatible-mode/v1/responses",
            EndpointKind::Responses,
        );
    }
}
