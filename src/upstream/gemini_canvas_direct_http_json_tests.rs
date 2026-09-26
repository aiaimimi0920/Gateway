use super::direct_http_test_support::make_payload;
use super::*;
use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::upstream::gemini_canvas_client_types::GeminiCanvasDirectHttpApiKeyTransport;
use serde_json::{json, Value};
use std::future::Future;
#[test]
fn send_gemini_canvas_direct_http_json_with_options_returns_json_value_result() {
    fn assert_future_output<F>(_future: F)
    where
        F: Future<Output = Result<Value, GatewayError>>,
    {
    }

    let http = rquest::Client::new();
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let runtime = gemini_canvas::GeminiCanvasRuntime {
        runtime_state_object_key: "credential-runtime/gemini-canvas/test.json".to_string(),
        share_id: "share-123".to_string(),
        api_base_url: "https://generativelanguage.googleapis.com".to_string(),
    };
    let request_body = json!({
        "contents": [{
            "parts": [{ "text": "generate an image" }]
        }]
    });

    assert_future_output(send_gemini_canvas_direct_http_json_with_options(
        &http,
        &payload,
        &runtime,
        "https://generativelanguage.googleapis.com/v1beta/models/gemini:generateContent",
        &request_body,
        std::time::Duration::from_secs(1),
        Some("AIza-test"),
        GeminiCanvasDirectHttpApiKeyTransport::HeaderOnly,
        None,
        None,
        false,
        false,
        true,
    ));
}
#[test]
fn pure_http_invalid_json_error_matches_contract() {
    let error = gemini_canvas_pure_http_invalid_json_error("gemini_canvas_compatible", "bad json");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_pure_http_invalid_json")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas pure HTTP response did not return valid JSON: bad json"
    );
}
