use super::*;

#[test]
fn decode_inline_audio_payload_uses_default_mime_and_decodes_bytes() {
    let result = GeminiCanvasBrowserOwnedInvocationResult {
        operation: "tts".to_string(),
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: None,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        stable_program_pair: None,
        latest_response_pair: None,
        aggregate_hints: None,
        captured_at: None,
        last_validated_at: None,
        new_chat_clicked: None,
        mode_selected: None,
        body_text: None,
        body_base64: Some(base64::engine::general_purpose::STANDARD.encode(b"abc")),
        mime_type: None,
        text: None,
        media: Vec::new(),
    };
    let result: crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult =
        result.into();
    let audio = decode_inline_audio_payload(
        &result,
        GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER,
        "missing audio",
        "missing_audio",
        "invalid audio",
        "invalid_audio",
        "audio/wav",
    )
    .expect("audio payload");
    assert_eq!(audio.mime_type, "audio/wav");
    assert_eq!(audio.bytes, b"abc");
}

#[test]
fn decode_modular_tts_audio_payload_reports_standard_contracts() {
    let result = GeminiCanvasBrowserOwnedInvocationResult {
        operation: "tts".to_string(),
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: None,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        stable_program_pair: None,
        latest_response_pair: None,
        aggregate_hints: None,
        captured_at: None,
        last_validated_at: None,
        new_chat_clicked: None,
        mode_selected: None,
        body_text: None,
        body_base64: Some("not-base64".to_string()),
        mime_type: None,
        text: None,
        media: Vec::new(),
    };
    let result: crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult =
        result.into();

    let error =
        decode_modular_tts_audio_payload(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
            .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_modular_invalid_tts_audio_payload")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas modular browser relay returned invalid inline audio bytes: Invalid symbol 45, offset 3."
    );
}

#[test]
fn decode_browser_tts_audio_payload_reports_standard_contracts() {
    let result = GeminiCanvasBrowserOwnedInvocationResult {
        operation: "tts".to_string(),
        share_url: None,
        share_id: None,
        share_follow_kind: None,
        before_url: None,
        final_url: None,
        page_url: None,
        canvas_program_url: None,
        app_path: None,
        conversation_id: None,
        response_id: None,
        last_seen_conversation_id: None,
        last_seen_response_id: None,
        candidate_pairs: Vec::new(),
        stable_program_pair: None,
        latest_response_pair: None,
        aggregate_hints: None,
        captured_at: None,
        last_validated_at: None,
        new_chat_clicked: None,
        mode_selected: None,
        body_text: None,
        body_base64: Some("not-base64".to_string()),
        mime_type: None,
        text: None,
        media: Vec::new(),
    };
    let result: crate::upstream::gemini::canvas_program_web_reverse::GeminiCanvasBrowserInvocationResult =
        result.into();

    let error =
        decode_browser_tts_audio_payload(&result, GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER)
            .expect_err("invalid base64 should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_invalid_tts_audio_payload")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas browser-backed TTS returned invalid base64 audio bytes: Invalid symbol 45, offset 3."
    );
}
