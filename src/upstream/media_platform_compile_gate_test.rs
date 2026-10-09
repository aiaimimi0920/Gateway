#[cfg(any(
    not(feature = "line-suno-web-reverse-api"),
    not(feature = "line-udio-web-reverse-api"),
    not(feature = "line-lumalabs-web-reverse-api")
))]
mod tests {
    use std::collections::HashMap;

    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};
    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::client::UpstreamClient;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "redacted".to_string(),
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
            search_query_field: None,
            fetch_path: None,
            fetch_urls_field: None,
            research_path: None,
            balance_path: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some("test-model".to_string()),
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
        }
    }

    fn assert_compiled_out(error: crate::error::GatewayError, expected_line: &str) {
        assert_eq!(
            error.code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
        assert!(
            error.message.contains(expected_line),
            "expected error message to mention {expected_line}, got {}",
            error.message
        );
    }

    #[cfg(not(feature = "line-suno-web-reverse-api"))]
    #[test]
    fn suno_request_plan_reports_compiled_out_when_line_disabled() {
        let error = UpstreamClient::build_request_plan(
            &make_payload("suno_compatible", "https://suno.com"),
            &make_request(EndpointKind::MusicGenerations),
            "chirp-v3-5",
            false,
        )
        .expect_err("suno request plan should be compiled out");

        assert_compiled_out(error, "suno");
    }

    #[cfg(not(feature = "line-udio-web-reverse-api"))]
    #[test]
    fn udio_request_plan_reports_compiled_out_when_line_disabled() {
        let error = UpstreamClient::build_request_plan(
            &make_payload("udio_compatible", "https://www.udio.com"),
            &make_request(EndpointKind::MusicGenerations),
            "udio-music",
            false,
        )
        .expect_err("udio request plan should be compiled out");

        assert_compiled_out(error, "udio");
    }

    #[cfg(not(feature = "line-lumalabs-web-reverse-api"))]
    #[test]
    fn lumalabs_request_plan_reports_compiled_out_when_line_disabled() {
        let error = UpstreamClient::build_request_plan(
            &make_payload("lumalabs_compatible", "https://app.lumalabs.ai"),
            &make_request(EndpointKind::ImagesGenerations),
            "uni-1",
            false,
        )
        .expect_err("lumalabs request plan should be compiled out");

        assert_compiled_out(error, "lumalabs");
    }
}
