#[path = "aistudio/mod.rs"]
mod modular;

pub use modular::common::{
    bridge_embeddings_response, build_embeddings_request, build_fixture_audio_binary_response,
    build_fixture_canonical_response, build_fixture_embeddings_response,
    build_generate_content_url, build_image_request_body, build_text_request_body,
    build_tts_request_body, config_from_payload, fixture_audio, fixture_image,
    AIStudioEmbeddingsRequest, AIStudioFixtureAudio, AIStudioFixtureImage, AIStudioWebConfig,
    AISTUDIO_DEFAULT_APP_URL,
};
pub use modular::web_reverse::{
    build_browser_request_spec, supports_text_endpoint, AIStudioBrowserRequestSpec,
    AISTUDIO_BROWSER_EXECUTOR_PROVIDER_KEY, AISTUDIO_WEB_REVERSE_ADAPTER,
    AISTUDIO_WEB_REVERSE_PROFILE,
};
pub use modular::{common, web_reverse};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{
        CanonicalMessage, CanonicalRelayRequest, CanonicalTool, CanonicalToolCall, ContentPart,
        EndpointKind, MessageRole, ProtocolFamily,
    };
    use crate::routing::candidate::ProviderAccountPayload;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn make_payload() -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: AISTUDIO_WEB_REVERSE_ADAPTER.to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: "unused".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: Some(
                "credential-runtime/aistudio/main/storage-state.json".to_string(),
            ),
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
            extra_body: Some(HashMap::from([(
                "appUrl".to_string(),
                Value::String(AISTUDIO_DEFAULT_APP_URL.to_string()),
            )])),
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some("gemini-2.5-flash".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "hello aistudio".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn make_weather_tool() -> CanonicalTool {
        CanonicalTool {
            tool_type: "function".to_string(),
            name: Some("weather".to_string()),
            description: Some("Return the current weather for a city.".to_string()),
            input_schema: Some(json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            })),
            raw: HashMap::new(),
        }
    }

    #[test]
    fn config_does_not_require_runtime_state_at_parse_time() {
        let mut payload = make_payload();
        payload.runtime_state_object_key = None;
        let config = config_from_payload(&payload).expect("config without runtime state");
        assert!(config.runtime_state_object_key.is_none());
        assert_eq!(config.cloud_api_key.as_deref(), Some("unused"));
    }

    #[test]
    fn config_allows_fixture_transport_without_runtime_state() {
        let mut payload = make_payload();
        payload.runtime_state_object_key = None;
        payload.extra_body = Some(HashMap::from([(
            "fixtureTransport".to_string(),
            Value::String("direct_http".to_string()),
        )]));
        let config = config_from_payload(&payload).expect("fixture config");
        assert!(config.fixture_transport);
        assert!(config.runtime_state_object_key.is_none());
    }

    #[test]
    fn config_parses_cloud_api_key_from_extra_body() {
        let mut payload = make_payload();
        payload.api_key = String::new();
        payload.extra_body = Some(HashMap::from([
            (
                "appUrl".to_string(),
                Value::String(AISTUDIO_DEFAULT_APP_URL.to_string()),
            ),
            (
                "cloudApiKey".to_string(),
                Value::String("AIzaSyFixtureCloudApiKey000000000000000000".to_string()),
            ),
        ]));
        let config = config_from_payload(&payload).expect("config");
        assert_eq!(
            config.cloud_api_key.as_deref(),
            Some("AIzaSyFixtureCloudApiKey000000000000000000")
        );
    }

    #[test]
    fn build_image_request_body_promotes_image_modality() {
        let request = make_request(EndpointKind::ImagesGenerations);
        let body = build_image_request_body(&request, "gemini-2.5-flash-image");
        assert_eq!(
            body["generationConfig"]["responseModalities"],
            json!(["TEXT", "IMAGE"])
        );
    }

    #[test]
    fn fixture_audio_and_image_are_available() {
        let image = fixture_image().expect("fixture image");
        let audio = fixture_audio();
        assert_eq!(image.mime_type, "image/png");
        assert_eq!(audio.mime_type, "audio/L16;codec=pcm;rate=24000");
        assert!(!image.bytes.is_empty());
        assert!(!audio.bytes.is_empty());
    }

    #[test]
    fn fixture_audio_binary_response_supports_wav() {
        let mut request = make_request(EndpointKind::AudioSpeech);
        request.raw_body = json!({
            "response_format": "wav",
        });
        let (bytes, content_type) =
            build_fixture_audio_binary_response(&request).expect("fixture wav audio");
        assert_eq!(content_type, "audio/wav");
        assert!(bytes.len() >= 44);
    }

    #[test]
    fn build_embeddings_request_uses_batch_for_multiple_inputs() {
        let mut request = make_request(EndpointKind::Embeddings);
        request.raw_body = json!({
            "input": ["alpha", "beta"],
            "dimensions": 32,
            "task_type": "RETRIEVAL_DOCUMENT"
        });

        let built = build_embeddings_request(
            &request,
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-embedding-001",
        )
        .expect("embeddings request");
        assert!(built
            .url
            .ends_with("/models/gemini-embedding-001:batchEmbedContents"));
        assert_eq!(built.body["requests"].as_array().unwrap().len(), 2);
        assert_eq!(built.body["requests"][0]["outputDimensionality"], json!(32));
        assert_eq!(
            built.body["requests"][0]["taskType"],
            json!("RETRIEVAL_DOCUMENT")
        );
    }

    #[test]
    fn bridge_embeddings_response_maps_single_response_to_openai_shape() {
        let mut request = make_request(EndpointKind::Embeddings);
        request.raw_body = json!({
            "input": "alpha",
            "encoding_format": "float"
        });
        let bridged = bridge_embeddings_response(
            &request,
            "gemini-embedding-001",
            &json!({
                "embedding": {
                    "values": [0.5, -0.25, 1.0]
                },
                "usageMetadata": {
                    "promptTokenCount": 7,
                    "totalTokenCount": 7
                }
            }),
        )
        .expect("bridge");
        assert_eq!(bridged["object"], json!("list"));
        assert_eq!(bridged["data"][0]["object"], json!("embedding"));
        assert_eq!(bridged["usage"]["prompt_tokens"], json!(7));
    }

    #[test]
    fn fixture_embeddings_support_base64_encoding() {
        let mut request = make_request(EndpointKind::Embeddings);
        request.raw_body = json!({
            "input": "alpha",
            "encoding_format": "base64"
        });
        let response =
            build_fixture_embeddings_response(&request, "gemini-embedding-001").expect("fixture");
        assert_eq!(response["object"], json!("list"));
        assert!(response["data"][0]["embedding"].as_str().is_some());
    }

    #[test]
    fn fixture_canonical_response_emits_required_tool_call() {
        let mut request = make_request(EndpointKind::ChatCompletions);
        request.messages = vec![
            CanonicalMessage {
                role: MessageRole::System,
                content: vec![ContentPart::Text {
                    text: "You must call exactly one tool.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
            CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Use only the weather tool for Hangzhou.".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            },
        ];
        request.tools = vec![make_weather_tool()];
        request.tool_choice = Some(json!("required"));

        let response =
            build_fixture_canonical_response(&request, "aistudio-web-reverse-fixture").unwrap();
        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name.as_deref(), Some("weather"));
        assert_eq!(
            response.tool_calls[0].arguments.as_deref(),
            Some("{\"city\":\"Hangzhou\"}")
        );
    }

    #[test]
    fn fixture_canonical_response_skips_roundtrip_history() {
        let mut request = make_request(EndpointKind::ChatCompletions);
        request.tools = vec![make_weather_tool()];
        request.tool_choice = Some(json!("required"));
        request.messages.push(CanonicalMessage {
            role: MessageRole::Assistant,
            content: vec![],
            name: None,
            tool_call_id: None,
            tool_calls: vec![CanonicalToolCall {
                id: Some("call_weather".to_string()),
                call_type: "function".to_string(),
                name: Some("weather".to_string()),
                arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                raw: HashMap::new(),
            }],
        });

        assert!(build_fixture_canonical_response(&request, "fixture").is_none());
    }
}
