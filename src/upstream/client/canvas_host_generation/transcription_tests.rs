use super::*;

fn upload() -> Value {
    json!({"model":"gemini-3-flash-preview","file":{
        "file_name":"fixture.mp3", "mime_type":"audio/mpeg", "base64":"SUQzZml4dHVyZQ=="
    }})
}

fn req(body: Value) -> CanonicalRelayRequest {
    openai::normalize_audio_transcriptions(body).unwrap()
}

fn candidate(parts: Value, finish: &str) -> Value {
    json!({"candidates":[{"finishReason":finish,"content":{"parts":parts}}]})
}

#[test]
fn transcription_embeds_exact_uploaded_bytes_not_the_filename_seed() {
    let mut raw = upload();
    raw["language"] = json!("zh");
    raw["prompt"] = json!("Neuro, Gateway");
    let body = request(&req(raw.clone()), "gemini-3-flash-preview").unwrap();
    assert_eq!(
        body["contents"][0]["parts"][0]["inlineData"],
        json!({"mimeType":"audio/mpeg","data":raw["file"]["base64"]})
    );
    let hint = body["contents"][0]["parts"][1]["text"].as_str().unwrap();
    assert!(hint.contains("zh") && hint.contains("Neuro, Gateway"));
    assert!(!hint.contains("fixture.mp3"));
    assert_eq!(
        body["generationConfig"],
        json!({"temperature":0,"responseModalities":["TEXT"]})
    );
    assert!(body.get("apiKey").is_none());
}

#[test]
fn transcription_accepts_multipart_scalars_and_mime_fallback_without_reencoding() {
    for (mime, name, expected) in [
        ("application/octet-stream", "voice.MP3", "audio/mpeg"),
        ("audio/x-wav", "voice.wav", "audio/wav"),
        ("audio/flac", "voice.flac", "audio/flac"),
        ("audio/ogg", "voice.ogg", "audio/ogg"),
        ("audio/webm", "voice.webm", "audio/webm"),
        ("audio/x-m4a", "voice.m4a", "audio/mp4"),
    ] {
        let mut raw = upload();
        raw["file"]["mime_type"] = json!(mime);
        raw["file"]["file_name"] = json!(name);
        raw["temperature"] = json!("0.0");
        raw["stream"] = json!("false");
        raw["response_format"] = json!("text");
        let body = request(&req(raw.clone()), "gemini-3-flash-preview").unwrap();
        assert_eq!(
            body["contents"][0]["parts"][0]["inlineData"]["mimeType"],
            expected
        );
        assert_eq!(
            body["contents"][0]["parts"][0]["inlineData"]["data"],
            raw["file"]["base64"]
        );
    }
}

#[test]
fn transcription_rejects_unimplemented_or_ambiguous_options_before_generation() {
    for (key, value) in [
        ("response_format", json!("srt")),
        ("response_format", json!("vtt")),
        ("response_format", json!("verbose_json")),
        ("response_format", json!(["json", "text"])),
        ("response_format", json!(null)),
        ("response_format", json!(" text ")),
        ("timestamp_granularities", json!(["word"])),
        ("include", json!(["logprobs"])),
        ("chunking_strategy", json!("auto")),
        ("stream", json!(true)),
        ("stream", json!("true")),
        ("temperature", json!(0.5)),
        ("temperature", json!("NaN")),
        ("language", json!(["zh", "en"])),
        ("language", json!("zh-CN")),
        ("model", json!(["a", "b"])),
        ("user", json!({})),
        ("prompt", json!("a".repeat(16385))),
    ] {
        let mut raw = upload();
        raw[key] = value;
        assert!(
            request(&req(raw), "gemini-3-flash-preview").is_err(),
            "{key}"
        );
    }
    for model in ["gemini-2.5-flash-preview-tts", "../model", "unknown", ""] {
        assert!(request(&req(upload()), model).is_err());
    }
}

#[test]
fn transcription_bounds_audio_and_rejects_remote_or_non_audio_input() {
    for patch in [
        json!({"base64":""}),
        json!({"base64":"%%%"}),
        json!({"base64":null}),
        json!({"base64":"a".repeat(MAX_INLINE_BYTES + 1)}),
        json!({"mime_type":"video/mp4"}),
        json!({"mime_type":"image/png"}),
        json!({"mime_type":["audio/wav"]}),
        json!({"mime_type":"application/octet-stream","file_name":"voice.exe"}),
    ] {
        let mut raw = upload();
        raw["file"]
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(request(&req(raw), "gemini-3-flash-preview").is_err());
    }
    let mut raw = upload();
    raw["file"] = json!({"url":"https://fixture.invalid/audio.mp3"});
    assert!(request(&req(raw), "gemini-3-flash-preview").is_err());
}

#[test]
fn transcription_requires_complete_non_thought_text_and_never_returns_a_truncated_prefix() {
    let parts =
        json!([{"text":"private reasoning", "thought":true},{"text":"你好"},{"text":"，巴黎。"}]);
    assert_eq!(
        transcript(&candidate(parts.clone(), "STOP")).unwrap(),
        "你好，巴黎。"
    );
    for body in [
        candidate(parts, "MAX_TOKENS"),
        json!({"done":false}),
        candidate(json!([{"text":"secret","thought":true}]), "STOP"),
        candidate(json!([{"text":"  "}]), "STOP"),
        candidate(
            json!([{"text":"prefix"},{"text":"a".repeat(1024 * 1024)}]),
            "STOP",
        ),
    ] {
        let error = transcript(&body).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_host_transcription_missing")
        );
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
    }
}

#[tokio::test]
async fn transcription_invalid_input_stops_before_browser_pool_startup() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"gemini_canvas_web_reverse_compatible","base_url":"http://127.0.0.1:1","api_key":"fixture"
    })).unwrap();
    let mut raw = upload();
    raw["response_format"] = json!("srt");
    let error = UpstreamClient::new(1)
        .execute_gemini_canvas_host_transcription(&payload, &req(raw), "gemini-3-flash-preview")
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("invalid_gemini_canvas_host_transcription_input")
    );
}
