//! Aggregate limits are tested with many valid small frames over real HTTP.
use super::accumulator_content::AccumulationLimits;
use super::accumulator_tests::accumulate_limited;
use super::event_stream_tests::event_frame;
use serde_json::json;

fn text(value: &str) -> Vec<u8> {
    event_frame("assistantResponseEvent", json!({"content":value}))
}

fn tool(id: &str, name: &str, input: &str) -> Vec<u8> {
    event_frame(
        "toolUseEvent",
        json!({"toolUseId":id,"name":name,"input":input,"stop":false}),
    )
}

async fn rejected(frames: Vec<Vec<u8>>, limits: AccumulationLimits) {
    let error = accumulate_limited(frames.concat(), limits)
        .await
        .unwrap_err();
    assert!(
        error.message.contains("Kiro accumulated response exceeds"),
        "{}",
        error.message
    );
    assert!(!error.message.contains("REJECTED_INPUT"));
}

#[tokio::test]
async fn small_text_frames_share_one_total_limit() {
    rejected(
        vec![text("abc"), text("de")],
        AccumulationLimits {
            bytes: 4,
            ..Default::default()
        },
    )
    .await;
}

#[tokio::test]
async fn repeated_tool_arguments_share_the_total_with_metadata() {
    rejected(
        vec![tool("a", "read", "12"), tool("a", "ignored", "345")],
        AccumulationLimits {
            bytes: 10,
            ..Default::default()
        },
    )
    .await;
}

#[tokio::test]
async fn text_and_tool_arguments_do_not_get_separate_budgets() {
    rejected(
        vec![
            text("abc"),
            tool("a", "read", "12"),
            tool("a", "ignored", "3"),
        ],
        AccumulationLimits {
            bytes: 12,
            ..Default::default()
        },
    )
    .await;
}

#[tokio::test]
async fn unique_tools_are_bounded_even_with_empty_arguments() {
    rejected(
        vec![tool("a", "", ""), tool("b", "", "REJECTED_INPUT")],
        AccumulationLimits {
            tools: 1,
            ..Default::default()
        },
    )
    .await;
}

#[tokio::test]
async fn normalization_and_utf8_metadata_are_included_in_budget() {
    rejected(
        vec![tool("", "", "")],
        AccumulationLimits {
            bytes: 1,
            ..Default::default()
        },
    )
    .await;
    rejected(
        vec![tool("\u{1f980}", "r", "")],
        AccumulationLimits {
            bytes: 14,
            ..Default::default()
        },
    )
    .await;
}

#[tokio::test]
async fn exact_combined_limit_keeps_first_name_and_normalized_arguments() {
    let result = accumulate_limited(
        vec![
            text("abc"),
            tool("a", "read", "1"),
            tool("a", "ignored", "2"),
        ]
        .concat(),
        AccumulationLimits {
            bytes: 12,
            tools: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.text, "abc");
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("read"));
    assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("12"));
    assert_eq!(result.finish_reason.as_deref(), Some("tool_calls"));
}
