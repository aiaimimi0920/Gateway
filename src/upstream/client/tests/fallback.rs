use super::*;

#[test]
fn gemini_canvas_text_direct_http_fallback_prefers_harvested_runtime_api_keys() {
    let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    payload.api_key.clear();
    let mut runtime_payload = payload.clone();
    runtime_payload.api_key = "AIzaHarvestedKeyOne".to_string();
    let runtime_api = make_gemini_canvas_runtime_api_context(
        runtime_payload,
        &[
            "AIzaHarvestedKeyOne",
            "AIzaHarvestedKeyTwo",
            "AIzaHarvestedKeyOne",
        ],
        "https://gemini.google.com/share/demo",
    );

    let attempts =
        build_gemini_canvas_text_direct_http_fallback_attempts(&payload, Some(&runtime_api));

    assert_eq!(attempts.len(), 3);
    assert_eq!(attempts[0].label, "runtime_api_harvested_key");
    assert_eq!(attempts[0].api_key_override, Some("AIzaHarvestedKeyOne"));
    assert_eq!(
        attempts[0].referer_override,
        Some("https://gemini.google.com/share/demo")
    );
    assert!(attempts[0].preserve_cross_origin_referer);
    assert!(!attempts[0].include_signed_headers);
    assert_eq!(attempts[1].api_key_override, Some("AIzaHarvestedKeyTwo"));
    assert_eq!(attempts[2].label, "legacy_payload_direct_http");
    assert_eq!(attempts[2].api_key_override, None);
    assert!(!attempts[2].preserve_cross_origin_referer);
    assert!(attempts[2].include_signed_headers);
}

#[test]
fn gemini_canvas_text_direct_http_fallback_keeps_legacy_only_when_payload_has_api_key() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let runtime_api = make_gemini_canvas_runtime_api_context(
        payload.clone(),
        &["AIzaHarvestedKeyOne"],
        "https://gemini.google.com/share/demo",
    );

    let attempts =
        build_gemini_canvas_text_direct_http_fallback_attempts(&payload, Some(&runtime_api));

    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].label, "legacy_payload_direct_http");
    assert_eq!(attempts[0].api_key_override, None);
    assert!(attempts[0].include_signed_headers);
}

#[test]
fn gemini_canvas_image_browser_fallback_treats_auth_and_session_errors_as_retryable() {
    let auth_required =
        GatewayError::unauthorized("auth required").with_code("gemini_canvas_auth_required");
    assert!(should_fallback_gemini_canvas_image_to_browser(
        &auth_required
    ));

    let session_invalid = GatewayError::bad_request("session invalid")
        .with_code("gemini_canvas_pure_http_session_invalid");
    assert!(should_fallback_gemini_canvas_image_to_browser(
        &session_invalid
    ));

    let followup_missing = GatewayError::service_unavailable("missing asset")
        .with_code("gemini_canvas_media_followup_missing_asset");
    assert!(should_fallback_gemini_canvas_image_to_browser(
        &followup_missing
    ));

    let page_missing = GatewayError::server_error("page asset missing")
        .with_code("gemini_canvas_page_missing_image_asset");
    assert!(should_fallback_gemini_canvas_image_to_browser(
        &page_missing
    ));

    let gateway_timeout = GatewayError::service_unavailable("timed out");
    let gateway_timeout = GatewayError {
        http_status: Some(504),
        ..gateway_timeout
    };
    assert!(should_fallback_gemini_canvas_image_to_browser(
        &gateway_timeout
    ));

    let unsupported = GatewayError::bad_request("unsupported").with_code("unsupported_image_count");
    assert!(!should_fallback_gemini_canvas_image_to_browser(
        &unsupported
    ));
}
