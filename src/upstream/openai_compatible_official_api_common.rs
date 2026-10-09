use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::{openai, responses};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::{merge_model_and_stream_into_body, RequestPlan};
use crate::upstream::openai_compatible_common::build_audio_transcription_request_body;
use rquest::Method;

pub fn build_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    stream: bool,
) -> Result<RequestPlan, GatewayError> {
    let use_responses_bridge = payload.bridges_openai_text_endpoint_to_responses(req.endpoint_kind);
    let use_chat_bridge_for_responses =
        payload.bridges_openai_responses_to_chat_completions(req.endpoint_kind);
    let use_responses_minimal_pack = req.endpoint_kind == EndpointKind::Responses
        && payload.prefers_forced_streaming_responses(req.endpoint_kind);
    let responses_request_was_tool_injected = req.endpoint_kind == EndpointKind::Responses
        && req.raw_body.get("tools").is_some()
        && req.tools.is_empty();
    let use_chat_bridge_for_messages =
        req.endpoint_kind == EndpointKind::Messages && !use_responses_bridge;
    let use_chat_bridge_for_completions = req.endpoint_kind == EndpointKind::Completions
        && !use_responses_bridge
        && payload.completions_path.is_none();

    // API bases commonly already end in /v1. Adjust only inferred defaults;
    // explicit endpoint paths retain their existing base-relative semantics.
    let default_path = |path: &'static str| {
        if payload.base_url.trim_end_matches('/').ends_with("/v1") {
            path.strip_prefix("/v1").unwrap_or(path)
        } else {
            path
        }
    };
    let path = match req.endpoint_kind {
        EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Completions
            if use_responses_bridge =>
        {
            payload
                .responses_path
                .as_deref()
                .unwrap_or(default_path("/v1/responses"))
        }
        EndpointKind::Messages if use_chat_bridge_for_messages => payload
            .chat_completions_path
            .as_deref()
            .unwrap_or(default_path("/v1/chat/completions")),
        EndpointKind::Completions if use_chat_bridge_for_completions => payload
            .chat_completions_path
            .as_deref()
            .unwrap_or(default_path("/v1/chat/completions")),
        EndpointKind::Responses if use_chat_bridge_for_responses => payload
            .chat_completions_path
            .as_deref()
            .unwrap_or(default_path("/v1/chat/completions")),
        EndpointKind::Responses => payload
            .responses_path
            .as_deref()
            .unwrap_or(default_path("/v1/responses")),
        EndpointKind::Completions => payload
            .completions_path
            .as_deref()
            .unwrap_or(default_path("/v1/completions")),
        EndpointKind::Embeddings => payload
            .embeddings_path
            .as_deref()
            .unwrap_or(default_path("/v1/embeddings")),
        EndpointKind::AudioTranscriptions => payload
            .audio_transcriptions_path
            .as_deref()
            .unwrap_or(default_path("/v1/audio/transcriptions")),
        EndpointKind::AudioSpeech => payload
            .audio_speech_path
            .as_deref()
            .unwrap_or(default_path("/v1/audio/speech")),
        _ => payload
            .chat_completions_path
            .as_deref()
            .unwrap_or(default_path("/v1/chat/completions")),
    };

    let body = if payload.is_discovered_native(req) {
        merge_model_and_stream_into_body(req.raw_body.clone(), model, Some(stream))
    } else {
        match req.endpoint_kind {
            EndpointKind::ChatCompletions | EndpointKind::Messages | EndpointKind::Completions
                if use_responses_bridge =>
            {
                responses::pack_responses_bridge(req, model, stream)
            }
            EndpointKind::Messages if use_chat_bridge_for_messages => {
                openai::pack_openai(req, model, stream)
            }
            EndpointKind::Completions if use_chat_bridge_for_completions => {
                openai::pack_openai(req, model, stream)
            }
            EndpointKind::Responses if use_chat_bridge_for_responses => {
                openai::pack_openai(req, model, stream)
            }
            EndpointKind::Responses
                if use_responses_minimal_pack || responses_request_was_tool_injected =>
            {
                responses::pack_responses(req, model, stream)
            }
            EndpointKind::Responses => {
                merge_model_and_stream_into_body(req.raw_body.clone(), model, Some(stream))
            }
            EndpointKind::Completions => {
                merge_model_and_stream_into_body(req.raw_body.clone(), model, Some(stream))
            }
            EndpointKind::Embeddings | EndpointKind::AudioSpeech => {
                merge_model_and_stream_into_body(req.raw_body.clone(), model, None)
            }
            EndpointKind::AudioTranscriptions => {
                build_audio_transcription_request_body(req, model)?
            }
            _ => openai::pack_openai(req, model, stream),
        }
    };

    Ok(RequestPlan {
        method: Method::POST,
        url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
        query: Vec::new(),
        body: Some(body),
        response_kind: req.endpoint_kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };
    use std::collections::HashMap;

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
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

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello".to_string(),
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

    fn make_responses_request(
        messages: Vec<CanonicalMessage>,
        raw_body: serde_json::Value,
    ) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::Responses,
            requested_model: Some("gpt-5.4".to_string()),
            stream: false,
            messages,
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body,
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn assert_response_url(plan: &RequestPlan, expected: &str) {
        assert_eq!(plan.url, expected);
    }

    fn assert_openai_chat_surface_plan(plan: &RequestPlan, expected_url: &str, model: &str) {
        assert_response_url(plan, expected_url);
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
        assert_eq!(plan.body.as_ref().unwrap()["model"], model);
    }

    fn assert_openai_responses_bridge_plan(plan: &RequestPlan, expected_url: &str) {
        assert_response_url(plan, expected_url);
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
        assert!(plan.body.as_ref().unwrap().get("input").is_some());
        assert!(plan.body.as_ref().unwrap().get("instructions").is_some());
    }

    #[test]
    fn plan_openai_chat_completions_url() {
        let payload = make_payload("https://api.openai.com");
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "gpt-4o", false).unwrap();
        assert_openai_chat_surface_plan(
            &plan,
            "https://api.openai.com/v1/chat/completions",
            "gpt-4o",
        );
    }

    #[test]
    fn plan_trailing_slash_stripped_from_base_url() {
        let payload = make_payload("https://api.openai.com/");
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "gpt-4o", false).unwrap();
        assert!(!plan.url.contains("//v1"));
    }

    #[test]
    fn plan_default_endpoints_accept_versioned_api_bases() {
        let endpoints = [
            (EndpointKind::ChatCompletions, "/chat/completions"),
            (EndpointKind::Messages, "/chat/completions"),
            (EndpointKind::Completions, "/chat/completions"),
            (EndpointKind::Responses, "/responses"),
            (EndpointKind::Embeddings, "/embeddings"),
            (EndpointKind::AudioSpeech, "/audio/speech"),
            (EndpointKind::AudioTranscriptions, "/audio/transcriptions"),
        ];
        for base in [
            "https://partner.example/v1",
            "https://partner.example/v1/",
            "https://partner.example/proxy/v1/",
        ] {
            for (endpoint, suffix) in endpoints {
                let payload = make_payload(base);
                let mut req = make_request(ProtocolFamily::OpenAi, endpoint);
                req.raw_body = serde_json::json!({"file": {"base64": "aGk="}});
                let plan = build_request_plan(&payload, &req, "partner-chat", false).unwrap();
                assert_eq!(plan.url, format!("{}{suffix}", base.trim_end_matches('/')));
            }
        }
    }

    #[test]
    fn plan_default_version_prefix_requires_exact_final_segment() {
        for base in [
            "https://partner.example",
            "https://partner.example/proxy",
            "https://partner.example/v10",
            "https://partner.example/my-v1",
            "https://partner.example/v1/proxy",
        ] {
            let payload = make_payload(base);
            let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
            let plan = build_request_plan(&payload, &req, "partner-chat", false).unwrap();
            assert_eq!(plan.url, format!("{base}/v1/chat/completions"));
        }
    }

    #[test]
    fn plan_explicit_endpoint_paths_remain_relative_to_configured_base() {
        for path in [
            "/chat/completions",
            "/v2/chat/completions",
            "/v1/chat/completions",
        ] {
            let mut payload = make_payload("https://partner.example/proxy/v1/");
            payload.chat_completions_path = Some(path.to_string());
            for endpoint in [
                EndpointKind::ChatCompletions,
                EndpointKind::Messages,
                EndpointKind::Completions,
                EndpointKind::Responses,
            ] {
                let req = make_request(ProtocolFamily::OpenAi, endpoint);
                let plan = build_request_plan(&payload, &req, "partner-chat", false).unwrap();
                assert_eq!(plan.url, format!("https://partner.example/proxy/v1{path}"));
            }
        }
    }

    #[test]
    fn plan_explicit_non_chat_paths_are_not_normalized() {
        let mut payload = make_payload("https://partner.example/v1/");
        payload.responses_path = Some("/v2/responses".to_string());
        payload.completions_path = Some("/v2/completions".to_string());
        payload.embeddings_path = Some("/v2/embeddings".to_string());
        payload.audio_speech_path = Some("/v2/audio/speech".to_string());
        payload.audio_transcriptions_path = Some("/v2/audio/transcriptions".to_string());
        for (endpoint, suffix) in [
            (EndpointKind::Responses, "/v2/responses"),
            (EndpointKind::Completions, "/v2/completions"),
            (EndpointKind::Embeddings, "/v2/embeddings"),
            (EndpointKind::AudioSpeech, "/v2/audio/speech"),
            (
                EndpointKind::AudioTranscriptions,
                "/v2/audio/transcriptions",
            ),
        ] {
            let mut req = make_request(ProtocolFamily::OpenAi, endpoint);
            req.raw_body = serde_json::json!({"file": {"base64": "aGk="}});
            let plan = build_request_plan(&payload, &req, "partner-chat", false).unwrap();
            assert_eq!(plan.url, format!("https://partner.example/v1{suffix}"));
        }
    }

    #[test]
    fn plan_responses_endpoint_url() {
        let payload = make_payload("https://api.openai.com");
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Responses);
        let plan = build_request_plan(&payload, &req, "gpt-4o", false).unwrap();
        assert_response_url(&plan, "https://api.openai.com/v1/responses");
    }

    #[test]
    fn plan_openai_responses_can_bridge_to_chat_completions_path() {
        let mut payload = make_payload("https://spark-api.example.com");
        payload.chat_completions_path = Some("/v2/chat/completions".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Responses);
        let plan = build_request_plan(&payload, &req, "qwen-test", false).unwrap();
        assert_response_url(&plan, "https://spark-api.example.com/v2/chat/completions");
        assert_eq!(plan.body.as_ref().unwrap()["messages"][0]["role"], "user");
        assert_eq!(plan.body.as_ref().unwrap()["stream"], false);
    }

    #[test]
    fn plan_custom_responses_provider_uses_minimal_bridge_body() {
        let mut payload = make_payload("https://chatgpt.com/backend-api/codex");
        payload.responses_path = Some("/responses".to_string());
        let req = make_responses_request(
            vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You are terse.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Say hello".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            serde_json::json!({
                "model": "gpt-5.4",
                "instructions": "You are terse.",
                "input": "Say hello",
            }),
        );

        let plan = build_request_plan(&payload, &req, "gpt-5.4", false).unwrap();
        assert_response_url(&plan, "https://chatgpt.com/backend-api/codex/responses");
        assert!(plan.body.as_ref().unwrap()["input"].is_array());
        assert_eq!(
            plan.body.as_ref().unwrap()["instructions"],
            serde_json::json!("You are terse.")
        );
    }

    #[test]
    fn plan_responses_request_rebuilds_body_after_tool_injection() {
        let mut payload = make_payload("https://fixture.example.com");
        payload.responses_path = Some("/v1/responses".to_string());

        let mut req = make_responses_request(
            vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "<tools>\nTOOL CALL FORMAT".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Use the weather tool for Hangzhou.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            serde_json::json!({
                "model": "xml-fallback-fixture",
                "stream": true,
                "tool_choice": "required",
                "tools": [{
                    "type": "function",
                    "name": "weather",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "city": {"type": "string"}
                        }
                    }
                }],
                "instructions": "You are terse.",
                "input": "Use the weather tool for Hangzhou."
            }),
        );
        req.requested_model = Some("xml-fallback-fixture".to_string());
        req.stream = true;

        let plan = build_request_plan(&payload, &req, "xml-fallback-fixture", true).unwrap();
        let body = plan.body.as_ref().unwrap();
        assert!(body.get("tools").is_none());
        assert!(body.get("tool_choice").is_none());
        assert_eq!(
            body.get("model"),
            Some(&serde_json::json!("xml-fallback-fixture"))
        );
        assert_eq!(body.get("stream"), Some(&serde_json::json!(true)));
        assert!(body
            .get("instructions")
            .and_then(|value| value.as_str())
            .is_some_and(|value| value.contains("<tools>")));
    }

    #[test]
    fn plan_openai_chat_can_bridge_to_responses_path() {
        let mut payload = make_payload("https://chatgpt.com/backend-api/codex");
        payload.responses_path = Some("/responses".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
        let plan = build_request_plan(&payload, &req, "gpt-5.4", false).unwrap();
        assert_openai_responses_bridge_plan(
            &plan,
            "https://chatgpt.com/backend-api/codex/responses",
        );
        assert_eq!(plan.body.as_ref().unwrap()["model"], "gpt-5.4");
    }

    #[test]
    fn plan_openai_messages_can_bridge_to_responses_path() {
        let mut payload = make_payload("https://chatgpt.com/backend-api/codex");
        payload.responses_path = Some("/responses".to_string());
        let req = make_request(ProtocolFamily::Anthropic, EndpointKind::Messages);
        let plan = build_request_plan(&payload, &req, "gpt-5.4", true).unwrap();
        assert_response_url(&plan, "https://chatgpt.com/backend-api/codex/responses");
        assert_eq!(plan.body.as_ref().unwrap()["stream"], true);
        assert!(plan.body.as_ref().unwrap().get("input").is_some());
    }

    #[test]
    fn plan_openai_completions_can_bridge_to_responses_path() {
        let mut payload = make_payload("https://chatgpt.com/backend-api/codex");
        payload.responses_path = Some("/responses".to_string());
        let req = make_request(ProtocolFamily::OpenAi, EndpointKind::Completions);
        let plan = build_request_plan(&payload, &req, "gpt-5.4", false).unwrap();
        assert_response_url(&plan, "https://chatgpt.com/backend-api/codex/responses");
        assert!(plan.body.as_ref().unwrap().get("input").is_some());
    }
}
