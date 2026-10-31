use super::*;
use crate::protocol::gemini_business;

fn request(body: Value) -> CanonicalRelayRequest {
    gemini_business::normalize_image_generations(body).unwrap()
}

fn candidate(mime: &str, data: &str) -> Value {
    json!({"candidates": [{"finishReason": "STOP", "content": {"parts": [
        {"inlineData": {"mimeType": mime, "data": data}}
    ]}}]})
}

#[test]
fn image_plan_preserves_model_prompt_and_requested_shape() {
    let model = gemini_canvas::GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL;
    let req = request(json!({"model": model, "prompt": "one red square", "n": 1, "size": "16:9"}));
    let body = image_request(&req, model).unwrap();
    assert_eq!(body["contents"][0]["parts"][0]["text"], "one red square");
    assert_eq!(
        body["generationConfig"]["responseModalities"],
        json!(["IMAGE"])
    );
    assert_eq!(
        body["generationConfig"]["imageConfig"]["aspectRatio"],
        "16:9"
    );
    assert!(body.get("apiKey").is_none());
    assert!(body.get("model").is_none());
}

#[test]
fn image_edits_preserve_inline_bytes_without_a_web_upload_or_jpeg_reencode() {
    let req = gemini_business::normalize_image_edits(json!({"prompt": "make it blue", "images": [
        {"mime_type": "image/png", "base64": "aGVsbG8="}
    ]}))
    .unwrap();
    let body = image_request(&req, gemini_canvas::GEMINI_25_FLASH_IMAGE_MODEL).unwrap();
    assert_eq!(
        body["contents"][0]["parts"][0]["inlineData"],
        json!({"mimeType": "image/png", "data": "aGVsbG8="})
    );
    assert!(body["contents"][0]["parts"][0].get("fileData").is_none());
}

#[test]
fn unsupported_image_contracts_fail_before_generation() {
    let model = gemini_canvas::GEMINI_25_FLASH_IMAGE_MODEL;
    for patch in [
        json!({"n": 2}),
        json!({"n": 0}),
        json!({"n": "1"}),
        json!({"response_format": "jpeg"}),
        json!({"mask": "fixture"}),
    ] {
        let mut body = json!({"prompt": "test"});
        body.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(image_request(&request(body), model).is_err());
    }
    let mut req = request(json!({"prompt": "test"}));
    req.endpoint_kind = EndpointKind::ImagesEdits;
    assert!(image_request(&req, model).is_err());
    assert!(image_request(&req, "unsupported-image-model").is_err());
}

#[test]
fn invalid_inline_edits_and_remote_references_are_not_silently_dropped() {
    for (mime, data) in [("image/png", "%%%"), ("audio/wav", "aGVsbG8=")] {
        let mut req = request(json!({"prompt": "test"}));
        req.endpoint_kind = EndpointKind::ImagesEdits;
        req.messages[0].content.insert(
            0,
            crate::protocol::canonical::ContentPart::ImageUrl {
                image_url: format!("data:{mime};base64,{data}"),
                detail: None,
            },
        );
        assert!(image_request(&req, gemini_canvas::GEMINI_25_FLASH_IMAGE_MODEL).is_err());
    }
    let mut req = request(json!({"prompt": "test"}));
    req.endpoint_kind = EndpointKind::ImagesEdits;
    req.messages[0].content.insert(
        0,
        crate::protocol::canonical::ContentPart::ImageUrl {
            image_url: "https://fixture.test/image.png".to_string(),
            detail: None,
        },
    );
    assert!(image_request(&req, gemini_canvas::GEMINI_25_FLASH_IMAGE_MODEL).is_err());
}

#[test]
fn host_media_requires_complete_nonempty_matching_output_not_acceptance() {
    assert_eq!(
        inline_media(&candidate("image/png", "aGVsbG8="), "image/")
            .unwrap()
            .bytes,
        b"hello"
    );
    for body in [
        json!({"done": false}),
        json!({"candidates": []}),
        candidate("image/png", ""),
        candidate("image/png", "%%%"),
        candidate("audio/wav", "aGVsbG8="),
    ] {
        assert!(inline_media(&body, "image/").is_err());
    }
    let mut body = candidate("image/png", "aGVsbG8=");
    body["candidates"][0]
        .as_object_mut()
        .unwrap()
        .remove("finishReason");
    assert!(inline_media(&body, "image/").is_err());
    body["candidates"][0]["finishReason"] = json!("MAX_TOKENS");
    assert!(inline_media(&body, "image/").is_err());
    body["candidates"][0]["finishReason"] = json!("STOP");
    body["candidates"][0]["content"]["parts"][0]["thought"] = json!(true);
    assert!(inline_media(&body, "image/").is_err());
}

