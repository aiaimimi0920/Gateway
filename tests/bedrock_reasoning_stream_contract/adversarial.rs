//! Mutated carriers, missing witnesses and pending EOF never produce a public success terminal.
use super::*;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

fn convert(source: Source, protocol: u8) -> Source {
    if protocol == 3 {
        Box::pin(
            bedrock_converse::translate_openai_sse_to_bedrock_eventstream_with_error(
                source,
                "model".into(),
            ),
        )
    } else {
        public(source, protocol)
    }
}
fn encoded(frames: &[Value], done: bool) -> Vec<u8> {
    let mut wire = frames
        .iter()
        .map(|v| format!("data: {v}\n\n"))
        .collect::<String>();
    if done {
        wire.push_str("data: [DONE]\n\n");
    }
    wire.into_bytes()
}
fn terminal(bytes: &Bytes, protocol: u8) -> bool {
    if protocol == 3 {
        return matches!(
            native_wire::decode(bytes).0.as_str(),
            "messageStop" | "metadata"
        );
    }
    if protocol == 0 {
        return std::str::from_utf8(bytes).unwrap().contains("[DONE]")
            || values(bytes)
                .iter()
                .any(|v| v["choices"][0]["finish_reason"].as_str().is_some());
    }
    values(bytes).iter().any(|v| {
        matches!(
            v["type"].as_str(),
            Some("message_stop" | "response.completed" | "response.incomplete")
        )
    })
}
async fn rejected(bytes: Vec<u8>, protocol: u8) {
    let source = Box::pin(stream::iter([Ok::<_, Error>(Bytes::from(bytes))]));
    let mut output = convert(source, protocol);
    let mut errors = 0;
    while let Some(part) = tokio::time::timeout(Duration::from_secs(2), output.next())
        .await
        .unwrap()
    {
        match part {
            Ok(bytes) => assert!(
                !terminal(&bytes, protocol),
                "invalid carrier emitted success"
            ),
            Err(error) => {
                errors += 1;
                let error = error.to_string();
                assert!(!error.contains("private-signature"));
                assert!(!error.contains("private-reasoning"));
            }
        }
    }
    assert_eq!(errors, 1);
}
#[tokio::test]
async fn native_carrier_sequence_projection_and_terminal_mutations_are_rejected() {
    let good = values(&collect(normalized(fixtures::sequence())).await);
    for protocol in [1, 2, 3] {
        for case in 0..14 {
            let mut frames = good.clone();
            let last = frames.len() - 1;
            match case {
                0 => frames[1]["gateway_bedrock_event"]["sequence"] = json!(999),
                1 => frames[1]["choices"][0]["delta"]["content"] = json!("forged"),
                2 => frames[1]["gateway_bedrock_event"]["body"]["delta"]["text"] = json!("forged"),
                3 => frames[last]["gateway_bedrock_terminal"]["events"] = json!(999),
                4 => {
                    frames[last]["gateway_bedrock_turn"]["content"][1]["reasoningContent"]
                        ["reasoningText"]["signature"] = json!("forged")
                }
                5 => frames[last]["usage"]["total_tokens"] = json!(999),
                6 => {
                    frames[0]
                        .as_object_mut()
                        .unwrap()
                        .remove("gateway_bedrock_event");
                }
                7 => frames[0]["gateway_vertex_turn"] = json!({}),
                8 => frames[0]["gateway_bedrock_event"]["unknown"] = json!(true),
                9 => {
                    frames.insert(2, frames[1].clone());
                }
                10 => {
                    frames[0]["type"] = json!("response.completed");
                }
                11 => frames[1]["gateway_bedrock_event"]["projection"] = json!("unknown"),
                12 => frames[0]["gateway_bedrock_event"]["projection"] = json!("content"),
                _ => frames[1]["gateway_bedrock_event"]["projection"] = json!("tool_input"),
            }
            rejected(encoded(&frames, true), protocol).await;
        }
    }
}

