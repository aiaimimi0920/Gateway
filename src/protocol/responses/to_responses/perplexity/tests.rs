//! Product snapshots cannot turn a truncated carrier into a successful standard response.
use super::*;
use bytes::Bytes;
use futures::StreamExt;

fn snapshot(sequence: u64) -> String {
    format!(
        "data: {}\n\n",
        serde_json::json!({public_snapshot::KEY:{
        "version":1,"sequence":sequence,"answer":{"text":"临时","sources":[]}},"choices":[]})
    )
}
async fn translate(input: String) -> (Vec<Value>, bool) {
    let input = futures::stream::iter([Ok::<_, ProtocolStreamError>(Bytes::from(input))]);
    let output = translate_openai_sse_to_responses_with_error(input, "public-alias".into());
    futures::pin_mut!(output);
    let mut events = Vec::new();
    let mut failed = false;
    while let Some(result) = output.next().await {
        match result {
            Ok(bytes) => {
                for line in std::str::from_utf8(&bytes).unwrap().lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(value) = serde_json::from_str(data) {
                            events.push(value);
                        }
                    }
                }
            }
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    (events, failed)
}
#[tokio::test]
async fn perplexity_web_responses_snapshot_only_eof_or_done_never_fabricates_success() {
    for suffix in ["", "data: [DONE]\n\n"] {
        let (events, failed) = translate(format!("{}{suffix}", snapshot(1))).await;
        assert!(failed);
        assert!(events.iter().any(|v| v["type"] == public_snapshot::EVENT));
        assert!(!events.iter().any(|v| v["type"] == "response.completed"));
    }
}
#[tokio::test]
async fn perplexity_web_responses_snapshot_rejects_reordering_and_after_standard_content() {
    let text = "data: {\"choices\":[{\"delta\":{\"content\":\"standard\"}}]}\n\n";
    let tool = "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"tool\",\"arguments\":\"\"}}]}}]}\n\n";
    for input in [
        snapshot(2),
        format!("{}{}", snapshot(1), snapshot(1)),
        format!("{text}{}", snapshot(1)),
        format!("{tool}{}", snapshot(1)),
    ] {
        let (events, failed) = translate(input).await;
        assert!(failed);
        assert!(!events.iter().any(|v| v["type"] == "response.completed"));
    }
}