#[test]
fn speech_formats_are_validated_before_send_and_pcm_is_not_compressed_audio() {
    for format in [json!("mp3"), json!("invalid"), json!(null), json!(42)] {
        let req = request(json!({"prompt": "test", "response_format": format}));
        assert!(tts_format(&req).is_err());
    }
    let req = request(json!({"prompt": "test", "response_format": "pcm"}));
    let media = inline_media(&candidate("audio/ogg", "aGVsbG8="), "audio/").unwrap();
    assert!(audio_response(&req, media).is_err());
    let mut req = req;
    req.raw_body["response_format"] = json!("wav");
    let media = inline_media(
        &candidate("audio/pcm;rate=24000;channels=1", "AAECAw=="),
        "audio/",
    )
    .unwrap();
    let response = audio_response(&req, media).unwrap();
    assert_eq!(response.content_type.as_deref(), Some("audio/wav"));
    assert_eq!(&response.body[..4], b"RIFF");
    assert_eq!(&response.body[44..], &[0, 1, 2, 3]);
    let media = inline_media(&candidate("audio/pcm;rate=4294967295", "AAE="), "audio/").unwrap();
    assert!(audio_response(&req, media).is_err());
    for (mime, data) in [
        ("audio/pcm;rate=24000;channels=1", "AA=="),
        ("audio/pcm;rate=24000;channels=2", "AAE="),
        ("audio/wav", "aGVsbG8="),
        ("audio/wavjunk", "aGVsbG8="),
    ] {
        let media = inline_media(&candidate(mime, data), "audio/").unwrap();
        assert!(
            audio_response(&req, media).is_err(),
            "{mime} must not become a successful WAV"
        );
    }
    req.raw_body["response_format"] = json!("opus");
    for mime in ["audio/ogg", "audio/opus"] {
        let media = inline_media(&candidate(mime, "aGVsbG8="), "audio/").unwrap();
        assert!(
            audio_response(&req, media).is_err(),
            "{mime} must not be relabeled as valid Ogg Opus"
        );
    }
}

#[test]
fn model_identifier_cannot_inject_a_path_or_api_key_query() {
    for model in [
        "",
        "model/key",
        "model?key=fixture",
        " model",
        "..",
        "model#fragment",
    ] {
        assert!(validate_model_path(model).is_err());
    }
    assert!(validate_model_path("gemini-3-flash-preview").is_ok());
}

#[test]
fn host_failure_keeps_diagnosis_but_prohibits_automatic_retry_or_account_fallback() {
    for error in [
        GatewayError::server_error("navigation failed")
            .with_code("gemini_canvas_browser_worker_failed"),
        GatewayError::service_unavailable("host missing"),
        GatewayError::rate_limited("host busy", 2000),
    ] {
        let original = error.clone();
        let error = without_host_replay(error);
        assert_eq!(error.message, original.message);
        assert_eq!(error.http_status, original.http_status);
        assert_eq!(error.code, original.code);
        assert!(!crate::retry::should_retry(
            &error,
            &crate::retry::RetryPolicy::default()
        ));
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
    }
    let error = inline_media(&json!({"done":false}), "image/")
        .err()
        .unwrap();
    assert!(!error.retryable);
}

#[tokio::test]
async fn canvas_music_and_video_never_fall_back_to_web_composer_or_remote_executor() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"gemini_canvas_web_reverse_compatible",
        "base_url":"https://gemini.google.com", "api_key":"unused-fixture"
    }))
    .unwrap();
    let mut client = UpstreamClient::new(1);
    client.request_time_browser_policy = RequestTimeBrowserPolicy::Disabled;
    for (endpoint, model) in [
        (
            EndpointKind::MusicGenerations,
            gemini_canvas::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL,
        ),
        (
            EndpointKind::VideosGenerations,
            gemini_canvas::GEMINI_CANVAS_VIDEO_PREVIEW_MODEL,
        ),
    ] {
        let mut req = request(json!({"prompt":"a blue circle", "n":1}));
        req.endpoint_kind = endpoint;
        let error = client
            .execute_gemini_canvas_modular_browser_relay_media(
                "fixture", &payload, &req, model, None,
            )
            .await
            .unwrap_err();
        assert_eq!(error.http_status, Some(503));
        assert!(
            !error.retryable,
            "unavailable Canvas media must not be replayed"
        );
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_host_media_contract_unavailable")
        );
    }
}

#[test]
fn google_canvas_pcm_samples_are_not_byte_swapped_by_the_standard_l16_encoder() {
    let mut req = request(json!({"prompt":"test", "response_format":"wav"}));
    for mime in [
        "audio/L16;codec=pcm;rate=24000",
        " audio/L16 ; CODEC=PCM ; RATE=24000 ; CHANNELS=1 ",
    ] {
        let media = InlineMedia {
            mime_type: mime.to_string(),
            bytes: vec![0x12, 0x34, 0x56, 0x78],
        };
        let response = audio_response(&req, media).unwrap();
        assert_eq!(&response.body[44..], &[0x12, 0x34, 0x56, 0x78]);
    }
    req.raw_body["response_format"] = json!("pcm");
    let media = InlineMedia {
        mime_type: "audio/L16;codec=pcm;rate=24000".to_string(),
        bytes: vec![0x12, 0x34],
    };
    let response = audio_response(&req, media).unwrap();
    assert_eq!(&response.body[..], &[0x12, 0x34]);
    req.raw_body["response_format"] = json!("wav");
    let media = InlineMedia {
        mime_type: "audio/L16;rate=24000".to_string(),
        bytes: vec![0x12, 0x34],
    };
    assert_eq!(
        &audio_response(&req, media).unwrap().body[44..],
        &[0x34, 0x12]
    );
}
