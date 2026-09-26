use super::super::*;

#[test]
fn request_time_browser_worker_blocking_error_maps_policy_codes() {
    assert_eq!(
        request_time_local_browser_worker_blocking_error(
            RequestTimeBrowserPolicy::Disabled,
            "chatgpt_web_reverse_compatible",
        )
        .and_then(|error| error.code),
        Some("request_time_browser_forbidden".to_string())
    );
    assert_eq!(
        request_time_local_browser_worker_blocking_error(
            RequestTimeBrowserPolicy::RemoteOnly,
            "qwen_web_compatible",
        )
        .and_then(|error| error.code),
        Some("browser_executor_required_unavailable".to_string())
    );
    assert!(request_time_local_browser_worker_blocking_error(
        RequestTimeBrowserPolicy::LocalAllowed,
        "qwen_web_compatible",
    )
    .is_none());
}

#[test]
fn keepalive_probe_headers_drop_internal_account_group_selectors() {
    let headers = HashMap::from([
        ("x-neuro-account-group".to_string(), "premium".to_string()),
        (
            "X-Account-Group-Id".to_string(),
            "legacy-premium".to_string(),
        ),
        ("x-provider-runtime".to_string(), "allowed".to_string()),
    ]);

    let result = build_keepalive_probe_headers(Some(&headers), None, "session-token");

    assert!(!result
        .keys()
        .any(|name| name.eq_ignore_ascii_case("x-neuro-account-group")));
    assert!(!result
        .keys()
        .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
    assert_eq!(
        result.get("x-provider-runtime").map(String::as_str),
        Some("allowed")
    );
}

#[test]
fn keepalive_probe_auth_cannot_reintroduce_internal_account_group_selector() {
    let session_auth = SessionAuthConfig {
        transport: "header".to_string(),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        header_name: Some("X-Account-Group-Id".to_string()),
        expires_at: None,
    };

    let result = build_keepalive_probe_headers(None, Some(&session_auth), "session-token");

    assert!(!result
        .keys()
        .any(|name| name.eq_ignore_ascii_case("x-account-group-id")));
}

#[test]
fn raw_keepalive_request_builder_drops_internal_account_group_selectors() {
    let headers = HashMap::from([
        ("x-neuro-account-group".to_string(), "premium".to_string()),
        (
            "X-Account-Group-Id".to_string(),
            "legacy-premium".to_string(),
        ),
        ("x-provider-runtime".to_string(), "allowed".to_string()),
    ]);

    let request = request_builder_with_headers(
        &Client::new(),
        rquest::Method::GET,
        "https://example.com/probe",
        &headers,
    )
    .build()
    .expect("build keepalive request");

    assert!(request.headers().get("x-neuro-account-group").is_none());
    assert!(request.headers().get("x-account-group-id").is_none());
    assert_eq!(
        request
            .headers()
            .get("x-provider-runtime")
            .and_then(|value| value.to_str().ok()),
        Some("allowed")
    );
}
