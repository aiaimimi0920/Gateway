use super::assert_management_access_with_expected;
use axum::http::HeaderMap;

fn header_map(name: &'static str, value: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(name, value.parse().expect("valid header value"));
    headers
}

#[test]
fn management_access_rejects_missing_config_by_default() {
    let err = assert_management_access_with_expected(None, false, None, &HeaderMap::new())
        .expect_err("missing management token must fail closed");

    assert_eq!(err.http_status, Some(503));
    assert_eq!(
        err.code.as_deref(),
        Some("gateway_management_token_not_configured")
    );
}

#[test]
fn management_access_allows_missing_config_only_with_explicit_override() {
    assert!(assert_management_access_with_expected(None, true, None, &HeaderMap::new()).is_ok());
}

#[test]
fn management_access_accepts_configured_management_headers() {
    assert!(assert_management_access_with_expected(
        Some("secret"),
        false,
        None,
        &header_map("x-management-token", "secret")
    )
    .is_ok());

    assert!(assert_management_access_with_expected(
        Some("secret"),
        false,
        None,
        &header_map("x-internal-api-key", "secret")
    )
    .is_ok());
}

#[test]
fn management_access_accepts_bearer_and_rejects_wrong_token() {
    assert!(assert_management_access_with_expected(
        Some("secret"),
        false,
        Some("secret"),
        &HeaderMap::new()
    )
    .is_ok());

    let err = assert_management_access_with_expected(
        Some("secret"),
        false,
        Some("wrong"),
        &HeaderMap::new(),
    )
    .expect_err("wrong management token must be rejected");
    assert_eq!(err.http_status, Some(401));
}
