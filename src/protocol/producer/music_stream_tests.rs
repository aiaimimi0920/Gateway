use super::{
    extract_job_id, extract_stream_job_id, parse_producer_music_stream_text, PRODUCER_DEFAULT_MODEL,
};
use serde_json::json;

#[test]
fn job_id_precedence_and_error_contracts_are_stable() {
    assert_eq!(
        extract_job_id(&json!({ "job_id": " snake ", "jobId": "camel" })).unwrap(),
        "snake"
    );

    let missing = extract_job_id(&json!({ "job_id": " ", "jobId": "fallback" }))
        .expect_err("an empty preferred job_id does not fall back to jobId");
    assert_eq!(missing.http_status, Some(500));
    assert_eq!(
        missing.provider_name.as_deref(),
        Some("producer_compatible")
    );
    assert_eq!(missing.code.as_deref(), Some("producer_missing_job_id"));
    assert_eq!(
        missing.message,
        "Producer.ai send-message response missing job_id"
    );

    let stream_missing = extract_stream_job_id(&json!({}))
        .expect_err("stream job extraction should retain its specific code");
    assert_eq!(stream_missing.http_status, Some(500));
    assert_eq!(
        stream_missing.provider_name.as_deref(),
        Some("producer_compatible")
    );
    assert_eq!(
        stream_missing.code.as_deref(),
        Some("producer_missing_stream_job_id")
    );
    assert_eq!(stream_missing.message, missing.message);
}

#[test]
fn music_stream_flushes_unterminated_frame_and_preserves_event_order() {
    let parsed = parse_producer_music_stream_text(
        [
            "event: conversation_id\ndata: {\"id\":\"first\"}",
            "event: conversation_id\ndata: {\"id\":\"second\"}",
            "event: generated-title\ndata: {\"title\":\"First title\"}",
            "event: generated-title\ndata: {\"title\":\"Final title\"}",
            "event: complete\ndata: {}",
        ]
        .join("\n\n")
        .as_str(),
        PRODUCER_DEFAULT_MODEL,
        "music-job-eof",
    )
    .expect("unterminated final frame should be flushed");

    let event_names = parsed["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        event_names,
        [
            "conversation_id",
            "conversation_id",
            "generated-title",
            "generated-title",
            "complete"
        ]
    );
    assert_eq!(parsed["conversation_id"], "second");
    assert_eq!(parsed["generated_title"], "Final title");
    assert_eq!(parsed["completed"], true);
    assert_eq!(parsed["final_event_seen"], false);
}

#[test]
fn music_stream_without_terminal_events_remains_pending() {
    let parsed = parse_producer_music_stream_text(
        "event: suggestion\ndata: {\"label\":\"keep waiting\"}\n\n",
        PRODUCER_DEFAULT_MODEL,
        "music-job-pending",
    )
    .expect("a pending stream should still produce a summary");

    assert_eq!(parsed["completed"], false);
    assert_eq!(parsed["final_event_seen"], false);
    assert_eq!(parsed["suggestions"][0]["label"], "keep waiting");
    assert_eq!(parsed["events"][0]["event"], "suggestion");
}

#[test]
fn music_stream_error_contract_preserves_custom_and_default_messages() {
    let custom = parse_producer_music_stream_text(
        "event: error\ndata: {\"message\":\" captcha required \"}",
        PRODUCER_DEFAULT_MODEL,
        "music-job-custom-error",
    )
    .expect_err("custom error event should fail");
    assert_eq!(custom.http_status, Some(500));
    assert_eq!(custom.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(custom.code.as_deref(), Some("producer_stream_error_event"));
    assert_eq!(custom.message, "captcha required");

    let fallback = parse_producer_music_stream_text(
        "event: error\ndata: not-json",
        PRODUCER_DEFAULT_MODEL,
        "music-job-default-error",
    )
    .expect_err("non-object error data should use the fallback message");
    assert_eq!(fallback.http_status, Some(500));
    assert_eq!(
        fallback.provider_name.as_deref(),
        Some("producer_compatible")
    );
    assert_eq!(
        fallback.code.as_deref(),
        Some("producer_stream_error_event")
    );
    assert_eq!(
        fallback.message,
        "Producer.ai stream returned an error event"
    );
}

#[test]
fn extract_job_id_reads_camel_or_snake_case() {
    assert_eq!(
        extract_job_id(&json!({ "job_id": "job_1" })).unwrap(),
        "job_1"
    );
    assert_eq!(
        extract_job_id(&json!({ "jobId": "job_2" })).unwrap(),
        "job_2"
    );
}

#[test]
fn parse_producer_music_stream_text_extracts_completed_contract() {
    let parsed = parse_producer_music_stream_text(
        [
            "event: conversation_id",
            "data: {\"id\":\"conv-music-1\"}",
            "",
            "event: part",
            "data: {\"kind\":\"lyric\",\"text\":\"first line\"}",
            "",
            "event: suggestion",
            "data: {\"label\":\"save to favorites\"}",
            "",
            "event: generated-title",
            "data: {\"title\":\"Neon Dreams\"}",
            "",
            "event: complete",
            "data: {}",
            "",
            "event: final",
            "data: {}",
            "",
        ]
        .join("\n")
        .as_str(),
        PRODUCER_DEFAULT_MODEL,
        "music-job-1",
    )
    .expect("music stream should parse");

    assert_eq!(parsed["object"], "music.generation");
    assert_eq!(parsed["provider"], "producer.ai");
    assert_eq!(parsed["model"], PRODUCER_DEFAULT_MODEL);
    assert_eq!(parsed["job_id"], "music-job-1");
    assert_eq!(parsed["conversation_id"], "conv-music-1");
    assert_eq!(parsed["generated_title"], "Neon Dreams");
    assert_eq!(parsed["completed"], true);
    assert_eq!(parsed["final_event_seen"], true);
    assert_eq!(parsed["parts"][0]["text"], "first line");
    assert_eq!(parsed["suggestions"][0]["label"], "save to favorites");
}

#[test]
fn parse_producer_music_stream_text_rejects_error_event_contract() {
    let error = parse_producer_music_stream_text(
        [
            "event: error",
            "data: {\"message\":\"captcha required\"}",
            "",
        ]
        .join("\n")
        .as_str(),
        PRODUCER_DEFAULT_MODEL,
        "music-job-2",
    )
    .expect_err("error event should fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_stream_error_event"));
    assert_eq!(error.message, "captcha required");
}
