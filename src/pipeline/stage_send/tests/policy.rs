use super::*;

#[test]
fn classify_failure_kind_rate_limit() {
    let e = GatewayError::rate_limited("too fast", 1000);
    assert_eq!(classify_failure_kind(&e), FailureKind::RateLimited);
}

#[test]
fn classify_failure_kind_server_error() {
    let e = GatewayError::server_error("boom");
    assert_eq!(classify_failure_kind(&e), FailureKind::General);
}

#[test]
fn should_try_next_on_server_error() {
    let e = GatewayError::server_error("upstream failed");
    assert!(should_try_next_candidate(&e));
}

#[test]
fn should_not_try_next_on_auth_error() {
    let e = GatewayError::unauthorized("bad creds");
    assert!(!should_try_next_candidate(&e));
}

#[test]
fn should_not_try_next_on_bad_request() {
    let e = GatewayError::bad_request("malformed");
    assert!(!should_try_next_candidate(&e));
}

#[test]
fn should_not_try_next_on_indeterminate_rate_limit_admission() {
    let error = GatewayError::service_unavailable("rate-limit admission result is indeterminate")
        .with_code("rate_limit_admission_indeterminate");

    assert!(!should_try_next_candidate(&error));
}

#[test]
fn indeterminate_rate_limit_admission_is_not_provider_failure_feedback() {
    let error = GatewayError::service_unavailable("rate-limit admission result is indeterminate")
        .with_code("rate_limit_admission_indeterminate");

    assert!(!should_record_provider_failure(&error));
    assert!(should_record_provider_failure(&GatewayError::server_error(
        "upstream unavailable",
    )));
}

#[test]
fn chatgpt_web_escalates_stable_server_failures_with_access_token_only() {
    let candidate = make_candidate("chatgpt_web_reverse_compatible");
    let error = GatewayError::server_error("Internal Server Error");

    assert!(should_escalate_chatgpt_web_to_browser_relay(
        &candidate.payload,
        &error,
    ));
}

#[test]
fn chatgpt_web_request_time_browser_is_allowed_by_default() {
    let candidate = make_candidate("chatgpt_web_reverse_compatible");

    assert!(chatgpt_web_request_time_browser_allowed(&candidate.payload));
}

#[test]
fn chatgpt_web_request_time_browser_can_be_disabled_by_bool_payload_flag() {
    let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
    let mut extra_body = HashMap::new();
    extra_body.insert(
        "requestTimeBrowserAllowed".to_string(),
        serde_json::json!(false),
    );
    candidate.payload.extra_body = Some(extra_body);

    assert!(!chatgpt_web_request_time_browser_allowed(
        &candidate.payload
    ));
}

#[test]
fn chatgpt_web_request_time_browser_can_be_disabled_by_mode_payload_flag() {
    let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
    let mut extra_body = HashMap::new();
    extra_body.insert(
        "requestTimeBrowserMode".to_string(),
        serde_json::json!("pure_http_only"),
    );
    candidate.payload.extra_body = Some(extra_body);

    assert!(!chatgpt_web_request_time_browser_allowed(
        &candidate.payload
    ));
}

#[test]
fn chatgpt_web_browser_fallback_forbidden_error_is_explicit() {
    let error = chatgpt_web_browser_fallback_forbidden_error(&GatewayError::server_error(
        "Internal Server Error",
    ));

    assert_eq!(
        error.code.as_deref(),
        Some("chatgpt_web_request_time_browser_forbidden")
    );
    assert!(error.message.contains("pure HTTP"));
}

#[test]
fn chatgpt_web_escalates_stable_server_failures_with_cookie_only_runtime() {
    let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
    candidate.payload.api_key = String::new();
    candidate.payload.headers.insert(
        "Cookie".to_string(),
        "__Secure-next-auth.session-token=abc; cf_clearance=def".to_string(),
    );
    let error = GatewayError::server_error("Internal Server Error");

    assert!(should_escalate_chatgpt_web_to_browser_relay(
        &candidate.payload,
        &error,
    ));
}

#[test]
fn chatgpt_web_does_not_escalate_stable_server_failures_without_runtime_seed() {
    let mut candidate = make_candidate("chatgpt_web_reverse_compatible");
    candidate.payload.api_key = String::new();
    let error = GatewayError::server_error("Internal Server Error");

    assert!(!should_escalate_chatgpt_web_to_browser_relay(
        &candidate.payload,
        &error,
    ));
}

#[test]
fn retry_policy_for_udio_media_excludes_rate_limit_retries() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    let candidate = make_candidate("udio_compatible");

    let policy = retry_policy_for_request(&req, &candidate.payload);

    assert!(!policy.retryable_statuses.contains(&429));
    assert!(policy.retryable_statuses.contains(&500));
}

#[test]
fn retry_policy_for_standard_chat_keeps_rate_limit_retries() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let candidate = make_candidate("openai_compatible");

    let policy = retry_policy_for_request(&req, &candidate.payload);

    assert!(policy.retryable_statuses.contains(&429));
}

#[test]
fn retry_policy_for_gemini_canvas_image_edits_disables_provider_retries() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
    let candidate = make_candidate("gemini_canvas_compatible");

    let policy = retry_policy_for_request(&req, &candidate.payload);

    assert_eq!(policy.max_retries, 0);
}

#[test]
fn retry_policy_for_gemini_canvas_program_chat_disables_provider_retries() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::ChatCompletions);
    let candidate = make_candidate("gemini_canvas_program_web_reverse_compatible");

    let policy = retry_policy_for_request(&req, &candidate.payload);

    assert_eq!(policy.max_retries, 0);
}

#[test]
fn retry_policy_for_gemini_canvas_program_video_excludes_rate_limit_retries() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let candidate = make_candidate("gemini_canvas_program_web_reverse_compatible");

    let policy = retry_policy_for_request(&req, &candidate.payload);

    assert!(!policy.retryable_statuses.contains(&429));
    assert!(policy.retryable_statuses.contains(&500));
}

#[test]
fn retry_policy_for_gemini_canvas_browser_video_excludes_rate_limit_retries() {
    let req = make_req(ProtocolFamily::OpenAi, EndpointKind::VideosGenerations);
    let candidate = make_candidate("gemini_canvas_web_reverse_compatible");

    let policy = retry_policy_for_request(&req, &candidate.payload);

    assert!(!policy.retryable_statuses.contains(&429));
}
