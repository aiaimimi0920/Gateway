use crate::protocol::canonical::CanonicalRelayRequest;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::Method;

pub(crate) fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> RequestPlan {
    RequestPlan {
        method: Method::POST,
        url: payload.base_url.clone(),
        query: Vec::new(),
        body: Some(req.raw_body.clone()),
        response_kind: req.endpoint_kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{EndpointKind, ProtocolFamily};
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: "custom_http".to_string(),
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
            requested_model: Some("local-model".to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({"hello":"world"}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn assert_custom_http_plan(plan: &RequestPlan, expected_url: &str) {
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, expected_url);
        assert_eq!(plan.body.as_ref().unwrap()["hello"], "world");
    }

    #[test]
    fn plan_custom_http_uses_base_url_verbatim() {
        let plan = build_request_plan(
            &make_payload("https://my-proxy.example.com/api/v2/llm"),
            &make_request(EndpointKind::ChatCompletions),
        );
        assert_custom_http_plan(&plan, "https://my-proxy.example.com/api/v2/llm");
    }
}
