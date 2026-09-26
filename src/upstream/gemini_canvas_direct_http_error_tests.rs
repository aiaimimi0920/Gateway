use super::*;
use crate::error::GatewayError;
#[test]
fn direct_http_image_json_attempts_exhausted_error_matches_contract() {
    let error = gemini_canvas_image_json_attempts_exhausted_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_json_attempts_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP image replay exhausted all known JSON contracts."
    );
}

#[test]
fn direct_http_page_harvest_exhausted_error_matches_contract() {
    let error = gemini_canvas_page_harvest_exhausted_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_page_harvest_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP page harvest exhausted all request modes."
    );
}

#[test]
fn direct_http_page_harvest_redirect_loop_error_matches_contract() {
    let error = gemini_canvas_page_harvest_redirect_loop_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_page_harvest_redirect_loop")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP page harvest exceeded the redirect follow limit."
    );
}

#[test]
fn direct_http_page_harvest_redirect_missing_location_error_matches_contract() {
    let error = gemini_canvas_page_harvest_redirect_missing_location_error(302);
    assert_eq!(error.http_status, Some(302));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_page_harvest_redirect_missing_location")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP page harvest returned a redirect without a usable Location header."
    );
}

#[test]
fn direct_http_page_harvest_unsafe_redirect_error_matches_contract() {
    let error = gemini_canvas_page_harvest_unsafe_redirect_error(302);
    assert_eq!(error.http_status, Some(302));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_page_harvest_unsafe_redirect")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP page harvest refused a cross-origin, credential-bearing, or non-HTTP(S) redirect target."
    );
}
#[test]
fn build_gemini_canvas_direct_http_page_harvest_failure_error_preserves_upstream_contract() {
    let attempted_urls = vec![
        "https://gemini.google.com/share/example".to_string(),
        "https://gemini.google.com/app".to_string(),
    ];
    let failures = vec![
        "https://gemini.google.com/share/example: 503 challenge".to_string(),
        "https://gemini.google.com/app: 401 session_invalid".to_string(),
    ];
    let mut upstream = GatewayError::unauthorized("session invalid")
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_pure_http_session_invalid");
    upstream.http_status = Some(401);

    let error = build_gemini_canvas_direct_http_page_harvest_failure_error(
        Some(upstream),
        &attempted_urls,
        "https://gemini.google.com/share/example",
        &failures,
    );

    assert_eq!(error.http_status, Some(401));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_pure_http_session_invalid")
    );
    assert_eq!(
        error.message,
        "session invalid; bootstrap_request_contract=bootstrap_source=page_harvest_helper, attempted_urls=https://gemini.google.com/share/example,https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/share/example, failures=https://gemini.google.com/share/example: 503 challenge | https://gemini.google.com/app: 401 session_invalid"
    );
}

#[test]
fn build_gemini_canvas_direct_http_page_harvest_failure_error_falls_back_to_bootstrap_contract() {
    let attempted_urls = vec!["https://gemini.google.com/app".to_string()];
    let failures = vec!["https://gemini.google.com/app: 503 exhausted".to_string()];

    let error = build_gemini_canvas_direct_http_page_harvest_failure_error(
        None,
        &attempted_urls,
        "https://gemini.google.com/app",
        &failures,
    );

    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_bootstrap_exhausted")
    );
    assert_eq!(
        error.message,
        "Gemini Canvas media bootstrap exhausted all page harvest candidates.; bootstrap_request_contract=bootstrap_source=page_harvest_helper, attempted_urls=https://gemini.google.com/app, is_text_mode=false, session_target_url=https://gemini.google.com/app, failures=https://gemini.google.com/app: 503 exhausted"
    );
}

#[test]
fn build_gemini_canvas_direct_http_text_bootstrap_failure_error_preserves_session_invalid_contract()
{
    let error = build_gemini_canvas_direct_http_text_bootstrap_failure_error(
        401,
        Some("text/html; charset=utf-8"),
        "<!DOCTYPE html><a href=\"https://accounts.google.com\">sign in</a>",
        "bootstrap_url=https://gemini.google.com/share/example, is_text_mode=true, cookie=<present>, origin=https://gemini.google.com, referer=https://gemini.google.com/share/example",
        "final_url=https://gemini.google.com/share/example, location=<none>, content_type=text/html; charset=utf-8, body_preview=<html>sign in</html>",
    );

    assert_eq!(error.http_status, Some(401));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_pure_http_session_invalid")
    );
    assert_eq!(
        error.message,
        "Gemini Canvas pure HTTP replay session is invalid or expired.; bootstrap_request_contract=bootstrap_url=https://gemini.google.com/share/example, is_text_mode=true, cookie: [REDACTED], origin=https://gemini.google.com, referer=https://gemini.google.com/share/example; bootstrap_response_meta=final_url=https://gemini.google.com/share/example, location=<none>, content_type=text/html; charset=utf-8, body_preview=<html>sign in</html>"
    );
}

#[test]
fn media_bootstrap_exhausted_error_matches_contract() {
    let error = gemini_canvas_media_bootstrap_exhausted_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_bootstrap_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas media bootstrap exhausted all page harvest candidates."
    );
}

#[test]
fn media_fetch_redirect_exhausted_error_matches_contract() {
    let error = gemini_canvas_media_fetch_redirect_exhausted_error();
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_fetch_redirect_exhausted")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP media download exhausted redirect/handoff attempts."
    );
}

#[test]
fn media_fetch_cookie_mismatch_redirect_error_matches_contract() {
    let error = gemini_canvas_media_fetch_cookie_mismatch_redirect_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_fetch_cookie_mismatch")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP media download redirected into Google account login/CookieMismatch instead of returning the requested asset."
    );
}

#[test]
fn media_fetch_cookie_mismatch_html_error_matches_contract() {
    let error = gemini_canvas_media_fetch_cookie_mismatch_html_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_fetch_cookie_mismatch")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP media download resolved to a Google login/CookieMismatch HTML page instead of a binary asset."
    );
}

#[test]
fn media_fetch_bad_redirect_error_matches_contract() {
    let error = gemini_canvas_media_fetch_bad_redirect_error(
        "https://user:password@attacker.example/hop?token=secret#fragment",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_fetch_bad_redirect")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP media redirect URL was not usable: https://attacker.example/hop"
    );
}

#[test]
fn media_fetch_redirect_missing_location_error_matches_contract() {
    let error = gemini_canvas_media_fetch_redirect_missing_location_error();
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_fetch_redirect_missing_location")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP media download returned a redirect without a usable Location header."
    );
}

#[test]
fn media_fetch_bad_asset_url_error_matches_contract() {
    let error = gemini_canvas_media_fetch_bad_asset_url_error(
        "https://user:password@attacker.example/media?api_key=secret#fragment",
    );
    assert_eq!(error.http_status, Some(500));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_media_fetch_bad_asset_url")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP media asset URL was not usable: https://attacker.example/media"
    );
}

#[test]
fn image_fetch_missing_inline_bytes_error_matches_contract() {
    let error = gemini_canvas_image_fetch_missing_inline_bytes_error();
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_fetch_missing_inline_bytes")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP image materialization completed without inline bytes."
    );
}

#[test]
fn image_fetch_invalid_inline_bytes_error_matches_contract() {
    let error = gemini_canvas_image_fetch_invalid_inline_bytes_error("bad base64");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_image_fetch_invalid_inline_bytes")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas direct HTTP image materialization returned invalid base64 bytes: bad base64"
    );
}
