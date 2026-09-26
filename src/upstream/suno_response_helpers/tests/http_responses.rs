use super::*;

#[test]
fn parse_suno_challenge_probe_body_reports_invalid_json_contract() {
    let err = parse_suno_challenge_probe_body("not-json").expect_err("invalid JSON should fail");
    assert_eq!(
        err.code.as_deref(),
        Some("suno_invalid_challenge_probe_body")
    );
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert!(err
        .message
        .starts_with("Suno challenge probe response body was not valid JSON."));
}

#[test]
fn parse_suno_challenge_probe_http_response_reads_success_contract() {
    let headers = rquest::header::HeaderMap::new();

    let body = parse_suno_challenge_probe_http_response(
        200,
        &headers,
        "{\"requiresChallenge\":false}",
        false,
    )
    .expect("valid challenge response should parse");

    assert_eq!(body["requiresChallenge"], false);
}

#[test]
fn parse_suno_challenge_probe_http_response_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error = parse_suno_challenge_probe_http_response(
        401,
        &headers,
        "{\"detail\":\"Unauthorized\"}",
        false,
    )
    .expect_err("non-2xx response should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_challenge_probe_verified_response_reads_success_contract() {
    let headers = rquest::header::HeaderMap::new();

    let body =
        parse_suno_challenge_probe_verified_response(200, &headers, "{\"required\":false}", true)
            .expect("valid challenge probe should pass");

    assert_eq!(body["required"], false);
}

#[test]
fn parse_suno_challenge_probe_verified_response_preserves_required_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error =
        parse_suno_challenge_probe_verified_response(200, &headers, "{\"required\":true}", true)
            .expect_err("required challenge without token should fail");

    assert_eq!(error.code.as_deref(), Some("suno_challenge_required"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_generation_body_reports_invalid_json_contract() {
    let err = parse_suno_generation_body("not-json").expect_err("invalid JSON should fail");
    assert_eq!(err.code.as_deref(), Some("suno_invalid_generation_body"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert!(err
        .message
        .starts_with("Suno generation response body was not valid JSON."));
}

#[test]
fn parse_suno_generation_http_response_reads_success_contract() {
    let headers = rquest::header::HeaderMap::new();

    let body = parse_suno_generation_http_response(200, &headers, "{\"clips\":[]}", false)
        .expect("valid generation response should parse");

    assert!(body["clips"].is_array());
}

#[test]
fn parse_suno_generation_http_response_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error =
        parse_suno_generation_http_response(401, &headers, "{\"detail\":\"Unauthorized\"}", false)
            .expect_err("non-2xx response should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_generation_http_clips_response_reads_success_contract() {
    let headers = rquest::header::HeaderMap::new();

    let (body, clips) = parse_suno_generation_http_clips_response(
        200,
        &headers,
        "{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}]}",
        false,
    )
    .expect("valid generation clips response should parse");

    assert!(body["clips"].is_array());
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].id, "clip-1");
}

#[test]
fn parse_suno_generation_http_clips_response_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error = parse_suno_generation_http_clips_response(
        401,
        &headers,
        "{\"detail\":\"Unauthorized\"}",
        false,
    )
    .expect_err("non-2xx response should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_generation_http_poll_seed_reads_success_contract() {
    let headers = rquest::header::HeaderMap::new();

    let (clips, clip_ids) = parse_suno_generation_http_poll_seed(
        200,
        &headers,
        "{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}]}",
        false,
    )
    .expect("valid generation poll seed should parse");

    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].id, "clip-1");
    assert_eq!(clip_ids, vec!["clip-1".to_string()]);
}

#[test]
fn parse_suno_generation_http_poll_seed_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error =
        parse_suno_generation_http_poll_seed(401, &headers, "{\"detail\":\"Unauthorized\"}", false)
            .expect_err("non-2xx response should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_feed_body_reports_invalid_json_contract() {
    let err = parse_suno_feed_body("not-json").expect_err("invalid JSON should fail");
    assert_eq!(err.code.as_deref(), Some("suno_invalid_feed_body"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert!(err
        .message
        .starts_with("Suno feed poll response body was not valid JSON."));
}

#[test]
fn parse_suno_feed_http_response_reads_success_contract() {
    let headers = rquest::header::HeaderMap::new();

    let body = parse_suno_feed_http_response(200, &headers, "{\"clips\":[]}", false)
        .expect("valid feed response should parse");

    assert!(body["clips"].is_array());
}

#[test]
fn parse_suno_feed_http_response_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error =
        parse_suno_feed_http_response(401, &headers, "{\"detail\":\"Unauthorized\"}", false)
            .expect_err("non-2xx response should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_feed_http_clips_response_reads_success_contract() {
    let headers = rquest::header::HeaderMap::new();

    let clips = parse_suno_feed_http_clips_response(
        200,
        &headers,
        "{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}]}",
        false,
    )
    .expect("valid feed clips response should parse");

    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].id, "clip-1");
}

#[test]
fn parse_suno_feed_http_clips_response_preserves_failure_contract() {
    let headers = rquest::header::HeaderMap::new();

    let error =
        parse_suno_feed_http_clips_response(401, &headers, "{\"detail\":\"Unauthorized\"}", false)
            .expect_err("non-2xx response should fail");

    assert_eq!(error.code.as_deref(), Some("suno_session_unauthorized"));
    assert_eq!(error.provider_name.as_deref(), Some("suno_compatible"));
}
