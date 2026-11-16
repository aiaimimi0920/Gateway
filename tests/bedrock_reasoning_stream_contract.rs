//! Synthetic AWS streaming fidelity; no platform credentials or live supplier calls.
#![cfg(feature = "family-bedrock-converse-official-api")]
#[path = "bedrock_reasoning_stream_contract/adversarial.rs"]
mod adversarial;
#[path = "bedrock_reasoning_response_contract/fixtures.rs"]
mod fixtures;
#[path = "bedrock_native_outbound_contract/fixture.rs"]
mod native_wire;
#[path = "bedrock_reasoning_stream_contract/routes.rs"]
mod routes;
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;

use bytes::Bytes;
use futures::{stream, Stream, StreamExt};
use neuro_gateway::protocol::{anthropic, bedrock_converse, responses, stream_error::StreamError};
use serde_json::Value;
use std::{pin::Pin, time::Duration};
type Error = StreamError<std::io::Error>;
type Source = Pin<Box<dyn Stream<Item = Result<Bytes, Error>> + Send>>;

fn normalized(frames: Vec<Vec<u8>>) -> Source {
    Box::pin(
        bedrock_converse::translate_bedrock_eventstream_to_openai_with_error(
            stream::iter(frames.into_iter().map(|v| Ok::<_, Error>(Bytes::from(v)))),
            "model".into(),
        ),
    )
}
fn public(source: Source, protocol: u8) -> Source {
    match protocol {
        1 => Box::pin(anthropic::translate_openai_sse_to_anthropic_with_error(
            source,
            "model".into(),
        )),
        2 => Box::pin(responses::translate_openai_sse_to_responses_with_error(
            source,
            "model".into(),
        )),
        _ => source,
    }
}
fn values(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .split("\n\n")
        .filter_map(|frame| frame.lines().find_map(|s| s.strip_prefix("data: ")))
        .filter(|s| *s != "[DONE]")
        .map(|s| serde_json::from_str(s).unwrap())
        .collect()
}
fn native_event(chunk: &Value) -> Value {
    let mut event = chunk["gateway_bedrock_event"].clone();
    match event.get("projection").and_then(Value::as_str) {
        Some("content") => {
            event["body"]["delta"]["text"] = chunk["choices"][0]["delta"]["content"].clone()
        }
        Some("tool_input") => {
            event["body"]["delta"]["toolUse"]["input"] =
                chunk["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"].clone()
        }
        None => {}
        _ => panic!("unknown native projection"),
    }
    event.as_object_mut().unwrap().remove("projection");
    event
}
async fn collect(source: Source) -> Vec<u8> {
    let mut output = Vec::new();
    for part in source.collect::<Vec<_>>().await {
        output.extend_from_slice(&part.expect("reasoning must not fail"));
    }
    output
}

#[tokio::test]
async fn normalized_stream_retains_every_native_event_and_final_ordered_turn() {
    let frames = fixtures::sequence();
    let expected: Vec<_> = frames
        .iter()
        .map(|v| native_wire::decode(&Bytes::copy_from_slice(v)))
        .collect();
    let output = values(&collect(normalized(frames)).await);
    let events: Vec<_> = output
        .iter()
        .filter(|v| v.get("gateway_bedrock_event").is_some())
        .map(native_event)
        .collect();
    assert_eq!(events.len(), expected.len());
    for (index, (event, original)) in events.iter().zip(expected).enumerate() {
        assert_eq!(event["sequence"], index);
        assert_eq!(event["event"], original.0);
        assert_eq!(event["body"], original.1);
    }
    let terminal = output.last().unwrap();
    assert_eq!(terminal["gateway_bedrock_terminal"]["events"], events.len());
    assert_eq!(
        terminal["gateway_bedrock_turn"]["content"],
        fixtures::content()
    );
}

