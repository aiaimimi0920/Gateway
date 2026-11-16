//! New retained-state limits must not reduce the old text-only streaming/body allowance.
use super::{fixtures::*, wire};
use neuro_gateway::{
    protocol::{bedrock_converse, stream_error::StreamError},
    upstream::bedrock_converse_official_api_common::accumulate_stream,
};
use serde_json::{json, Value};

fn frames(deltas: Vec<Value>) -> Vec<Vec<u8>> {
    let mut result = vec![wire::event("messageStart", json!({"role":"assistant"}))];
    for delta in deltas {
        result.push(wire::event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"reasoningContent":delta}}),
        ));
    }
    result.extend([
        wire::event("contentBlockStop", json!({"contentBlockIndex":0})),
        wire::event("messageStop", json!({"stopReason":"end_turn"})),
        wire::event(
            "metadata",
            json!({"usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2}}),
        ),
    ]);
    result
}

#[tokio::test]
async fn reasoning_delta_union_type_identity_and_required_text_are_validated() {
    for deltas in [
        vec![json!({"signature":"s"})],
        vec![json!({"text":"x","signature":"s"})],
        vec![json!({"text":1})],
        vec![json!({"redactedContent":"%%%"})],
        vec![json!({"redactedContent":""}), json!({"text":"x"})],
        vec![json!({"signature":"s"}), json!({"redactedContent":""})],
        vec![json!({"text":"x"}), json!({"future":"x"})],
    ] {
        let error = accumulate_stream(wire::response(frames(deltas)), "fixture")
            .await
            .unwrap_err();
        assert!(!error.message.contains("private-signature"));
    }
}

#[tokio::test]
async fn signature_fragments_and_empty_reasoning_match_native_json() {
    for (deltas, expected) in [
        (
            vec![
                json!({"signature":" sig"}),
                json!({"text":""}),
                json!({"signature":"\r\n "}),
            ],
            json!({"reasoningText":{"text":"","signature":" sig\r\n "}}),
        ),
        (
            vec![json!({"text":""})],
            json!({"reasoningText":{"text":""}}),
        ),
        (
            vec![json!({"redactedContent":""})],
            json!({"redactedContent":""}),
        ),
    ] {
        let result = accumulate_stream(wire::response(frames(deltas)), "fixture")
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap()["bedrock_turn"]["content"],
            json!([{"reasoningContent":expected}])
        );
    }
}

#[tokio::test]
async fn reasoning_snapshot_cannot_certify_missing_metadata_truncation_or_late_deltas() {
    let good = frames(vec![json!({"text":"x"})]);
    let mut truncated = good.clone();
    truncated.last_mut().unwrap().pop();
    let mut late = good[..2].to_vec();
    late.push(wire::event(
        "contentBlockStop",
        json!({"contentBlockIndex":0}),
    ));
    late.push(wire::event(
        "contentBlockDelta",
        json!({"contentBlockIndex":0,"delta":{"reasoningContent":{"signature":"s"}}}),
    ));
    for bytes in [good[..good.len() - 1].to_vec(), truncated, late] {
        assert!(accumulate_stream(wire::response(bytes), "fixture")
            .await
            .is_err());
    }
}

