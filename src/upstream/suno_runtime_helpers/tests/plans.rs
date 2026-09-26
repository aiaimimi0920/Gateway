use super::*;

#[test]
fn unsupported_request_plan_error_matches_contract() {
    let error = unsupported_request_plan_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(error.code.as_deref(), Some("unsupported_suno_endpoint"));
}

#[test]
fn plan_suno_chat_endpoint_rejected_locally() {
    let payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
    let req = make_request(EndpointKind::ChatCompletions);
    let err = crate::upstream::client::UpstreamClient::build_request_plan(
        &payload,
        &req,
        "chirp-v3-5",
        false,
    )
    .expect_err("suno chat requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_suno_endpoint"));
}

#[test]
fn build_suno_challenge_check_plan_uses_check_endpoint() {
    let plan = build_suno_challenge_check_plan(
        "https://studio-api-prod.suno.com/",
        EndpointKind::ImagesGenerations,
    );
    assert_suno_request_plan(
        &plan,
        "https://studio-api-prod.suno.com/api/c/check",
        EndpointKind::ImagesGenerations,
    );
    assert_eq!(
        plan.body.as_ref().unwrap(),
        &suno::build_challenge_check_request()
    );
}

#[test]
fn build_suno_generate_plan_uses_v2_web_endpoint() {
    let mut req = make_request(EndpointKind::MusicGenerations);
    req.raw_body = json!({
        "prompt": "lush synthwave chorus"
    });
    let plan = build_suno_generate_plan(
        "https://studio-api-prod.suno.com/",
        &req,
        "chirp-v3-5",
        Some("pro"),
    )
    .expect("suno generate plan");
    assert_suno_request_plan(
        &plan,
        "https://studio-api-prod.suno.com/api/generate/v2-web/",
        EndpointKind::MusicGenerations,
    );
    let body = plan.body.as_ref().expect("expected suno generate body");
    assert_eq!(body["gpt_description_prompt"], "lush synthwave chorus");
    assert_eq!(body["mv"], "chirp-auk-turbo");
    assert_eq!(body["generation_type"], "TEXT");
    assert_eq!(body["metadata"]["user_tier"], "pro");
    assert_eq!(body["metadata"]["web_client_pathname"], "/create");
    assert!(
        body["transaction_uuid"].as_str().is_some(),
        "generate plan should include a transaction uuid"
    );
    assert!(
        body["metadata"]["create_session_token"].as_str().is_some(),
        "generate plan should include a create session token"
    );
}

#[test]
fn build_suno_feed_poll_plan_uses_feed_v3_endpoint() {
    let clip_ids = vec!["clip_a".to_string(), "clip_b".to_string()];
    let plan = build_suno_feed_poll_plan(
        "https://studio-api-prod.suno.com/",
        EndpointKind::VideosGenerations,
        &clip_ids,
    );
    assert_suno_request_plan(
        &plan,
        "https://studio-api-prod.suno.com/api/feed/v3",
        EndpointKind::VideosGenerations,
    );
    assert_eq!(
        plan.body.as_ref().unwrap(),
        &suno::build_feed_poll_request(&clip_ids)
    );
}

#[test]
fn missing_runtime_cookie_error_matches_contracts() {
    let regular = missing_runtime_cookie_error(false);
    assert_eq!(regular.http_status, Some(500));
    assert_eq!(regular.code.as_deref(), Some("missing_suno_runtime_cookie"));
    assert_eq!(
        regular.message.as_str(),
        "Suno requests require runtime Cookie headers from keepalive ensure."
    );

    let browser = missing_runtime_cookie_error(true);
    assert_eq!(browser.http_status, Some(500));
    assert_eq!(browser.code.as_deref(), Some("missing_suno_runtime_cookie"));
    assert_eq!(
        browser.message.as_str(),
        "Suno browser-backed requests require runtime Cookie headers from keepalive ensure."
    );
}

#[test]
fn missing_runtime_bearer_error_matches_contracts() {
    let regular = missing_runtime_bearer_error(false);
    assert_eq!(regular.http_status, Some(500));
    assert_eq!(regular.code.as_deref(), Some("missing_suno_runtime_bearer"));
    assert_eq!(
        regular.message.as_str(),
        "Suno requests require a runtime Clerk bearer token from keepalive ensure."
    );

    let browser = missing_runtime_bearer_error(true);
    assert_eq!(browser.http_status, Some(500));
    assert_eq!(browser.code.as_deref(), Some("missing_suno_runtime_bearer"));
    assert_eq!(
        browser.message.as_str(),
        "Suno browser-backed requests require a runtime Clerk bearer token from keepalive ensure."
    );
}

