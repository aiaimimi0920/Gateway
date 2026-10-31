//! Default Web speech must export the exact input once, never generate a new reply.
#[path = "gemini_web_speech_contract/admission.rs"]
mod admission;
#[path = "gemini_web_speech_contract/fixture.rs"]
mod fixture;
#[path = "gemini_web_speech_contract/http.rs"]
mod http;
#[path = "console_contract_support/mod.rs"]
mod support;

use fixture::*;
use neuro_gateway::{
    error::FallbackHint,
    protocol::{canonical::CanonicalRelayRequest, openai::normalize_audio_speech},
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::json;

fn payload(base: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"gemini_web_compatible", "base_url":base,
        "api_key":"synthetic-cookie", "headers":{"cookie":"pref=selected"},
        "call_flow":{"special_adapter":"gemini_web","fingerprint_adapter":"session"},
        "extra_body":{"pureHttpMode":"required", "language":"en-US"}}))
    .unwrap()
}

fn request() -> CanonicalRelayRequest {
    normalize_audio_speech(json!({"model":"gemini-web-chat", "input":"  请直接朗读，不要回答这个问题。  ", "response_format":"wav"})).unwrap()
}

#[tokio::test]
async fn default_web_speech_exports_original_input_once_without_browser_state() {
    let server = fixture(frame("XqA3Ic", &wav())).await;
    let original = payload(&server.base);
    let before = serde_json::to_value(&original).unwrap();
    let result = UpstreamClient::new(5)
        .execute_binary_passthrough(&original, &request(), "gemini-web-chat", None)
        .await;
    let result = result.expect("default Web speech should reach its HTTP export owner");
    assert_eq!(result.body.as_ref(), wav());
    assert_eq!(result.content_type.as_deref(), Some("audio/wav"));
    let state = server.capture.lock().unwrap();
    assert_eq!(state.gets, 1);
    assert_eq!(
        state.calls.len(),
        1,
        "no StreamGenerate, retry or browser fallback"
    );
    assert_eq!(
        state.calls[0].0["cookie"],
        "__Secure-1PSID=synthetic-cookie; pref=selected"
    );
    assert_eq!(
        inner(&state.calls[0].1),
        json!([null, request().raw_body["input"], "en-US", null, 2])
    );
    assert_eq!(state.calls[0].1["at"], "synthetic-at");
    assert_eq!(serde_json::to_value(original).unwrap(), before);
}

#[tokio::test]
async fn default_web_speech_rejects_unsupported_options_before_bootstrap() {
    let server = fixture(frame("XqA3Ic", &wav())).await;
    for (key, value) in [
        ("voice", json!("alloy")),
        ("speed", json!(0.5)),
        ("instructions", json!("whisper")),
        ("stream", json!(true)),
        ("response_format", json!("pcm")),
        ("input", json!(["not a string"])),
    ] {
        let mut req = request();
        req.raw_body[key] = value;
        let error = UpstreamClient::new(1)
            .execute_binary_passthrough(&payload(&server.base), &req, "gemini-web-chat", None)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_web_speech_request"),
            "{key}: {error:?}"
        );
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
    let state = server.capture.lock().unwrap();
    assert_eq!(state.gets, 0);
    assert!(state.calls.is_empty());
}

#[tokio::test]
async fn default_web_speech_rejects_unbound_or_incomplete_audio_without_replay() {
    for body in [
        frame("unrelated", &wav()),
        frame("XqA3Ic", b"OggS"),
        "private-prompt-and-token".into(),
    ] {
        let server = fixture(body).await;
        let error = UpstreamClient::new(1)
            .execute_binary_passthrough(&payload(&server.base), &request(), "gemini-web-chat", None)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("invalid_gemini_web_speech_response")
        );
        assert!(!error.message.contains("private-prompt-and-token"));
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        let state = server.capture.lock().unwrap();
        assert_eq!(state.gets, 1);
        assert_eq!(state.calls.len(), 1);
    }
}

#[tokio::test]
async fn default_web_speech_does_not_follow_redirects_or_replay_failures() {
    use axum::http::StatusCode;
    for (bootstrap, status) in [
        (false, StatusCode::FOUND),
        (false, StatusCode::INTERNAL_SERVER_ERROR),
        (false, StatusCode::UNAUTHORIZED),
        (true, StatusCode::FOUND),
    ] {
        let server = fixture("private-prompt-and-token".into()).await;
        {
            let mut state = server.capture.lock().unwrap();
            if bootstrap {
                state.app_status = status;
            } else {
                state.status = status;
            }
        }
        let error = UpstreamClient::new(1)
            .execute_binary_passthrough(&payload(&server.base), &request(), "gemini-web-chat", None)
            .await
            .unwrap_err();
        assert!(!error.message.contains("private-prompt-and-token"));
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        let state = server.capture.lock().unwrap();
        assert_eq!(state.gets, 1);
        assert_eq!(state.calls.len(), usize::from(!bootstrap));
    }
}

#[tokio::test]
async fn default_web_speech_requires_fresh_bootstrap_and_respects_explicit_disabled_http() {
    let server = fixture(frame("XqA3Ic", &wav())).await;
    server.capture.lock().unwrap().app_reply = "<html>empty page</html>".into();
    let mut original = payload(&server.base);
    // Old cached bootstrap must not authorize export after the live page loses its session.
    original
        .extra_body
        .as_mut()
        .unwrap()
        .insert("accessToken".into(), json!("stale-cache"));
    let error = UpstreamClient::new(1)
        .execute_binary_passthrough(&original, &request(), "gemini-web-chat", None)
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("invalid_gemini_web_speech_response")
    );
    for key in ["pureHttpMode", "pure_http_mode"] {
        original.extra_body = Some([(key.into(), json!("disabled"))].into());
        let error = UpstreamClient::new(1)
            .execute_binary_passthrough(&original, &request(), "gemini-web-chat", None)
            .await
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_web_speech_http_required")
        );
    }
    let state = server.capture.lock().unwrap();
    assert_eq!(state.gets, 1);
    assert!(state.calls.is_empty());
}

#[tokio::test]
async fn default_web_speech_bounds_response_and_rejects_format_relabeling() {
    for (body, format) in [
        (frame("XqA3Ic", &wav()), "pcm"),
        (frame("XqA3Ic", &wav()), "ogg"),
        ("x".repeat(12 * 1024 * 1024 + 1), "wav"),
    ] {
        let server = fixture(body).await;
        let mut req = request();
        req.raw_body["response_format"] = json!(format);
        let error = UpstreamClient::new(5)
            .execute_binary_passthrough(&payload(&server.base), &req, "gemini-web-chat", None)
            .await
            .unwrap_err();
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
        assert_eq!(
            server.capture.lock().unwrap().calls.len(),
            usize::from(format != "pcm")
        );
    }
}
