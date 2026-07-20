#[cfg(not(feature = "line-grok-web-reverse-api"))]
mod tests {
    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::grok;
    use std::collections::HashMap;

    fn make_payload() -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: "grok_compatible".to_string(),
            base_url: "https://grok.com".to_string(),
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

    #[test]
    fn unsupported_grok_request_plan_reports_compiled_out() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("grok-3".to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::Value::Null,
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };
        let error = grok::build_request_plan(&make_payload(), &req, "grok-3")
            .expect_err("grok line disabled");
        assert_eq!(error.http_status, Some(409));
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(error.message.contains("grok_web"));
    }
}
