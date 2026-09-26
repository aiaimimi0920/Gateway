use super::*;

#[test]
fn parse_suno_remote_browser_worker_success_reads_result_contract() {
    let parsed = parse_suno_remote_browser_worker_success(json!({
        "clips": [{
            "id": "clip-1",
            "audio_url": "https://cdn.example.com/song.mp3",
            "status": "complete"
        }],
        "completed": true,
        "message": "done"
    }))
    .expect("remote suno worker result");

    assert_eq!(parsed.clips.len(), 1);
    assert!(parsed.completed);
    assert_eq!(parsed.message.as_deref(), Some("done"));
}

#[test]
fn parse_suno_remote_browser_worker_success_rejects_invalid_contract() {
    let err = parse_suno_remote_browser_worker_success(json!({
        "clips": [],
        "completed": "yes"
    }))
    .expect_err("invalid remote result should fail");
    assert_eq!(err.code.as_deref(), Some("suno_remote_result_parse_failed"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_remote_browser_worker_verified_result_reads_nonempty_clips_contract() {
    let parsed = parse_suno_remote_browser_worker_verified_result(json!({
        "clips": [{
            "id": "clip-1",
            "audio_url": "https://cdn.example.com/song.mp3",
            "status": "complete"
        }],
        "completed": true,
        "message": "done"
    }))
    .expect("verified remote suno worker result");

    assert_eq!(parsed.clips.len(), 1);
    assert!(parsed.completed);
    assert_eq!(parsed.message.as_deref(), Some("done"));
}

#[test]
fn parse_suno_remote_browser_worker_verified_result_rejects_empty_clips_contract() {
    let err = parse_suno_remote_browser_worker_verified_result(json!({
        "clips": [],
        "completed": false,
        "message": null
    }))
    .expect_err("empty clips should fail");

    assert_eq!(err.code.as_deref(), Some("suno_missing_feed_clips"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn parse_suno_browser_worker_output_reads_result_contract() {
    let parsed = parse_suno_browser_worker_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"clips\":[],\"completed\":true,\"message\":\"done\"}}",
        "",
    )
    .expect("suno worker output");

    assert!(parsed.ok);
    assert_eq!(parsed.status, Some(200));
    assert!(parsed.result.is_some());
}

#[test]
fn extract_suno_browser_worker_success_reads_nonempty_clips() {
    let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
        "ok": true,
        "status": 200,
        "result": {
            "clips": [{
                "id": "clip-1",
                "audio_url": "https://cdn.example.com/song.mp3",
                "status": "complete"
            }],
            "completed": true,
            "message": "done"
        }
    }))
    .expect("suno worker result");

    let success = extract_suno_browser_worker_success(result).expect("suno worker success payload");
    assert_eq!(success.clips.len(), 1);
    assert!(success.completed);
}

#[test]
fn extract_suno_browser_worker_success_rejects_empty_clips_contract() {
    let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
        "ok": true,
        "status": 200,
        "result": {
            "clips": [],
            "completed": true,
            "message": "done"
        }
    }))
    .expect("suno worker result");

    let err = extract_suno_browser_worker_success(result).expect_err("empty clips should fail");
    assert_eq!(err.code.as_deref(), Some("suno_missing_feed_clips"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
}

#[test]
fn classify_suno_browser_worker_failure_prefers_worker_error_contract() {
    let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
        "ok": false,
        "status": 400,
        "error": {
            "code": "suno_worker_blocked",
            "message": "challenge required",
            "status": 422,
            "body": "{\"detail\":\"Unauthorized\"}"
        }
    }))
    .expect("suno worker result");

    let err = classify_suno_browser_worker_failure(result, "suno_compatible");
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.code.as_deref(), Some("suno_worker_blocked"));
    assert_eq!(err.message, "challenge required");
    assert_eq!(err.http_status, Some(422));
}

