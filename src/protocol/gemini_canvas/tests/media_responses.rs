use super::*;

#[test]
fn build_openai_images_response_can_emit_direct_urls() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::ImagesGenerations,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "prompt": "banana",
            "response_format": "url"
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let response = build_openai_images_response_from_urls(
        &req,
        "banana",
        &[GeminiCanvasMediaAsset {
            kind: "image".to_string(),
            url: "https://lh3.googleusercontent.com/gg-dl/abc".to_string(),
            mime_type: "image/png".to_string(),
            download_token: None,
            body_base64: None,
            alt: Some("AI 生成".to_string()),
            width: Some(1024),
            height: Some(1024),
            duration_seconds: None,
        }],
    )
    .unwrap();

    assert_eq!(
        response["data"][0]["url"].as_str(),
        Some("https://lh3.googleusercontent.com/gg-dl/abc")
    );
}

#[test]
fn build_music_generation_response_uses_music_object_shape() {
    let response = build_music_generation_response(
        GEMINI_CANVAS_MUSIC_PREVIEW_MODEL,
        "warm lo-fi piano with rain",
        &GeminiCanvasMediaAsset {
            kind: "video".to_string(),
            url: "https://contribution.usercontent.google.com/download?filename=ivory_rain.mp4"
                .to_string(),
            mime_type: "video/mp4".to_string(),
            download_token: None,
            body_base64: None,
            alt: None,
            width: Some(0),
            height: Some(0),
            duration_seconds: Some(10.0),
        },
        Some("音乐已经准备就绪"),
    );

    assert_eq!(response["object"], "music.generation");
    assert_eq!(response["provider"], "gemini_canvas");
    assert_eq!(response["data"][0]["mime_type"], "video/mp4");
}

#[test]
fn video_body_indicates_music_modality_mismatch_detects_music_markers() {
    let body = "I have created your original live ambient piano track. http://googleusercontent.com/generated_music_content/0 gemini.google.com/music";
    assert!(video_body_indicates_music_modality_mismatch(body));
}

#[test]
fn video_body_indicates_music_modality_mismatch_ignores_regular_video_markers() {
    let body =
            "video.generation completed with generated_video_content/0 and contribution.usercontent.google.com/download?filename=video.mp4";
    assert!(!video_body_indicates_music_modality_mismatch(body));
}

#[test]
fn build_tts_request_body_sets_audio_modality_and_voice() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioSpeech,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text {
                text: "say hello".to_string(),
            }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "input": "say hello",
            "voice": "Kore",
            "response_format": "wav",
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let body = build_tts_request_body(&req, GEMINI_CANVAS_DEFAULT_TTS_MODEL);
    assert_eq!(
        body["generationConfig"]["responseModalities"],
        json!(["AUDIO"])
    );
    assert_eq!(
        body["generationConfig"]["speechConfig"]["voiceConfig"]["prebuiltVoiceConfig"]["voiceName"],
        "Kore"
    );
    assert!(body.get("model").is_none());
}

#[test]
fn extract_inline_image_from_generate_content_response_reads_inline_data() {
    let image = extract_inline_image_from_generate_content_response(&json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "inlineData": {
                        "mimeType": "image/png",
                        "data": base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3, 4]),
                    }
                }]
            }
        }]
    }))
    .unwrap();

    assert_eq!(image.mime_type, "image/png");
    assert_eq!(image.bytes, vec![1, 2, 3, 4]);
}

#[test]
fn extract_images_from_imagen_predict_response_reads_generated_images_shape() {
    let response = json!({
        "generatedImages": [{
            "image": {
                "mimeType": "image/png",
                "imageBytes": base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3, 4]),
            }
        }]
    });

    let images = extract_images_from_imagen_predict_response(&response).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime_type, "image/png");
    assert_eq!(images[0].bytes, vec![1, 2, 3, 4]);
}

#[test]
fn extract_images_from_imagen_predict_response_reads_predictions_shape() {
    let response = json!({
        "predictions": [{
            "mimeType": "image/jpeg",
            "bytesBase64Encoded": base64::engine::general_purpose::STANDARD.encode([9u8, 8, 7]),
        }]
    });

    let images = extract_images_from_imagen_predict_response(&response).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime_type, "image/jpeg");
    assert_eq!(images[0].bytes, vec![9, 8, 7]);
}

#[test]
fn extract_audio_from_generate_content_response_reads_inline_data() {
    let audio = extract_audio_from_generate_content_response(&json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "inlineData": {
                            "mimeType": "audio/L16;codec=pcm;rate=24000",
                            "data": base64::engine::general_purpose::STANDARD.encode([0u8, 1u8, 2u8, 3u8]),
                        }
                    }]
                }
            }]
        }))
        .unwrap();

    assert_eq!(audio.mime_type, "audio/L16;codec=pcm;rate=24000");
    assert_eq!(audio.bytes, vec![0, 1, 2, 3]);
}

#[test]
fn build_audio_binary_response_wraps_pcm_in_wav_by_default() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioSpeech,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "input": "say hello",
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let (body, content_type) = build_audio_binary_response(
        &req,
        &GeminiCanvasAudio {
            mime_type: "audio/L16;codec=pcm;rate=24000".to_string(),
            bytes: vec![0x12, 0x34, 0x56, 0x78],
        },
    )
    .unwrap();

    assert_eq!(content_type, "audio/wav");
    assert!(body.starts_with(b"RIFF"));
    assert!(body.windows(4).any(|chunk| chunk == b"WAVE"));
}

#[test]
fn build_audio_binary_response_accepts_opus_payloads() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioSpeech,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "input": "say hello",
            "response_format": "opus",
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let (body, content_type) = build_audio_binary_response(
        &req,
        &GeminiCanvasAudio {
            mime_type: "audio/ogg".to_string(),
            bytes: vec![0x4f, 0x67, 0x67, 0x53],
        },
    )
    .unwrap();

    assert_eq!(body, vec![0x4f, 0x67, 0x67, 0x53]);
    assert_eq!(content_type, "audio/ogg");
}

#[test]
fn build_audio_binary_response_rejects_wav_when_only_ogg_is_available() {
    let req = CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind: EndpointKind::AudioSpeech,
        requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
        stream: false,
        messages: vec![],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: json!({
            "input": "say hello",
            "response_format": "wav",
        }),
        previous_response_id: None,
        explicit_session_key: None,
        extra: HashMap::new(),
    };

    let error = build_audio_binary_response(
        &req,
        &GeminiCanvasAudio {
            mime_type: "audio/ogg".to_string(),
            bytes: vec![0x4f, 0x67, 0x67, 0x53],
        },
    )
    .unwrap_err();

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_tts_wav_unavailable")
    );
}
