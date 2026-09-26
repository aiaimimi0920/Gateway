use super::*;

#[test]
fn unsupported_media_adapter_endpoint_error_matches_contract() {
    let error = unsupported_media_adapter_endpoint_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_endpoint")
    );
    assert_eq!(
            error.message.as_str(),
            "Gemini Canvas adapters currently support /v1/images/generations, /v1/images/edits, /v1/music/generations, and /v1/videos/generations."
        );
}

#[test]
fn gemini_canvas_unsupported_request_plan_error_matches_contract() {
    let error = unsupported_request_plan_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_endpoint")
    );
}

#[test]
fn plan_gemini_canvas_chat_endpoint_rejected_locally() {
    let payload = pure_http_mode_payload(None);
    let req = make_request(EndpointKind::ChatCompletions);
    let err = crate::upstream::client::UpstreamClient::build_request_plan(
        &payload,
        &req,
        GEMINI_CANVAS_DEFAULT_MODEL,
        false,
    )
    .expect_err("gemini canvas chat requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(
        err.code.as_deref(),
        Some("unsupported_gemini_canvas_endpoint")
    );
}

#[test]
fn unsupported_modular_endpoint_error_matches_contract() {
    let error = unsupported_modular_endpoint_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_modular_endpoint")
    );
    assert_eq!(
            error.message.as_str(),
            "Gemini Canvas modular browser relay currently supports /v1/images/generations, /v1/music/generations, /v1/videos/generations, and /v1/audio/speech."
        );
}

#[test]
fn unsupported_image_count_error_matches_contract() {
    let error = unsupported_image_count_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_image_count")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image generation currently supports only n=1 requests."
    );
}

#[test]
fn unsupported_modular_image_count_error_matches_contract() {
    let error = unsupported_modular_image_count_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_modular_image_count")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas browser relay image generation currently supports only n=1 requests."
    );
}

#[test]
fn unsupported_image_edit_count_error_matches_contract() {
    let error = unsupported_image_edit_count_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_image_edit_count")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas image edits currently support only n=1 requests."
    );
}

#[test]
fn unsupported_modular_image_edits_error_matches_contract() {
    let error = unsupported_modular_image_edits_error("gemini_canvas_compatible");
    assert_eq!(error.http_status, Some(400));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_gemini_canvas_modular_image_edits")
    );
    assert_eq!(
            error.message.as_str(),
            "Gemini Canvas modular browser relay does not implement image edits yet. Keep using the legacy mixed lane until the true browser-owned edit flow is split out."
        );
}
