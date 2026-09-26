use serde_json::{json, Value};

use super::{parse_tool_call_stream_text, PRODUCER_VIDEO_DEFAULT_MODEL};

const TOOL_NAME: &str = "video__create_music_video";

fn parse(raw_text: &str, stream_job_id: &str) -> Value {
    parse_tool_call_stream_text(
        raw_text,
        PRODUCER_VIDEO_DEFAULT_MODEL,
        stream_job_id,
        TOOL_NAME,
    )
    .expect("tool-call stream should parse")
}

fn event(name: &str, data: Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

#[test]
fn unterminated_final_frame_preserves_output_shape_and_event_order() {
    let raw_text = concat!(
        "event: conversation_id\n",
        "data: {\"id\":\"conv/video 1\"}\n\n",
        "data: not-json\n\n",
        "event: message\n",
        "data: {\"sequence\":1,\"parts\":[]}\n\n",
        "event: message\n",
        "data: {\"sequence\":2,\"parts\":[]}\n\n",
        "event: final\n",
        "data: {}"
    );

    let parsed = parse(raw_text, "stream/job ?");

    assert_eq!(parsed["object"], "video.generation");
    assert_eq!(parsed["provider"], "producer.ai");
    assert_eq!(parsed["model"], PRODUCER_VIDEO_DEFAULT_MODEL);
    assert_eq!(parsed["tool_name"], TOOL_NAME);
    assert_eq!(parsed["job_id"], "stream/job ?");
    assert_eq!(parsed["stream_job_id"], "stream/job ?");
    assert_eq!(parsed["tool_return_job_id"], Value::Null);
    assert_eq!(parsed["conversation_id"], "conv/video 1");
    assert_eq!(
        parsed["progress_url"],
        "https://www.producer.ai/session/conv%2Fvideo%201"
    );
    assert_eq!(
        parsed["status_path"],
        "/__api/music-video/stream%2Fjob%20%3F/status"
    );
    assert_eq!(
        parsed["detail_path"],
        "/__api/music-video/get/stream%2Fjob%20%3F"
    );
    assert_eq!(parsed["library_path"], "/__api/music-videos/get");
    assert_eq!(parsed["messages"][0]["sequence"], 1);
    assert_eq!(parsed["messages"][1]["sequence"], 2);
    assert_eq!(parsed["events"][0]["event"], "conversation_id");
    assert_eq!(parsed["events"][1]["event"], Value::Null);
    assert_eq!(parsed["events"][1]["data"], "not-json");
    assert_eq!(parsed["events"][4]["event"], "final");
    assert_eq!(parsed["final_event_seen"], true);
    assert!(parsed.get("completed").is_none());
    assert!(parsed.get("status").is_none());
}

#[test]
fn tool_returns_filter_parts_and_keep_last_valid_job_and_url_order() {
    let first_message = json!({
        "sequence": 1,
        "parts": [
            {"part_kind": "text", "content": {}},
            {"part_kind": "tool-return", "tool_name": "other", "content": {"jobId": "ignored"}},
            {"part_kind": "tool-return", "tool_name": TOOL_NAME, "content": "not-an-object"},
            {"part_kind": "tool-return", "content": {
                "jobId": "job-camel-1",
                "final_video_url": "https://cdn.example/one.mp4"
            }},
            {"part_kind": "tool-return", "tool_name": format!(" {TOOL_NAME} "), "content": {
                "job_id": "job-snake-2",
                "jobId": "job-camel-hidden",
                "url": "https://cdn.example/two.mp4",
                "final_video_url": "https://cdn.example/two-fallback.mp4"
            }}
        ]
    });
    let second_message = json!({
        "sequence": 2,
        "parts": [
            {"part_kind": "tool-return", "tool_name": TOOL_NAME, "content": {
                "job_id": " ",
                "jobId": "blocked-camel",
                "url": " ",
                "final_video_url": "https://cdn.example/blocked.mp4"
            }},
            {"part_kind": "tool-return", "tool_name": TOOL_NAME, "content": {
                "jobId": "job-camel-last",
                "final_video_url": "https://cdn.example/last.mp4"
            }}
        ]
    });
    let raw_text = format!(
        "{}{}",
        event("message", first_message),
        event("message", second_message)
    );

    let parsed = parse(&raw_text, "stream-job");

    assert_eq!(parsed["job_id"], "job-camel-last");
    assert_eq!(parsed["tool_return_job_id"], "job-camel-last");
    assert_eq!(parsed["messages"][0]["sequence"], 1);
    assert_eq!(parsed["messages"][1]["sequence"], 2);
    assert_eq!(parsed["tool_returns"].as_array().unwrap().len(), 4);
    assert_eq!(parsed["tool_returns"][0]["jobId"], "job-camel-1");
    assert_eq!(parsed["tool_returns"][1]["job_id"], "job-snake-2");
    assert_eq!(parsed["tool_returns"][2]["jobId"], "blocked-camel");
    assert_eq!(parsed["tool_returns"][3]["jobId"], "job-camel-last");
    assert_eq!(
        parsed["video_urls"],
        json!([
            "https://cdn.example/one.mp4",
            "https://cdn.example/two.mp4",
            "https://cdn.example/last.mp4"
        ])
    );
}

#[test]
fn blank_primary_aliases_block_camel_job_and_fallback_url() {
    let message = json!({
        "parts": [{
            "part_kind": "tool-return",
            "tool_name": TOOL_NAME,
            "content": {
                "job_id": " ",
                "jobId": "blocked-camel",
                "url": " ",
                "final_video_url": "https://cdn.example/blocked.mp4"
            }
        }]
    });

    let parsed = parse(&event("message", message), "stream-fallback");

    assert_eq!(parsed["job_id"], "stream-fallback");
    assert_eq!(parsed["tool_return_job_id"], Value::Null);
    assert_eq!(parsed["video_urls"], json!([]));
    assert_eq!(parsed["tool_returns"].as_array().unwrap().len(), 1);
}

#[test]
fn error_frames_preserve_custom_and_default_error_contracts() {
    let custom = parse_tool_call_stream_text(
        "event: error\ndata: {\"message\":\" render failed \"}\n\n",
        PRODUCER_VIDEO_DEFAULT_MODEL,
        "stream-job",
        TOOL_NAME,
    )
    .expect_err("error event should fail");
    assert_eq!(custom.http_status, Some(500));
    assert_eq!(custom.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(
        custom.code.as_deref(),
        Some("producer_tool_call_stream_error_event")
    );
    assert_eq!(custom.message, "render failed");

    let fallback = parse_tool_call_stream_text(
        "event: error\ndata: not-json",
        PRODUCER_VIDEO_DEFAULT_MODEL,
        "stream-job",
        TOOL_NAME,
    )
    .expect_err("malformed error event should fail");
    assert_eq!(fallback.http_status, Some(500));
    assert_eq!(
        fallback.provider_name.as_deref(),
        Some("producer_compatible")
    );
    assert_eq!(
        fallback.code.as_deref(),
        Some("producer_tool_call_stream_error_event")
    );
    assert_eq!(
        fallback.message,
        "Producer.ai tool-call stream returned an error event"
    );
}

#[test]
fn parse_tool_call_stream_text_extracts_music_video_job_id() {
    let parsed = parse_tool_call_stream_text(
        [
            "event: conversation_id",
            "data: {\"id\":\"conv-video-1\"}",
            "",
            "event: message",
            "data: {\"kind\":\"request\",\"parts\":[{\"part_kind\":\"tool-return\",\"tool_name\":\"video__create_music_video\",\"content\":{\"job_id\":\"mv-job-77\"}}]}",
            "",
            "event: final",
            "data: {}",
            "",
        ]
        .join("\n")
        .as_str(),
        PRODUCER_VIDEO_DEFAULT_MODEL,
        "stream-job-1",
        "video__create_music_video",
    )
    .unwrap();

    assert_eq!(parsed["object"], "video.generation");
    assert_eq!(parsed["job_id"], "mv-job-77");
    assert_eq!(parsed["stream_job_id"], "stream-job-1");
    assert_eq!(parsed["conversation_id"], "conv-video-1");
    assert_eq!(parsed["status_path"], "/__api/music-video/mv-job-77/status");
}
