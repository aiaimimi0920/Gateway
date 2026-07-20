use rquest::Method;

use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::gemini::api as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    if !surface::supports_endpoint(req.endpoint_kind) {
        return Err(GatewayError::bad_request(
            "Gemini API modular adapters currently support only chat/messages/responses/completions style requests",
        )
        .with_code("unsupported_gemini_api_modular_endpoint"));
    }

    let path = resolve_path(payload, model, stream);

    Ok(RequestPlan {
        method: Method::POST,
        url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
        query: surface::default_query(stream),
        body: Some(surface::build_text_request_body(req, model)),
        response_kind: req.endpoint_kind,
    })
}

fn resolve_path(payload: &ProviderAccountPayload, model: &str, stream: bool) -> String {
    let default_path = surface::default_path(model, stream);
    payload
        .chat_completions_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace("{model}", model))
        .unwrap_or(default_path)
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
            adapter: crate::protocol::gemini::shared::GEMINI_API_MODULAR_ADAPTER.to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
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

    fn make_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("gemini-2.5-pro".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello modular gemini".to_string(),
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

    fn assert_stream_generate_content_plan(plan: &RequestPlan) {
        assert_eq!(
            plan.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent"
        );
        assert_eq!(plan.query, vec![("alt".to_string(), "sse".to_string())]);
        assert!(plan.body.as_ref().unwrap()["contents"].is_array());
    }

    #[test]
    fn modular_gemini_api_request_plan_builds_generate_content_path() {
        let plan = build_request_plan(&make_payload(), &make_request(), "gemini-2.5-pro", false)
            .expect("plan");
        assert_eq!(plan.method, Method::POST);
        assert!(plan.url.contains(":generateContent"));
        assert_eq!(plan.query, surface::default_query(false));
        assert!(plan
            .body
            .as_ref()
            .and_then(|body| body.get("stream"))
            .is_none());
        assert!(plan
            .body
            .as_ref()
            .and_then(|body| body.get("model"))
            .is_none());
        assert_eq!(
            plan.body
                .as_ref()
                .and_then(|body| body.get("contents"))
                .and_then(|value| value.as_array())
                .and_then(|items| items.first())
                .and_then(|item| item.get("parts"))
                .and_then(|value| value.as_array())
                .and_then(|parts| parts.first())
                .and_then(|part| part.get("text"))
                .and_then(|value| value.as_str()),
            Some("hello modular gemini")
        );
    }

    #[test]
    fn plan_gemini_api_compatible_url_and_body() {
        let mut req = make_request();
        req.stream = true;
        let plan = build_request_plan(&make_payload(), &req, "gemini-2.5-pro", true).unwrap();
        assert_stream_generate_content_plan(&plan);
    }
}
