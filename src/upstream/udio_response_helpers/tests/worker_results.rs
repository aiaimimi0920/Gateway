use super::*;

#[test]
fn parse_udio_remote_browser_worker_success_reads_result_contract() {
    let parsed = parse_udio_remote_browser_worker_success(serde_json::json!({
        "trackIds": ["track-1"],
        "songs": [],
        "completed": true
    }))
    .expect("udio remote result");

    assert_eq!(parsed.track_ids, vec!["track-1"]);
    assert!(parsed.completed);
}

#[test]
fn parse_udio_remote_browser_worker_success_rejects_invalid_contract() {
    let error = parse_udio_remote_browser_worker_success(serde_json::json!({
        "trackIds": [],
        "completed": "yes"
    }))
    .expect_err("invalid remote result should fail");

    assert_eq!(
        error.code.as_deref(),
        Some("udio_remote_result_parse_failed")
    );
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn parse_udio_remote_browser_worker_verified_result_reads_track_ids_contract() {
    let worker = parse_udio_remote_browser_worker_verified_result(serde_json::json!({
        "trackIds": ["track-1"],
        "songs": [],
        "completed": true,
        "message": "done"
    }))
    .expect("verified remote result");

    assert_eq!(worker.track_ids, vec!["track-1"]);
    assert!(worker.completed);
    assert_eq!(worker.message.as_deref(), Some("done"));
}

#[test]
fn parse_udio_remote_browser_worker_verified_result_rejects_empty_track_ids_contract() {
    let error = parse_udio_remote_browser_worker_verified_result(serde_json::json!({
        "trackIds": [],
        "songs": [],
        "completed": false
    }))
    .expect_err("empty track ids should fail");

    assert_eq!(error.code.as_deref(), Some("udio_missing_track_ids"));
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn parse_udio_browser_worker_output_reads_result_contract() {
    let parsed = parse_udio_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true}}",
        "",
    )
    .expect("udio worker output");

    assert!(parsed.ok);
    assert_eq!(parsed.status, Some(200));
    let result = parsed.result.expect("worker result");
    assert_eq!(result.track_ids, vec!["track-1"]);
    assert!(result.completed);
}

#[test]
fn parse_udio_browser_worker_output_rejects_empty_stdout_contract() {
    let error = parse_udio_browser_worker_output("", "permission denied")
        .expect_err("empty stdout should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("udio_browser_worker_empty_output")
    );
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn extract_udio_browser_worker_success_reads_track_ids() {
    let result = parse_udio_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true}}",
        "",
    )
    .expect("udio worker output");

    let payload = extract_udio_browser_worker_success(result).expect("udio worker success");
    assert_eq!(payload.track_ids, vec!["track-1"]);
    assert!(payload.completed);
}

#[test]
fn extract_udio_browser_worker_success_rejects_empty_track_ids_contract() {
    let result = parse_udio_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[],\"songs\":[],\"completed\":true}}",
        "",
    )
    .expect("udio worker output");

    let error =
        extract_udio_browser_worker_success(result).expect_err("missing track ids should fail");
    assert_eq!(error.code.as_deref(), Some("udio_missing_track_ids"));
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
}

#[test]
fn classify_udio_browser_worker_failure_prefers_worker_error_contract() {
    let result = parse_udio_browser_worker_output(
        "{\"ok\":false,\"status\":400,\"error\":{\"code\":\"udio_worker_blocked\",\"message\":\"challenge required\",\"status\":422,\"body\":\"{\\\"detail\\\":\\\"Unauthorized\\\"}\"}}",
        "",
    )
    .expect("udio worker output");

    let error = classify_udio_browser_worker_failure(result, "", "udio_compatible");
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(error.code.as_deref(), Some("udio_worker_blocked"));
    assert_eq!(error.message, "challenge required");
    assert_eq!(error.http_status, Some(422));
}

#[test]
fn classify_udio_browser_worker_failure_uses_stderr_when_body_missing() {
    let result = parse_udio_browser_worker_output("{\"ok\":false,\"status\":429}", "")
        .expect("udio worker output");

    let error =
        classify_udio_browser_worker_failure(result, "permission denied", "udio_compatible");
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(error.http_status, Some(429));
    assert!(error.message.contains("permission denied"));
}

#[test]
fn resolve_udio_browser_worker_result_reads_success_contract() {
    let result = parse_udio_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true,\"message\":\"done\"}}",
        "",
    )
    .expect("udio worker output");

    let payload = resolve_udio_browser_worker_result(result, "", "udio_compatible")
        .expect("ok worker result should succeed");

    assert_eq!(payload.track_ids, vec!["track-1"]);
    assert!(payload.completed);
    assert_eq!(payload.message.as_deref(), Some("done"));
}

