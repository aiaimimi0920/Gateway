use rquest::Method;
use serde_json::Value;

use crate::error::GatewayError;
use crate::protocol::accio;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse, EndpointKind};
use crate::protocol::cohere;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    match req.endpoint_kind {
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Responses => {
            let path = payload
                .chat_completions_path
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(cohere::default_path(stream));
            Ok(RequestPlan {
                method: Method::POST,
                url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
                query: Vec::new(),
                body: Some(cohere::pack_cohere(req, model, stream)),
                response_kind: EndpointKind::ChatCompletions,
            })
        }
        _ => Err(GatewayError::bad_request(
            "Cohere Chat adapters currently support only chat/messages/responses style requests",
        )
        .with_code("unsupported_cohere_endpoint")),
    }
}

pub async fn accumulate_stream(
    response: rquest::Response,
    model: &str,
) -> Result<CanonicalRelayResponse, GatewayError> {
    accio::accumulate_accio_stream(response, model).await
}

pub fn unpack_response(body: &Value) -> Result<CanonicalRelayResponse, GatewayError> {
    accio::unpack_accio_response(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole, ProtocolFamily};
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "cohere_compatible".to_string(),
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

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::CohereChat,
            endpoint_kind,
            requested_model: Some("command-r".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello cohere".to_string(),
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

    fn assert_cohere_chat_surface_plan(plan: &RequestPlan) {
        assert_eq!(plan.url, "https://api.cohere.com/v2/chat");
        assert_eq!(plan.body.as_ref().unwrap()["model"], "command-r");
        assert!(plan.body.as_ref().unwrap()["messages"].is_array());
    }

    #[test]
    fn build_request_plan_uses_v2_chat_default_path() {
        let plan = build_request_plan(
            &make_payload("https://api.cohere.ai"),
            &make_request(EndpointKind::ChatCompletions),
            "command-r",
            false,
        )
        .expect("cohere plan");
        assert_eq!(plan.url, "https://api.cohere.ai/v2/chat");
    }

    #[test]
    fn plan_cohere_compatible_url_and_body() {
        let payload = make_payload("https://api.cohere.com");
        let req = make_request(EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "command-r", false).unwrap();
        assert_cohere_chat_surface_plan(&plan);
    }
}
