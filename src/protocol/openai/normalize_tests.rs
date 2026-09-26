use crate::protocol::canonical::{ContentPart, EndpointKind, MessageRole, ProtocolFamily};
use crate::protocol::openai::{
    normalize_audio_speech, normalize_audio_transcriptions, normalize_chat_completions,
    normalize_embeddings, normalize_legacy_completions,
};
use serde_json::json;

#[test]
fn normalization_errors_preserve_status_and_exact_messages() {
    let cases = [
        (
            normalize_chat_completions(json!({})).unwrap_err(),
            "missing or invalid `messages` array",
        ),
        (
            normalize_chat_completions(json!({"messages": "invalid"})).unwrap_err(),
            "missing or invalid `messages` array",
        ),
        (
            normalize_chat_completions(json!({"messages": [{"content": "missing role"}]}))
                .unwrap_err(),
            "message missing `role` field",
        ),
        (
            normalize_chat_completions(json!({
                "messages": [{"role": "developer", "content": "unsupported"}]
            }))
            .unwrap_err(),
            "unknown message role: developer",
        ),
        (
            normalize_legacy_completions(json!({})).unwrap_err(),
            "missing `prompt` field",
        ),
        (
            normalize_embeddings(json!({})).unwrap_err(),
            "missing `input` field",
        ),
        (
            normalize_audio_speech(json!({})).unwrap_err(),
            "missing `input` field",
        ),
        (
            normalize_audio_transcriptions(json!({"file": "invalid"})).unwrap_err(),
            "missing `file` field",
        ),
    ];

    for (error, expected_message) in cases {
        assert_eq!(error.http_status, Some(400));
        assert_eq!(error.message, expected_message);
    }
}

#[test]
fn chat_preserves_content_shapes_and_message_tool_fields() {
    let request = normalize_chat_completions(json!({
        "messages": [
            {"role": "system"},
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "plain"},
                    {"type": "text", "text": "annotated", "cache_control": {"type": "ephemeral"}},
                    {"type": "image_url", "image_url": {"url": "https://example.test/a.png", "detail": "high"}},
                    {"type": "input_audio", "input_audio": {"data": "AA=="}},
                    17
                ]
            },
            {
                "role": "assistant",
                "name": "agent",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {"name": "lookup", "arguments": {"q": "rust"}}
                }]
            },
            {"role": "function", "tool_call_id": "call_1", "content": {"ok": true}}
        ]
    }))
    .unwrap();

    assert!(request.messages[0].content.is_empty());
    assert!(matches!(
        &request.messages[1].content[0],
        ContentPart::Text { text } if text == "plain"
    ));
    assert!(matches!(
        &request.messages[1].content[1],
        ContentPart::Raw { value } if value["cache_control"]["type"] == "ephemeral"
    ));
    assert!(matches!(
        &request.messages[1].content[2],
        ContentPart::ImageUrl { image_url, detail }
            if image_url == "https://example.test/a.png" && detail.as_deref() == Some("high")
    ));
    assert!(matches!(
        &request.messages[1].content[3],
        ContentPart::Raw { value } if value["type"] == "input_audio"
    ));
    assert!(matches!(
        &request.messages[1].content[4],
        ContentPart::Raw { value } if value == &json!(17)
    ));
    assert_eq!(request.messages[2].name.as_deref(), Some("agent"));
    assert!(request.messages[2].content.is_empty());
    assert_eq!(
        request.messages[2].tool_calls[0].id.as_deref(),
        Some("call_1")
    );
    assert_eq!(
        request.messages[2].tool_calls[0].name.as_deref(),
        Some("lookup")
    );
    assert_eq!(
        request.messages[2].tool_calls[0].arguments.as_deref(),
        Some("{\"q\":\"rust\"}")
    );
    assert_eq!(request.messages[3].role, MessageRole::Tool);
    assert_eq!(request.messages[3].tool_call_id.as_deref(), Some("call_1"));
    assert!(matches!(
        &request.messages[3].content[0],
        ContentPart::Raw { value } if value == &json!({"ok": true})
    ));
}

#[test]
fn chat_preserves_tools_reasoning_session_extra_and_raw_body() {
    let body = json!({
        "model": "gpt-test",
        "messages": [],
        "stream": true,
        "tools": [{
            "type": "function",
            "function": {
                "name": "lookup",
                "description": "Lookup a record",
                "parameters": {"type": "object", "required": ["id"]}
            },
            "cache_control": {"type": "ephemeral"}
        }],
        "tool_choice": {"type": "function", "function": {"name": "lookup"}},
        "reasoning": {"effort": "high"},
        "user": "session-42",
        "vendor_extension": {"enabled": true}
    });
    let expected_raw = body.clone();
    let request = normalize_chat_completions(body).unwrap();

    assert_eq!(request.protocol_family, ProtocolFamily::OpenAi);
    assert_eq!(request.endpoint_kind, EndpointKind::ChatCompletions);
    assert!(request.stream);
    assert_eq!(request.explicit_session_key.as_deref(), Some("session-42"));
    assert_eq!(request.reasoning, Some(json!({"effort": "high"})));
    assert_eq!(
        request.tool_choice,
        Some(json!({"type": "function", "function": {"name": "lookup"}}))
    );
    assert_eq!(request.tools[0].name.as_deref(), Some("lookup"));
    assert_eq!(
        request.tools[0].input_schema,
        Some(json!({"type": "object", "required": ["id"]}))
    );
    assert_eq!(
        request.tools[0].raw.get("cache_control"),
        Some(&json!({"type": "ephemeral"}))
    );
    assert_eq!(
        request.extra.get("vendor_extension"),
        Some(&json!({"enabled": true}))
    );
    assert_eq!(request.extra.len(), 1);
    assert_eq!(request.raw_body, expected_raw);
}