#[test]
fn resolve_udio_browser_worker_result_preserves_failure_contract() {
    let result = parse_udio_browser_worker_output(
        "{\"ok\":false,\"status\":429,\"error\":{\"code\":\"udio_rate_limited\",\"message\":\"too many requests\",\"status\":429}}",
        "",
    )
    .expect("udio worker output");

    let error = resolve_udio_browser_worker_result(result, "", "udio_compatible")
        .expect_err("failed worker result should error");

    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(error.code.as_deref(), Some("udio_rate_limited"));
    assert_eq!(error.message, "too many requests");
    assert_eq!(error.http_status, Some(429));
}

#[test]
fn parse_udio_browser_worker_verified_output_reads_success_contract() {
    let payload = parse_udio_browser_worker_verified_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"trackIds\":[\"track-1\"],\"songs\":[],\"completed\":true,\"message\":\"done\"}}",
        "",
        "udio_compatible",
    )
    .expect("verified worker output");

    assert_eq!(payload.track_ids, vec!["track-1"]);
    assert!(payload.completed);
    assert_eq!(payload.message.as_deref(), Some("done"));
}

#[test]
fn parse_udio_browser_worker_verified_output_preserves_failure_contract() {
    let error = parse_udio_browser_worker_verified_output(
        "{\"ok\":false,\"status\":429,\"error\":{\"code\":\"udio_rate_limited\",\"message\":\"too many requests\",\"status\":429}}",
        "",
        "udio_compatible",
    )
    .expect_err("failed worker output should error");

    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert_eq!(error.code.as_deref(), Some("udio_rate_limited"));
    assert_eq!(error.message, "too many requests");
    assert_eq!(error.http_status, Some(429));
}

#[test]
fn build_udio_browser_executor_service_result_preserves_track_and_song_contract() {
    let result = parse_udio_remote_browser_worker_success(serde_json::json!({
        "trackIds": ["track-1"],
        "songs": [{
            "audio_url": "https://cdn.example.com/song.mp3"
        }],
        "completed": true,
        "message": "done"
    }))
    .expect("udio remote result");

    let body = build_udio_browser_executor_service_result(&result);
    assert_eq!(body["trackIds"][0], "track-1");
    assert_eq!(
        body["songs"][0]["audio_url"],
        "https://cdn.example.com/song.mp3"
    );
    assert_eq!(body["completed"], true);
    assert_eq!(body["message"], "done");
}

#[test]
fn build_udio_browser_executor_service_result_preserves_null_message_contract() {
    let result = parse_udio_remote_browser_worker_success(serde_json::json!({
        "trackIds": ["track-1"],
        "songs": [],
        "completed": false,
        "message": null
    }))
    .expect("udio remote result");

    let body = build_udio_browser_executor_service_result(&result);
    assert_eq!(body["trackIds"][0], "track-1");
    assert_eq!(body["completed"], false);
    assert!(body["message"].is_null());
}

#[test]
fn resolve_udio_worker_latest_songs_falls_back_to_pending_track_ids_contract() {
    let worker = parse_udio_remote_browser_worker_success(serde_json::json!({
        "trackIds": ["track-1", "track-2"],
        "songs": [],
        "completed": false
    }))
    .expect("udio remote result");

    let songs = resolve_udio_worker_latest_songs(&worker).expect("pending songs");
    assert_eq!(songs.len(), 2);
    assert_eq!(songs[0].id, "track-1");
    assert_eq!(songs[0].status, "pending");
    assert!(!songs[0].finished);
}

#[test]
fn resolve_udio_worker_latest_songs_uses_worker_song_feed_contract() {
    let worker = parse_udio_remote_browser_worker_success(serde_json::json!({
        "trackIds": ["track-1"],
        "songs": [{
            "id": "track-1",
            "image_url": "https://cdn.example.com/image.png",
            "song_path": "https://cdn.example.com/song.mp3",
            "finished": true,
            "readyToStream": true
        }],
        "completed": true
    }))
    .expect("udio remote result");

    let songs = resolve_udio_worker_latest_songs(&worker).expect("worker songs");
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].id, "track-1");
    assert_eq!(
        songs[0].audio_url.as_deref(),
        Some("https://cdn.example.com/song.mp3")
    );
    assert!(songs[0].finished);
}
