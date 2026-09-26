use super::*;

#[test]
fn suno_challenge_required_error_matches_contract() {
    let error = suno_challenge_required_error();
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(error.code.as_deref(), Some("suno_challenge_required"));
    assert_eq!(
        error.message.as_str(),
        "Suno requires an active browser challenge token before generation can continue."
    );
}

#[test]
fn classify_suno_empty_422_as_challenge_when_token_missing() {
    let headers = rquest::header::HeaderMap::new();
    let err = classify_suno_upstream_error(422, &headers, "", true);
    assert_eq!(err.code.as_deref(), Some("suno_challenge_required"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn classify_suno_401_as_session_unauthorized() {
    let headers = rquest::header::HeaderMap::new();
    let err = classify_suno_upstream_error(401, &headers, "{\"detail\":\"Unauthorized\"}", false);
    assert_eq!(err.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn classify_suno_media_fetch_error_preserves_unauthorized_contract() {
    let headers = rquest::header::HeaderMap::new();
    let error = classify_suno_media_fetch_error(401, &headers, "{\"detail\":\"Unauthorized\"}");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn classify_suno_media_fetch_error_preserves_generic_failure_contract() {
    let headers = rquest::header::HeaderMap::new();
    let error = classify_suno_media_fetch_error(503, &headers, "service unavailable");

    assert_eq!(error.http_status, Some(503));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
    assert!(error.message.contains("service unavailable"));
}

#[test]
fn ensure_successful_suno_media_fetch_status_accepts_2xx_contract() {
    let headers = rquest::header::HeaderMap::new();

    ensure_successful_suno_media_fetch_status(204, &headers, "").expect("2xx should pass");
}

#[test]
fn ensure_successful_suno_media_fetch_status_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error =
        ensure_successful_suno_media_fetch_status(401, &headers, "{\"detail\":\"Unauthorized\"}")
            .expect_err("non-2xx should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn ensure_successful_suno_upstream_status_accepts_2xx_contract() {
    let headers = rquest::header::HeaderMap::new();

    ensure_successful_suno_upstream_status(204, &headers, "", false).expect("2xx should pass");
}

#[test]
fn ensure_successful_suno_upstream_status_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error = ensure_successful_suno_upstream_status(
        401,
        &headers,
        "{\"detail\":\"Unauthorized\"}",
        false,
    )
    .expect_err("non-2xx should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}
