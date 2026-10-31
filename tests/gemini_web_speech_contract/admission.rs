//! Result admission negatives operate only on synthetic loopback responses.
use super::*;

#[tokio::test]
async fn native_ogg_opus_and_default_format_roundtrip_without_transcoding() {
    for format in [None, Some("ogg"), Some("opus")] {
        let audio = ogg();
        let server = fixture(frame("XqA3Ic", &audio)).await;
        let mut req = request();
        req.raw_body
            .as_object_mut()
            .unwrap()
            .remove("response_format");
        req.raw_body["voice"] = json!("default");
        req.raw_body["speed"] = json!(1.0);
        if let Some(format) = format {
            req.raw_body["response_format"] = json!(format);
        }
        let response = UpstreamClient::new(1)
            .execute_binary_passthrough(&payload(&server.base), &req, "gemini-web-chat", None)
            .await
            .unwrap();
        assert_eq!(response.body.as_ref(), audio);
        assert_eq!(response.content_type.as_deref(), Some("audio/ogg"));
        assert_eq!(server.capture.lock().unwrap().calls.len(), 1);
    }
}

#[tokio::test]
async fn strict_rpc_envelope_rejects_duplicates_wrong_lengths_and_asset_urls() {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.encode(wav());
    let row = json!([
        "wrb.fr",
        "XqA3Ic",
        json!([bytes]).to_string(),
        null,
        null,
        null,
        "generic"
    ]);
    let good = frame("XqA3Ic", &wav());
    for reply in [
        json!([row.clone(), row.clone()]).to_string(),
        good.replacen("\n", "\n999999", 1),
        good[..good.len() - 3].to_string(),
        format!("{good}private-text-{bytes}"),
        json!([["wrb.fr", "XqA3Ic", "[\"http://127.0.0.1:1/private-audio\"]", null, null, null, "generic"]]).to_string(),
        json!([["wrb.fr", "XqA3Ic", json!([bytes]).to_string(), null, null, {"error":"private"}, "generic"]]).to_string(),
    ] {
        let server = fixture(reply).await;
        let error = UpstreamClient::new(1).execute_binary_passthrough(&payload(&server.base), &request(), "gemini-web-chat", None).await.unwrap_err();
        assert_eq!(error.code.as_deref(), Some("invalid_gemini_web_speech_response"));
        assert!(!error.message.contains("private-text-"));
        assert!(!error.message.contains("private-audio"));
        assert!(!error.retryable);
        let captured = server.capture.lock().unwrap();
        assert_eq!(captured.gets, 1, "no result URL may be downloaded");
        assert_eq!(captured.calls.len(), 1);
    }
}

#[tokio::test]
async fn audio_admission_rejects_truncated_or_corrupted_containers() {
    let mut wrong_wav_size = wav();
    wrong_wav_size[4] = 1;
    let mut wrong_wav_alignment = wav();
    wrong_wav_alignment[32] = 1;
    let mut wrong_ogg_crc = ogg();
    *wrong_ogg_crc.last_mut().unwrap() ^= 1;
    let mut incomplete_ogg = ogg();
    incomplete_ogg.truncate(incomplete_ogg.len() - 1);
    for (bytes, format) in [
        (wrong_wav_size, "wav"),
        (wrong_wav_alignment, "wav"),
        (wrong_ogg_crc, "ogg"),
        (incomplete_ogg, "opus"),
    ] {
        let server = fixture(frame("XqA3Ic", &bytes)).await;
        let mut req = request();
        req.raw_body["response_format"] = json!(format);
        let error = UpstreamClient::new(1)
            .execute_binary_passthrough(&payload(&server.base), &req, "gemini-web-chat", None)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("invalid_gemini_web_speech_response")
        );
        assert_eq!(server.capture.lock().unwrap().calls.len(), 1);
    }
}

#[tokio::test]
async fn unsupported_models_large_input_and_invalid_call_flow_never_bootstrap() {
    let server = fixture(frame("XqA3Ic", &wav())).await;
    for (key, value, model) in [
        ("input", json!("x".repeat(16 * 1024 + 1)), "gemini-web-chat"),
        ("input", json!(""), "gemini-web-chat"),
        ("input", json!("hello"), "gemini-2.5-flash-preview-tts"),
        ("language", json!("fr-FR"), "gemini-web-chat"),
        ("response_format", json!("mp3"), "gemini-web-chat"),
    ] {
        let mut req = request();
        req.raw_body[key] = value;
        let error = UpstreamClient::new(1)
            .execute_binary_passthrough(&payload(&server.base), &req, model, None)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_web_speech_request")
        );
    }
    let mut invalid = payload(&server.base);
    invalid.call_flow.as_mut().unwrap().special_adapter =
        neuro_gateway::protocol::call_flow::SpecialAdapter::GeminiCanvas;
    let error = UpstreamClient::new(1)
        .execute_binary_passthrough(&invalid, &request(), "gemini-web-chat", None)
        .await
        .unwrap_err();
    assert!(error.message.starts_with("Invalid call_flow:"));
    assert_eq!(server.capture.lock().unwrap().gets, 0);
}