#[test]
fn legacy_prompt_preserves_array_and_raw_values() {
    let body = json!({
        "model": "legacy-model",
        "prompt": ["alpha", {"token": 7}, false],
        "stream": true,
        "user": "legacy-session",
        "vendor_extension": "raw-only"
    });
    let expected_raw = body.clone();
    let request = normalize_legacy_completions(body).unwrap();

    assert_eq!(request.protocol_family, ProtocolFamily::OpenAi);
    assert_eq!(request.endpoint_kind, EndpointKind::Completions);
    assert!(request.stream);
    assert_eq!(
        request.explicit_session_key.as_deref(),
        Some("legacy-session")
    );
    assert_eq!(request.messages.len(), 3);
    assert_eq!(request.messages[0].text_content(), "alpha");
    assert!(matches!(
        &request.messages[1].content[0],
        ContentPart::Raw { value } if value == &json!({"token": 7})
    ));
    assert!(matches!(
        &request.messages[2].content[0],
        ContentPart::Raw { value } if value == &json!(false)
    ));
    assert!(request.extra.is_empty());
    assert_eq!(request.raw_body, expected_raw);

    let raw_request = normalize_legacy_completions(json!({"prompt": {"nested": true}})).unwrap();
    assert!(matches!(
        &raw_request.messages[0].content[0],
        ContentPart::Raw { value } if value == &json!({"nested": true})
    ));
}

#[test]
fn embeddings_and_speech_preserve_endpoint_session_and_raw_body() {
    let embeddings_body = json!({
        "model": "embed-model",
        "input": {"tokens": [1, 2, 3]},
        "stream": true,
        "user": "embed-session",
        "encoding_format": "float"
    });
    let expected_embeddings_raw = embeddings_body.clone();
    let embeddings = normalize_embeddings(embeddings_body).unwrap();
    assert_eq!(embeddings.protocol_family, ProtocolFamily::OpenAi);
    assert_eq!(embeddings.endpoint_kind, EndpointKind::Embeddings);
    assert!(!embeddings.stream);
    assert_eq!(
        embeddings.explicit_session_key.as_deref(),
        Some("embed-session")
    );
    assert!(embeddings.extra.is_empty());
    assert_eq!(embeddings.raw_body, expected_embeddings_raw);
    assert!(matches!(
        &embeddings.messages[0].content[0],
        ContentPart::Raw { value } if value == &json!({"tokens": [1, 2, 3]})
    ));

    let speech_body = json!({
        "model": "speech-model",
        "input": ["hello", 9],
        "stream": true,
        "user": "speech-session",
        "voice": "alloy"
    });
    let expected_speech_raw = speech_body.clone();
    let speech = normalize_audio_speech(speech_body).unwrap();
    assert_eq!(speech.endpoint_kind, EndpointKind::AudioSpeech);
    assert!(!speech.stream);
    assert_eq!(
        speech.explicit_session_key.as_deref(),
        Some("speech-session")
    );
    assert_eq!(speech.messages.len(), 2);
    assert_eq!(speech.messages[0].text_content(), "hello");
    assert!(matches!(
        &speech.messages[1].content[0],
        ContentPart::Raw { value } if value == &json!(9)
    ));
    assert!(speech.extra.is_empty());
    assert_eq!(speech.raw_body, expected_speech_raw);
}

#[test]
fn audio_transcriptions_preserve_prompt_filename_and_default_fallbacks() {
    let prompt_body = json!({
        "model": "whisper-test",
        "prompt": "  medical dictation  ",
        "file": {"file_name": " ignored.wav ", "base64": "AA=="},
        "user": "audio-session",
        "language": "en"
    });
    let expected_prompt_raw = prompt_body.clone();
    let prompt_request = normalize_audio_transcriptions(prompt_body).unwrap();
    assert_eq!(prompt_request.protocol_family, ProtocolFamily::OpenAi);
    assert_eq!(
        prompt_request.endpoint_kind,
        EndpointKind::AudioTranscriptions
    );
    assert!(!prompt_request.stream);
    assert_eq!(
        prompt_request.messages[0].text_content(),
        "medical dictation"
    );
    assert_eq!(
        prompt_request.explicit_session_key.as_deref(),
        Some("audio-session")
    );
    assert!(prompt_request.extra.is_empty());
    assert_eq!(prompt_request.raw_body, expected_prompt_raw);

    let filename_request = normalize_audio_transcriptions(json!({
        "prompt": "   ",
        "file": {"file_name": "  note.wav  "}
    }))
    .unwrap();
    assert_eq!(
        filename_request.messages[0].text_content(),
        "transcribe note.wav"
    );

    let default_request = normalize_audio_transcriptions(json!({
        "file": {"file_name": "   "}
    }))
    .unwrap();
    assert_eq!(
        default_request.messages[0].text_content(),
        "audio transcription request"
    );
}

