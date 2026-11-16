//! Tap actual fragmented SSE bytes; distinguish reported totals from provisional native counters.
use super::*;
use futures::StreamExt;
use serde_json::json;
async fn capture(frames: &[Value]) -> Option<TokenUsage> {
    let bytes: String = frames
        .iter()
        .map(|value| format!("data: {value}\n\n"))
        .collect();
    let chunks: Vec<_> = bytes
        .as_bytes()
        .chunks(3)
        .map(|b| Ok::<_, std::io::Error>(Bytes::copy_from_slice(b)))
        .collect();
    let (stream, usage) = tap_sse_usage_with_error(futures::stream::iter(chunks));
    stream.collect::<Vec<_>>().await;
    snapshot_tapped_usage(&usage)
}
#[tokio::test]
async fn usage_capture_preserves_reported_total_even_zero_and_cache() {
    for total in [0, 17] {
        let usage = capture(&[
            json!({"usage":{"prompt_tokens":9,"completion_tokens":3,"total_tokens":total,
            "prompt_tokens_details":{"cached_tokens":2},"cache_creation_input_tokens":4}}),
        ])
        .await
        .unwrap();
        assert_eq!(
            (
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens
            ),
            (9, 3, total)
        );
        assert_eq!(usage.cache_read_input_tokens, Some(2));
        assert_eq!(usage.cache_creation_input_tokens, Some(4));
    }
}
#[tokio::test]
async fn usage_capture_latest_explicit_zero_snapshot_does_not_keep_old_positive_counts() {
    let usage = capture(&[
        json!({"usage":{"prompt_tokens":9,"completion_tokens":3,"total_tokens":17}}),
        json!({"usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}}),
    ])
    .await
    .unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (0, 0, 0)
    );
}
#[tokio::test]
async fn usage_capture_native_partial_delta_still_merges_input_and_output_without_total() {
    let (stream, latest) = tap_sse_usage_with_error(futures::stream::iter(vec![Ok::<_,std::io::Error>(Bytes::from_static(
        b"event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":0,\"total_tokens\":0,\"cache_read_input_tokens\":2}}}\n\nevent: message_delta\ndata: {\"usage\":{\"output_tokens\":3}}\n\n"))]));
    stream.collect::<Vec<_>>().await;
    let usage = snapshot_tapped_usage(&latest).unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (9, 3, 12)
    );
    assert_eq!(usage.cache_read_input_tokens, Some(2));
}
#[tokio::test]
async fn usage_capture_no_usage_or_only_native_start_remains_unknown() {
    assert!(capture(&[json!({"usage":null}), json!({"choices":[]})])
        .await
        .is_none());
    let (stream, latest) = tap_sse_usage_with_error(futures::stream::iter(vec![Ok::<_,std::io::Error>(Bytes::from_static(
        b"event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":0}}}\n\n"))]));
    stream.collect::<Vec<_>>().await;
    assert!(snapshot_tapped_usage(&latest).is_none());
}
#[tokio::test]
async fn usage_capture_responses_nested_report_preserves_extra_total() {
    let usage = capture(&[
        json!({"response":{"usage":{"input_tokens":9,"output_tokens":3,"total_tokens":17}}}),
    ])
    .await
    .unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (9, 3, 17)
    );
}
