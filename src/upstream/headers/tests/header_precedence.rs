use super::*;

#[test]
fn search_api_can_request_json_response_with_custom_accept_header() {
    let mut payload = make_payload("search_api_compatible");
    payload
        .headers
        .insert("Accept".to_string(), "application/json".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "authorization"),
        Some("Bearer sk-test-1234")
    );
    assert_eq!(header_str(&headers, "accept"), Some("application/json"));
    assert_eq!(
        header_str(&headers, "content-type"),
        Some("application/json")
    );
}

#[test]
fn gemini_api_ignores_forwarded_x_goog_api_key_override() {
    let payload = make_payload("gemini_api_compatible");
    let mut extra_headers = HashMap::new();
    extra_headers.insert(
        "x-goog-api-key".to_string(),
        "gateway-platform-key".to_string(),
    );
    extra_headers.insert(
        "authorization".to_string(),
        "Bearer gateway-platform-key".to_string(),
    );
    let headers = build_upstream_headers_with(&payload, Some(&extra_headers));
    assert_eq!(header_str(&headers, "x-goog-api-key"), Some("sk-test-1234"));
    assert!(headers.get("authorization").is_none());
}

#[test]
fn account_group_headers_are_never_forwarded_to_upstream() {
    let mut payload = make_payload("openai_compatible");
    payload.headers.insert(
        "x-neuro-account-group".to_string(),
        "provider-internal".to_string(),
    );
    payload.headers.insert(
        "X-Account-Group-Id".to_string(),
        "provider-legacy".to_string(),
    );
    let extra_headers = HashMap::from([
        (
            "x-neuro-account-group".to_string(),
            "internal-premium".to_string(),
        ),
        (
            "X-Account-Group-Id".to_string(),
            "internal-legacy".to_string(),
        ),
        ("x-request-id".to_string(), "request-42".to_string()),
    ]);

    let headers = build_upstream_headers_with(&payload, Some(&extra_headers));

    assert!(headers.get("x-neuro-account-group").is_none());
    assert!(headers.get("x-account-group-id").is_none());
    assert_eq!(header_str(&headers, "x-request-id"), Some("request-42"));
}

#[test]
fn internal_account_group_header_cannot_be_reintroduced_as_auth_header() {
    let mut payload = make_payload("custom_http");
    payload.auth_header_name = Some("X-Account-Group-Id".to_string());
    payload.auth_token = Some("internal-selector-value".to_string());

    let headers = build_upstream_headers(&payload);

    assert!(headers.get("x-account-group-id").is_none());
}

#[test]
fn custom_headers_are_appended() {
    let mut payload = make_payload("openai_compatible");
    payload
        .headers
        .insert("x-trace-id".to_string(), "trace-abc".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "x-trace-id"), Some("trace-abc"));
}

#[test]
fn custom_headers_can_override_default_headers() {
    let mut payload = make_payload("openai_compatible");
    payload.headers.insert(
        "content-type".to_string(),
        "application/json; charset=utf-8".to_string(),
    );
    let headers = build_upstream_headers(&payload);
    assert_eq!(
        header_str(&headers, "content-type"),
        Some("application/json; charset=utf-8")
    );
}

#[test]
fn accio_custom_headers_override_defaults() {
    let mut payload = make_payload("accio_compatible");
    payload
        .headers
        .insert("x-utdid".to_string(), "utd-test".to_string());
    payload
        .headers
        .insert("x-app-version".to_string(), "0.5.9".to_string());
    payload
        .headers
        .insert("appKey".to_string(), "99999999".to_string());
    payload
        .headers
        .insert("x-language".to_string(), "en-US".to_string());
    payload
        .headers
        .insert("x-os".to_string(), "darwin".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "utdid"), Some("utd-test"));
    assert_eq!(header_str(&headers, "version"), Some("0.5.9"));
    assert_eq!(header_str(&headers, "x-app-version"), Some("0.5.9"));
    assert_eq!(header_str(&headers, "appKey"), Some("99999999"));
    assert_eq!(header_str(&headers, "x-language"), Some("en-US"));
    assert_eq!(header_str(&headers, "x-os"), Some("darwin"));
}
