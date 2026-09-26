use super::*;

#[test]
fn classify_udio_html_checkpoint_as_browser_challenge() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::CONTENT_TYPE,
        rquest::header::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    let err = classify_udio_upstream_error(
        403,
        &headers,
        "<!DOCTYPE html><title>Vercel Security Checkpoint</title>",
    );
    assert_eq!(err.code.as_deref(), Some("udio_browser_challenge_required"));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn classify_udio_user_disallowed_as_browser_challenge() {
    let headers = rquest::header::HeaderMap::new();
    let err = classify_udio_upstream_error(403, &headers, r#"{"error":"User disallowed"}"#);
    assert_eq!(err.code.as_deref(), Some("udio_browser_challenge_required"));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn classify_udio_401_as_session_unauthorized() {
    let headers = rquest::header::HeaderMap::new();
    let err = classify_udio_upstream_error(401, &headers, r#"{"detail":"Unauthorized"}"#);
    assert_eq!(err.code.as_deref(), Some("udio_session_unauthorized"));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn classify_udio_media_fetch_error_preserves_challenge_contract() {
    let mut headers = rquest::header::HeaderMap::new();
    headers.insert(
        rquest::header::HeaderName::from_static("x-vercel-mitigated"),
        rquest::header::HeaderValue::from_static("challenge"),
    );
    let err = classify_udio_media_fetch_error(429, &headers, "rate limited");
    assert_eq!(err.code.as_deref(), Some("udio_browser_challenge_required"));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn classify_udio_media_fetch_error_preserves_unauthorized_contract() {
    let headers = rquest::header::HeaderMap::new();
    let err = classify_udio_media_fetch_error(401, &headers, r#"{"detail":"Unauthorized"}"#);
    assert_eq!(err.code.as_deref(), Some("udio_session_unauthorized"));
    assert_eq!(err.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn ensure_successful_udio_media_fetch_status_accepts_2xx_contract() {
    let headers = rquest::header::HeaderMap::new();

    ensure_successful_udio_media_fetch_status(204, &headers, "").expect("2xx should pass");
}

#[test]
fn ensure_successful_udio_media_fetch_status_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error =
        ensure_successful_udio_media_fetch_status(401, &headers, "{\"detail\":\"Unauthorized\"}")
            .expect_err("non-2xx should fail");

    assert_eq!(error.code.as_deref(), Some("udio_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
}