#[test]
fn classify_suno_browser_worker_failure_uses_top_level_status_when_error_missing() {
    let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
        "ok": false,
        "status": 429
    }))
    .expect("suno worker result");

    let err = classify_suno_browser_worker_failure(result, "suno_compatible");
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.http_status, Some(429));
}

#[test]
fn resolve_suno_browser_worker_result_reads_success_contract() {
    let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
        "ok": true,
        "result": {
            "clips": [{
                "id": "clip-1",
                "status": "complete"
            }],
            "completed": true,
            "message": "done"
        }
    }))
    .expect("suno worker result");

    let success = resolve_suno_browser_worker_result(result, "suno_compatible")
        .expect("ok worker result should succeed");

    assert_eq!(success.clips.len(), 1);
    assert_eq!(success.message.as_deref(), Some("done"));
}

#[test]
fn resolve_suno_browser_worker_result_preserves_failure_contract() {
    let result: SunoBrowserWorkerResult = serde_json::from_value(json!({
        "ok": false,
        "status": 429,
        "error": {
            "status": 429,
            "code": "suno_rate_limited",
            "message": "too many requests",
            "body": "upstream body"
        }
    }))
    .expect("suno worker result");

    let err = resolve_suno_browser_worker_result(result, "suno_compatible")
        .expect_err("failed worker result should error");

    assert_eq!(err.code.as_deref(), Some("suno_rate_limited"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.http_status, Some(429));
}

#[test]
fn parse_suno_browser_worker_verified_output_reads_success_contract() {
    let success = parse_suno_browser_worker_verified_output(
        "{\"ok\":true,\"status\":200,\"result\":{\"clips\":[{\"id\":\"clip-1\",\"status\":\"complete\"}],\"completed\":true,\"message\":\"done\"}}",
        "",
        "suno_compatible",
    )
    .expect("verified worker output");

    assert_eq!(success.clips.len(), 1);
    assert!(success.completed);
    assert_eq!(success.message.as_deref(), Some("done"));
}

#[test]
fn parse_suno_browser_worker_verified_output_preserves_failure_contract() {
    let err = parse_suno_browser_worker_verified_output(
        "{\"ok\":false,\"status\":429,\"error\":{\"status\":429,\"code\":\"suno_rate_limited\",\"message\":\"too many requests\",\"body\":\"upstream body\"}}",
        "",
        "suno_compatible",
    )
    .expect_err("failed worker output should error");

    assert_eq!(err.code.as_deref(), Some("suno_rate_limited"));
    assert_eq!(err.provider_name.as_deref(), Some("suno_compatible"));
    assert_eq!(err.http_status, Some(429));
}

#[test]
fn build_suno_browser_executor_service_result_preserves_clips_contract() {
    let result = parse_suno_remote_browser_worker_success(json!({
        "clips": [{
            "id": "clip-1",
            "audio_url": "https://cdn.example.com/song.mp3",
            "status": "complete"
        }],
        "completed": true,
        "message": "done"
    }))
    .expect("remote suno worker result");

    let body = build_suno_browser_executor_service_result(&result);
    assert_eq!(body["clips"][0]["id"], "clip-1");
    assert_eq!(
        body["clips"][0]["audio_url"],
        "https://cdn.example.com/song.mp3"
    );
    assert_eq!(body["completed"], true);
    assert_eq!(body["message"], "done");
}

#[test]
fn build_suno_browser_executor_service_result_preserves_null_message_contract() {
    let result = parse_suno_remote_browser_worker_success(json!({
        "clips": [{
            "id": "clip-1",
            "audio_url": "https://cdn.example.com/song.mp3",
            "status": "complete"
        }],
        "completed": false,
        "message": null
    }))
    .expect("remote suno worker result");

    let body = build_suno_browser_executor_service_result(&result);
    assert_eq!(body["clips"][0]["id"], "clip-1");
    assert_eq!(body["completed"], false);
    assert!(body["message"].is_null());
}