#[tokio::test]
async fn ordered_snapshot_retains_empty_tool_input_and_native_block_indices() {
    let mut bytes = frames(vec![json!({"text":""})]);
    bytes.truncate(3);
    bytes.extend([
        wire::event(
            "contentBlockStart",
            json!({"contentBlockIndex":9,"start":{"toolUse":{"toolUseId":"t","name":"f"}}}),
        ),
        wire::event("contentBlockStop", json!({"contentBlockIndex":9})),
        wire::event(
            "contentBlockDelta",
            json!({"contentBlockIndex":4,"delta":{"text":"later-index-first"}}),
        ),
        wire::event("contentBlockStop", json!({"contentBlockIndex":4})),
        wire::event("messageStop", json!({"stopReason":"tool_use"})),
        wire::event(
            "metadata",
            json!({"usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2}}),
        ),
    ]);
    let result = accumulate_stream(wire::response(bytes), "fixture")
        .await
        .unwrap();
    assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{}"));
    let content = serde_json::to_value(result).unwrap()["bedrock_turn"]["content"].clone();
    assert!(content[0].get("reasoningContent").is_some());
    assert_eq!(content[1]["text"], "later-index-first");
    assert_eq!(content[2]["toolUse"]["input"], json!({}));
}

#[tokio::test]
async fn text_only_over_optional_snapshot_limit_still_works_but_late_reasoning_fails() {
    let text = "x".repeat(4 * 1024 * 1024 + 1);
    let mut bytes = vec![
        wire::event("messageStart", json!({"role":"assistant"})),
        wire::event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":text}}),
        ),
        wire::event("contentBlockStop", json!({"contentBlockIndex":0})),
    ];
    let tail = vec![
        wire::event("messageStop", json!({"stopReason":"end_turn"})),
        wire::event(
            "metadata",
            json!({"usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2}}),
        ),
    ];
    let result = accumulate_stream(
        wire::response([bytes.clone(), tail.clone()].concat()),
        "fixture",
    )
    .await
    .unwrap();
    assert_eq!(result.text.len(), text.len());
    assert!(serde_json::to_value(result)
        .unwrap()
        .get("bedrock_turn")
        .is_none());
    bytes.push(wire::event(
        "contentBlockDelta",
        json!({"contentBlockIndex":1,"delta":{"reasoningContent":{"text":""}}}),
    ));
    bytes.push(wire::event(
        "contentBlockStop",
        json!({"contentBlockIndex":1}),
    ));
    bytes.extend(tail);
    assert!(accumulate_stream(wire::response(bytes), "fixture")
        .await
        .is_err());
}

#[tokio::test]
async fn reasoning_limits_cover_accumulated_fragments_and_escaped_serialized_bytes() {
    for deltas in [
        vec![
            json!({"text":"x".repeat(2*1024*1024)}),
            json!({"signature":"s".repeat(2*1024*1024)}),
        ],
        vec![json!({"text":"\n".repeat(2*1024*1024)})],
        vec![json!({"text":"x".repeat(4*1024*1024+1)})],
    ] {
        assert!(accumulate_stream(wire::response(frames(deltas)), "fixture")
            .await
            .is_err());
    }
}

#[test]
fn opaque_turn_does_not_enable_text_tool_injection_or_other_vendor_state() {
    let text = "<tool_calls>[{\"name\":\"not_executable\",\"arguments\":{}}]</tool_calls>";
    let mut body = neuro_gateway::protocol::openai::build_chat_completions_success(
        "t",
        0,
        "m",
        text,
        None,
        &[],
        Some("stop"),
    );
    body["gateway_bedrock_turn"] =
        json!({"content":[{"text":text},{"reasoningContent":{"reasoningText":{"text":"r"}}}]});
    let result = neuro_gateway::protocol::openai::unpack_openai_response(&body).unwrap();
    assert_eq!(result.text, text);
    assert!(result.tool_calls.is_empty());
    body["gateway_vertex_turn"] = json!({});
    assert!(neuro_gateway::protocol::openai::unpack_openai_response(&body).is_err());
}

#[test]
fn public_tool_projection_must_match_without_html_entity_repair() {
    for protocol in 0..3 {
        let (mut body, read) = public_body(protocol);
        body["gateway_bedrock_turn"] = json!({"content":content()});
        body["gateway_bedrock_turn"]["content"][2]["toolUse"]["input"]["v"] = json!("\"");
        assert!(read(&body).is_err());
    }
}

#[tokio::test]
async fn malformed_realtime_reasoning_fails_and_drops_source_before_error() {
    use futures::{Stream, StreamExt};
    use std::{
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context, Poll},
    };
    type Error = StreamError<std::io::Error>;
    struct Source(Option<bytes::Bytes>, Arc<AtomicBool>);
    impl Stream for Source {
        type Item = Result<bytes::Bytes, Error>;
        fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.0
                .take()
                .map_or(Poll::Pending, |bytes| Poll::Ready(Some(Ok(bytes))))
        }
    }
    impl Drop for Source {
        fn drop(&mut self) {
            self.1.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let mut frames = sequence();
    frames[3] = super::wire::event(
        "contentBlockDelta",
        json!({"contentBlockIndex":1,
        "delta":{"reasoningContent":{"text":"private-reasoning","signature":"private-signature"}}}),
    );
    let mut stream = Box::pin(
        bedrock_converse::translate_bedrock_eventstream_to_openai_with_error(
            Source(Some(bytes::Bytes::from(frames.concat())), dropped.clone()),
            "fixture".into(),
        ),
    );
    let mut errors = 0;
    while let Some(value) = stream.next().await {
        match value {
            Ok(bytes) => {
                let text = std::str::from_utf8(&bytes).unwrap();
                assert!(!text.contains("[DONE]"));
                assert!(!text.contains("\"finish_reason\":\""));
            }
            Err(_) => {
                errors += 1;
                assert!(dropped.load(Ordering::SeqCst));
            }
        }
    }
    assert_eq!(errors, 1);
}