#[test]
fn normalize_basic_request() {
    let body = json!({
        "model": "gpt-4o",
        "messages": [
            {"role": "system", "content": "Be helpful."},
            {"role": "user", "content": "Hello!"}
        ],
        "stream": false,
    });
    let req = normalize_chat_completions(body).unwrap();
    assert_eq!(req.requested_model.as_deref(), Some("gpt-4o"));
    assert_eq!(req.messages.len(), 2);
    assert!(!req.stream);
    assert_eq!(req.messages[0].role, MessageRole::System);
    assert_eq!(req.messages[1].role, MessageRole::User);
}

#[test]
fn normalize_array_content_parts() {
    let body = json!({
        "model": "gpt-4o",
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Look at this image:"},
                {"type": "image_url", "image_url": {"url": "https://example.com/img.png"}}
            ]
        }]
    });
    let req = normalize_chat_completions(body).unwrap();
    assert_eq!(req.messages[0].content.len(), 2);
    assert!(matches!(
        req.messages[0].content[0],
        ContentPart::Text { .. }
    ));
    assert!(matches!(
        req.messages[0].content[1],
        ContentPart::ImageUrl { .. }
    ));
}

#[test]
fn normalize_captures_extra_fields() {
    let body = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "hi"}],
        "temperature": 0.7,
        "max_tokens": 1000,
        "top_p": 0.9,
    });
    let req = normalize_chat_completions(body).unwrap();
    assert_eq!(req.extra.get("temperature"), Some(&json!(0.7)));
    assert_eq!(req.extra.get("max_tokens"), Some(&json!(1000)));
    assert_eq!(req.extra.get("top_p"), Some(&json!(0.9)));
}

#[test]
fn normalize_missing_messages_returns_error() {
    let body = json!({"model": "gpt-4o"});
    assert!(normalize_chat_completions(body).is_err());
}

#[test]
fn normalize_legacy_completions_maps_prompt_to_messages() {
    let req = normalize_legacy_completions(json!({
        "model": "gpt-3.5-turbo-instruct",
        "prompt": "finish this sentence",
        "stream": true,
        "user": "user-123"
    }))
    .unwrap();
    assert_eq!(req.endpoint_kind, EndpointKind::Completions);
    assert!(req.stream);
    assert_eq!(
        req.requested_model.as_deref(),
        Some("gpt-3.5-turbo-instruct")
    );
    assert_eq!(req.messages[0].text_content(), "finish this sentence");
    assert_eq!(req.explicit_session_key.as_deref(), Some("user-123"));
}

#[test]
fn normalize_embeddings_maps_input_to_messages() {
    let req = normalize_embeddings(json!({
        "model": "text-embedding-3-large",
        "input": ["alpha", "beta"],
        "user": "embed-user"
    }))
    .unwrap();
    assert_eq!(req.endpoint_kind, EndpointKind::Embeddings);
    assert_eq!(
        req.requested_model.as_deref(),
        Some("text-embedding-3-large")
    );
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].text_content(), "alpha");
    assert_eq!(req.messages[1].text_content(), "beta");
    assert_eq!(req.explicit_session_key.as_deref(), Some("embed-user"));
}

#[test]
fn normalize_audio_speech_maps_input_to_messages() {
    let req = normalize_audio_speech(json!({
        "model": "tts-1",
        "input": "say hello"
    }))
    .unwrap();
    assert_eq!(req.endpoint_kind, EndpointKind::AudioSpeech);
    assert_eq!(req.requested_model.as_deref(), Some("tts-1"));
    assert_eq!(req.messages[0].text_content(), "say hello");
}

#[test]
fn normalize_audio_transcriptions_uses_prompt_or_filename() {
    let req = normalize_audio_transcriptions(json!({
        "model": "whisper-1",
        "prompt": "medical dictation",
        "file": {
            "file_name": "note.wav",
            "mime_type": "audio/wav",
            "base64": "ZmFrZQ=="
        }
    }))
    .unwrap();
    assert_eq!(req.endpoint_kind, EndpointKind::AudioTranscriptions);
    assert_eq!(req.requested_model.as_deref(), Some("whisper-1"));
    assert_eq!(req.messages[0].text_content(), "medical dictation");
}