#[test]
fn unsupported_suno_edit_endpoint_error_matches_contract() {
    let error = unsupported_suno_edit_endpoint_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(
        error.code.as_deref(),
        Some("unsupported_suno_edit_endpoint")
    );
    assert_eq!(
        error.message.as_str(),
        "Suno adapters do not currently support /v1/images/edits."
    );
}

#[test]
fn unsupported_suno_media_generation_endpoint_error_matches_contract() {
    let error = unsupported_suno_media_generation_endpoint_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(error.code.as_deref(), Some("unsupported_suno_endpoint"));
    assert_eq!(
        error.message.as_str(),
        "Suno adapters currently support only /v1/images/generations, /v1/videos/generations, and /v1/music/generations."
    );
}

#[test]
fn unsupported_suno_image_inputs_error_matches_contract() {
    let error = unsupported_suno_image_inputs_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(error.code.as_deref(), Some("unsupported_suno_image_inputs"));
    assert_eq!(
        error.message.as_str(),
        "Suno image generation currently does not support uploaded image or mask inputs."
    );
}

#[test]
fn unsupported_suno_video_count_error_matches_contract() {
    let error = unsupported_suno_video_count_error();
    assert_eq!(error.http_status, Some(400));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(error.code.as_deref(), Some("unsupported_suno_video_count"));
    assert_eq!(
        error.message.as_str(),
        "Suno video generation currently supports only n=1 requests."
    );
}

#[test]
fn validate_suno_media_request_rejects_uploaded_image_inputs() {
    let mut req = make_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "prompt": "cover art",
        "image": "data:image/png;base64,aGVsbG8=",
    });
    let err =
        validate_suno_media_request(&req).expect_err("uploaded image inputs should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_suno_image_inputs"));
}

#[test]
fn validate_suno_media_request_rejects_unsupported_endpoint() {
    let req = make_request(EndpointKind::ChatCompletions);
    let err =
        validate_suno_media_request(&req).expect_err("non-media endpoints should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.code.as_deref(), Some("unsupported_suno_endpoint"));
    assert_eq!(
        err.message.as_str(),
        "Suno adapters currently support only /v1/images/generations, /v1/videos/generations, and /v1/music/generations."
    );
}

#[test]
fn validate_suno_media_request_rejects_multi_video_output() {
    let mut req = make_request(EndpointKind::VideosGenerations);
    req.raw_body = json!({
        "prompt": "cinematic stage clip",
        "n": 2,
    });
    let err = validate_suno_media_request(&req).expect_err("n>1 video requests should be rejected");
    assert_eq!(err.http_status, Some(400));
    assert_eq!(err.code.as_deref(), Some("unsupported_suno_video_count"));
}

#[test]
fn build_suno_runtime_headers_sets_browser_contract() {
    let mut base_headers = HeaderMap::new();
    base_headers.insert("cookie", HeaderValue::from_static("a=b"));
    let headers = build_suno_runtime_headers(&base_headers);
    assert_eq!(
        headers.get("origin").and_then(|value| value.to_str().ok()),
        Some("https://suno.com")
    );
    assert_eq!(
        headers.get("referer").and_then(|value| value.to_str().ok()),
        Some("https://suno.com/")
    );
    assert_eq!(
        headers
            .get("referring-pathname")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );
    assert_eq!(
        headers
            .get("referring-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://suno.com")
    );
    assert!(
        headers.get("device-id").is_some(),
        "runtime headers should synthesize a device id when missing"
    );
    assert!(
        headers.get("browser-token").is_some(),
        "runtime headers should synthesize a browser token"
    );
}

#[test]
fn suno_user_tier_reads_camel_and_snake_case_extra_body_fields() {
    let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com");
    payload.extra_body = Some(HashMap::from([("userTier".to_string(), json!("  pro  "))]));
    assert_eq!(suno_user_tier(&payload).as_deref(), Some("pro"));

    payload.extra_body = Some(HashMap::from([("user_tier".to_string(), json!("free"))]));
    assert_eq!(suno_user_tier(&payload).as_deref(), Some("free"));
}