#[tokio::test]
async fn xml_tool_detection_cannot_buffer_or_rewrite_a_native_bedrock_witness() {
    let bytes = collect(normalized(fixtures::sequence())).await;
    let dropped = Arc::new(AtomicBool::new(false));
    let source = pending(bytes, dropped.clone());
    let mut output = neuro_gateway::protocol::tool_inject::wrap_streaming_tool_detection_with_error(
        source,
        "model".into(),
        "test".into(),
        Vec::new(),
        None,
        None,
    );
    let error = tokio::time::timeout(Duration::from_secs(1), output.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("native_stream_tool_injection_conflict"));
    assert!(dropped.load(Ordering::SeqCst));
    assert!(output.next().await.is_none());
}
#[tokio::test]
async fn missing_terminal_done_or_clean_complete_frame_is_not_success() {
    let good = values(&collect(normalized(fixtures::sequence())).await);
    for protocol in [1, 2, 3] {
        rejected(encoded(&good[..good.len() - 1], true), protocol).await;
        rejected(encoded(&good, false), protocol).await;
        let mut trailing = encoded(&good, true);
        trailing.extend_from_slice(b"data: {\"unfinished\":");
        rejected(trailing, protocol).await;
    }
}
#[tokio::test]
async fn extra_protocol_content_after_witness_is_not_drained_as_success() {
    let good = values(&collect(normalized(fixtures::sequence())).await);
    for protocol in [1, 2, 3] {
        for extra in [
            b"data: {\"choices\":[]}\n\n".as_slice(),
            b"event: error\ndata: {\"error\":{}}\n\n",
            b"data: broken\n\n",
        ] {
            let mut wire = encoded(&good, true);
            wire.extend_from_slice(extra);
            rejected(wire, protocol).await;
        }
    }
}
struct DropSignal(Arc<AtomicBool>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
fn pending(bytes: Vec<u8>, dropped: Arc<AtomicBool>) -> Source {
    let signal = DropSignal(dropped);
    let mut first = Some(Bytes::from(bytes));
    Box::pin(stream::poll_fn(move |_| {
        let _keep = &signal;
        first.take().map_or(std::task::Poll::Pending, |v| {
            std::task::Poll::Ready(Some(Ok(v)))
        })
    }))
}
#[tokio::test]
async fn public_and_native_terminal_waits_for_real_eof_and_drop_releases_source() {
    let wire = collect(normalized(fixtures::sequence())).await;
    for protocol in [1, 2, 3] {
        let dropped = Arc::new(AtomicBool::new(false));
        let mut output = convert(pending(wire.clone(), dropped.clone()), protocol);
        let mut events = 0;
        let mut reasoning = false;
        loop {
            match tokio::time::timeout(Duration::from_millis(40), output.next()).await {
                Ok(Some(Ok(bytes))) => {
                    assert!(!terminal(&bytes, protocol));
                    events += 1;
                    reasoning |= if protocol == 3 {
                        native_wire::decode(&bytes)
                            .1
                            .pointer("/delta/reasoningContent")
                            .is_some()
                    } else {
                        values(&bytes).iter().any(|v| {
                            v.pointer("/gateway_bedrock_event/body/delta/reasoningContent")
                                .is_some()
                        })
                    };
                }
                Err(_) => break,
                _ => panic!("pending source ended early"),
            }
        }
        assert!(events > 0);
        assert!(reasoning);
        assert!(!dropped.load(Ordering::SeqCst));
        drop(output);
        assert!(dropped.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn error_is_delivered_after_source_drop_without_polling_future_input() {
    let mut good = values(&collect(normalized(fixtures::sequence())).await);
    good[1]["choices"][0]["delta"]["content"] = json!("forged");
    for protocol in [1, 2, 3] {
        let dropped = Arc::new(AtomicBool::new(false));
        let source = pending(encoded(&good, true), dropped.clone());
        let mut output = convert(source, protocol);
        loop {
            match tokio::time::timeout(Duration::from_secs(1), output.next())
                .await
                .unwrap()
                .unwrap()
            {
                Ok(bytes) => assert!(!terminal(&bytes, protocol)),
                Err(_) => {
                    assert!(dropped.load(Ordering::SeqCst));
                    break;
                }
            }
        }
        assert!(output.next().await.is_none());
    }
}

#[tokio::test]
async fn native_carrier_keeps_the_existing_service_integer_usage_boundary() {
    let mut frames = fixtures::sequence();
    *frames.last_mut().unwrap() = wire::event(
        "metadata",
        json!({"usage":{"inputTokens":1,"outputTokens":2,"totalTokens":i32::MAX as u64+1}}),
    );
    let bytes = collect(normalized(frames)).await;
    rejected(bytes, 3).await;
}
#[tokio::test]
async fn malformed_original_reasoning_never_finishes_or_exposes_payload_in_errors() {
    for protocol in 0..4 {
        let mut frames = fixtures::sequence();
        frames[3] = wire::event(
            "contentBlockDelta",
            json!({"contentBlockIndex":1,"delta":{"reasoningContent":{"text":"private-reasoning","signature":"private-signature"}}}),
        );
        let mut output = convert(normalized(frames), protocol);
        let mut errors = 0;
        while let Some(part) = output.next().await {
            match part {
                Ok(bytes) => assert!(!terminal(&bytes, protocol)),
                Err(error) => {
                    errors += 1;
                    let error = error.to_string();
                    assert!(!error.contains("private-reasoning"));
                    assert!(!error.contains("private-signature"));
                }
            }
        }
        assert_eq!(errors, 1);
    }
}
