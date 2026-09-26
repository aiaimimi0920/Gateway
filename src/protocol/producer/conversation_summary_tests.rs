use super::summarize_conversation_stream_text;
use serde_json::Value;

#[test]
fn summary_collects_retry_and_text_parts_and_flushes_eof_frame() {
    let summary = summarize_conversation_stream_text(
        [
            "event: part",
            "data: {\"part\":{\"part_kind\":\"retry-prompt\",\"content\":\" retry with a new prompt \"}}",
            "",
            "event: part",
            "data: {\"part\":{\"part_kind\":\"text\",\"content\":\" proposal ready \"}}",
            "",
            "event: suggestion",
            "data: {\"parts\":[{\"part_kind\":\"text\",\"content\":\" choose an action \"},{\"part_kind\":\"tool-call\",\"tool_name\":\"synthetic__suggest_actions\",\"args\":{\"accept\":\" Render now \",\"revise\":\" Adjust style \"}}]}",
        ]
        .join("\n")
        .as_str(),
    )
    .expect("conversation summary");

    assert_eq!(summary["conversation_id"], Value::Null);
    assert_eq!(
        summary["retry_prompts"],
        serde_json::json!(["retry with a new prompt"])
    );
    assert_eq!(
        summary["message_texts"],
        serde_json::json!(["proposal ready", "choose an action"])
    );
    let suggestions = summary["suggestions"]
        .as_array()
        .expect("suggestions array");
    assert_eq!(suggestions.len(), 2);
    assert!(suggestions.iter().any(|value| value == "Render now"));
    assert!(suggestions.iter().any(|value| value == "Adjust style"));
    assert_eq!(summary["final_event_seen"], false);
}

#[test]
fn summary_preserves_custom_error_contract() {
    let error = summarize_conversation_stream_text(
        "event: error\ndata: {\"message\":\" quota wait \"}\n\n",
    )
    .expect_err("error frame must fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_stream_error_event"));
    assert_eq!(error.message, "quota wait");
}

#[test]
fn summary_uses_stable_default_for_non_object_error_data() {
    let error = summarize_conversation_stream_text("event: error\ndata: not-json\n\n")
        .expect_err("non-object error frame must fail");

    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.provider_name.as_deref(), Some("producer_compatible"));
    assert_eq!(error.code.as_deref(), Some("producer_stream_error_event"));
    assert_eq!(
        error.message,
        "Producer.ai conversation stream returned an error event"
    );
}

#[test]
fn summarize_conversation_stream_text_extracts_tools_and_suggestions() {
    let summary = summarize_conversation_stream_text(
        [
            "event: conversation_id",
            "data: {\"id\":\"conv-video-1\"}",
            "",
            "event: part",
            "data: {\"part\":{\"part_kind\":\"tool-call\",\"tool_name\":\"video__propose_music_video\",\"args\":{\"inputs\":{\"start_s\":0}}}}",
            "",
            "event: part",
            "data: {\"part\":{\"part_kind\":\"tool-return\",\"tool_name\":\"video__create_music_video\",\"content\":{\"job_id\":\"mv-job-1\"}}}",
            "",
            "event: suggestion",
            "data: {\"parts\":[{\"part_kind\":\"tool-call\",\"tool_name\":\"synthetic__suggest_actions\",\"args\":{\"action1\":\"Start the render\"}}]}",
            "",
            "event: final",
            "data: {}",
            "",
        ]
        .join("\n")
        .as_str(),
    )
    .unwrap();

    assert_eq!(summary["conversation_id"], "conv-video-1");
    assert_eq!(
        summary["tool_calls"][0]["tool_name"],
        "video__propose_music_video"
    );
    assert_eq!(
        summary["tool_returns"][0]["tool_name"],
        "video__create_music_video"
    );
    assert_eq!(summary["tool_returns"][0]["content"]["job_id"], "mv-job-1");
    assert_eq!(summary["suggestions"][0], "Start the render");
    assert_eq!(summary["final_event_seen"], true);
}