#[tokio::test]
async fn native_roundtrip_keeps_original_blocks_close_timing_and_reasoning_fragments() {
    let frames = fixtures::sequence();
    let expected: Vec<_> = frames
        .iter()
        .map(|v| native_wire::decode(&Bytes::copy_from_slice(v)))
        .collect();
    let output = bedrock_converse::translate_openai_sse_to_bedrock_eventstream_with_error(
        normalized(frames),
        "model".into(),
    )
    .collect::<Vec<_>>()
    .await;
    let actual: Vec<_> = output
        .into_iter()
        .map(|v| native_wire::decode(&v.expect("native reasoning roundtrip")))
        .collect();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn messages_and_responses_keep_real_time_events_and_validated_terminal_turn() {
    for protocol in [1, 2] {
        let frames = fixtures::sequence();
        let count = frames.len();
        let output = values(&collect(public(normalized(frames), protocol)).await);
        assert_eq!(
            output
                .iter()
                .filter(|v| v.get("gateway_bedrock_event").is_some())
                .count(),
            count
        );
        let terminal = output
            .iter()
            .find_map(|v| {
                let v = v.get("response").unwrap_or(v);
                v.get("gateway_bedrock_terminal").map(|_| v)
            })
            .expect("public terminal witness");
        assert_eq!(
            terminal["gateway_bedrock_turn"]["content"],
            fixtures::content()
        );
        assert_eq!(terminal["gateway_bedrock_terminal"]["events"], count);
    }
}

#[tokio::test]
async fn reasoning_is_delivered_before_source_eof_not_only_as_terminal_snapshot() {
    let prefix = fixtures::sequence()[..5].concat();
    let source = stream::iter([Ok::<_, Error>(Bytes::from(prefix))]).chain(stream::pending());
    let mut output = Box::pin(
        bedrock_converse::translate_bedrock_eventstream_to_openai_with_error(
            source,
            "model".into(),
        ),
    );
    let mut found = false;
    for _ in 0..5 {
        let part = tokio::time::timeout(Duration::from_secs(1), output.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        for v in values(&part) {
            found |= v
                .pointer("/gateway_bedrock_event/body/delta/reasoningContent/text")
                .is_some();
            assert!(v.get("gateway_bedrock_terminal").is_none());
        }
    }
    assert!(found);
}

#[tokio::test]
async fn native_carrier_does_not_duplicate_large_visible_text_and_shrink_client_frame_budget() {
    // The client admits 64 MiB frames, not the service-only 24 MiB restriction.
    let size = 34 * 1024 * 1024;
    let mut frames = wire::sequence();
    frames[1] = wire::event(
        "contentBlockDelta",
        serde_json::json!({"contentBlockIndex":0,"delta":{"text":"x".repeat(size)}}),
    );
    let parts = normalized(frames).collect::<Vec<_>>().await;
    assert!(
        parts.iter().all(Result::is_ok),
        "native carrier reduced the existing client budget"
    );
    let mut visible = 0;
    for part in parts {
        let part = part.unwrap();
        assert!(part.len() <= 64 * 1024 * 1024);
        visible += values(&part)
            .iter()
            .filter_map(|v| {
                v.pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
            })
            .map(str::len)
            .sum::<usize>();
    }
    assert_eq!(visible, size);
}

#[tokio::test]
async fn large_client_text_remains_service_compliant_without_losing_native_indices() {
    let size = 34 * 1024 * 1024;
    let mut frames = wire::sequence();
    frames[1] = wire::event(
        "contentBlockDelta",
        serde_json::json!({"contentBlockIndex":0,"delta":{"text":"x".repeat(size)}}),
    );
    let output = bedrock_converse::translate_openai_sse_to_bedrock_eventstream_with_error(
        normalized(frames),
        "model".into(),
    )
    .collect::<Vec<_>>()
    .await;
    assert!(
        output.iter().all(Result::is_ok),
        "native output shrank the admitted text budget"
    );
    let frames: Vec<_> = output
        .into_iter()
        .map(|v| {
            let v = v.unwrap();
            let (kind, body) = native_wire::decode(&v);
            if kind == "contentBlockDelta" && body.pointer("/delta/text").is_some() {
                assert_eq!(body["contentBlockIndex"], 0);
                assert!(v.len() < 24 * 1024 * 1024);
            }
            v.to_vec()
        })
        .collect();
    let result = neuro_gateway::upstream::bedrock_converse_official_api_common::accumulate_stream(
        wire::response(frames),
        "model",
    )
    .await
    .unwrap();
    assert_eq!(result.text.len(), size);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.usage.unwrap().total_tokens, 13);
}
