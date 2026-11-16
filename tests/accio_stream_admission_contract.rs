//! Deterministic wire boundary cases complement the real HTTP ingress contracts.
#![cfg(feature = "line-accio-web-reverse-api")]
#[path = "accio_relay_contract/fixture.rs"]
mod fixture;
use bytes::Bytes;
use futures::StreamExt;
use neuro_gateway::protocol::{accio, stream_error::StreamError};
use serde_json::{json, Value};
use std::convert::Infallible;

async fn translate(chunks: Vec<Vec<u8>>) -> Vec<Result<Bytes, StreamError<Infallible>>> {
    accio::translate_accio_stream_with_error(
        futures::stream::iter(chunks.into_iter().map(|bytes| Ok(Bytes::from(bytes)))),
        fixture::MODEL.into(),
    )
    .collect()
    .await
}
fn joined(outputs: &[Result<Bytes, StreamError<Infallible>>]) -> String {
    outputs
        .iter()
        .filter_map(|output| output.as_ref().ok())
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .collect()
}
#[tokio::test]
async fn accio_stream_handles_every_byte_boundary_and_crlf_multiline_sse() {
    let body = fixture::chunks("good").concat();
    for split in 1..body.len() {
        let outputs = translate(vec![body[..split].to_vec(), body[split..].to_vec()]).await;
        assert!(
            outputs.iter().all(Result::is_ok),
            "split {split}: {outputs:?}"
        );
        let text = joined(&outputs);
        assert_eq!(text.matches("[DONE]").count(), 1);
        assert!(text.contains("OK 你好"));
        assert!(text.contains("\"total_tokens\":9"));
        assert!(text.contains("\"cache_read_input_tokens\":4"));
        assert!(text.contains("\"cache_creation_input_tokens\":3"));
    }
    let wire=b"event: message\r\ndata: {\"type\":\"content_block_delta\",\r\ndata: \"delta\":{\"text\":\"OK\"}}\r\n\r\ndata: [DONE]\r\n\r\n";
    let outputs = translate(wire.chunks(1).map(<[u8]>::to_vec).collect()).await;
    assert!(outputs.iter().all(Result::is_ok), "{outputs:?}");
    assert_eq!(joined(&outputs).matches("[DONE]").count(), 1);
}
#[tokio::test]
async fn accio_stream_rejects_malformed_oversized_or_orphan_frames_without_success() {
    let mut cases = vec![
        vec![b"data: {\"raw_response_json\":5}\n\n".to_vec()],
        vec![b"data: {\"raw_response_json\":\"{broken\"}\n\n".to_vec()],
        vec![b"data: \xff\n\n".to_vec()],
        vec![vec![b'x'; 256 * 1024 + 1]],
        vec![fixture::text("OK"), b"data: {truncated".to_vec()],
        fixture::chunks("orphan-tool"),
        vec![
            fixture::event(json!({"type":"content_block_delta","delta":{"text":""}})),
            b"data: [DONE]\n\n".to_vec(),
        ],
    ];
    let start = fixture::event(
        json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tool-1","name":"weather"}}),
    );
    cases.push(vec![start.clone(), start, b"data: [DONE]\n\n".to_vec()]);
    let outer = json!({"raw_response_json":json!({"type":"message_stop"}).to_string(),"error_code":"5015","error_message":"synthetic-private-upstream"});
    cases.push(vec![
        fixture::text("partial"),
        format!("data: {outer}\n\n").into_bytes(),
    ]);
    for (index, chunks) in cases.into_iter().enumerate() {
        let outputs = translate(chunks).await;
        assert_eq!(
            outputs.iter().filter(|output| output.is_err()).count(),
            1,
            "case {index}: {outputs:?}"
        );
        let text = joined(&outputs);
        assert!(!text.contains("[DONE]"), "case {index}: {text}");
        assert!(!format!("{outputs:?}").contains("synthetic-private-upstream"));
    }
}
#[tokio::test]
async fn accio_native_accumulator_preserves_total_only_and_cache_usage_without_overflow() {
    for usage in [
        json!({"total_tokens":19}),
        json!({"input_tokens":u64::MAX,"output_tokens":u64::MAX,"total_tokens":u64::MAX}),
    ] {
        let wire = [
            fixture::text("OK"),
            fixture::event(
                json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":usage}),
            ),
            b"data: [DONE]\n\n".to_vec(),
        ]
        .concat();
        let response = rquest::Response::from(axum::http::Response::new(wire));
        let response = accio::accumulate_accio_stream(response, fixture::MODEL)
            .await
            .unwrap();
        assert_eq!(
            response.usage.unwrap().total_tokens,
            usage["total_tokens"].as_u64().unwrap()
        );
    }
}
#[tokio::test]
async fn accio_transport_errors_are_not_converted_to_success_or_swallowed() {
    let wire = vec![
        Ok(Bytes::from(fixture::text("partial"))),
        Err(StreamError::Transport("synthetic transport error")),
    ];
    let outputs: Vec<_> = accio::translate_accio_stream_with_error(
        futures::stream::iter(wire),
        fixture::MODEL.into(),
    )
    .collect()
    .await;
    assert_eq!(outputs.iter().filter(|output| output.is_err()).count(), 1);
    assert!(matches!(
        &outputs.last().unwrap(),
        Err(StreamError::Transport("synthetic transport error"))
    ));
    assert!(!outputs
        .iter()
        .filter_map(|output| output.as_ref().ok())
        .any(|bytes| String::from_utf8_lossy(bytes).contains("[DONE]")));
}
#[tokio::test]
async fn accio_legacy_binary_envelope_compatibility_survives_arbitrary_chunking() {
    // Existing compatibility format deliberately has no CRC guarantee. Not a claim of Accio AWS support.
    let frame = |raw: Value| {
        let body = serde_json::to_vec(&raw).unwrap();
        let mut frame = Vec::new();
        frame.extend_from_slice(&((body.len() + 16) as u32).to_be_bytes());
        frame.extend_from_slice(&[0; 8]);
        frame.extend(body);
        frame.extend_from_slice(&[0; 4]);
        frame
    };
    let bytes = [
        frame(json!({"contentBlockDelta":{"delta":{"text":"OK"}}})),
        frame(json!({"messageStop":{"stopReason":"end_turn"}})),
    ]
    .concat();
    let outputs = translate(bytes.chunks(1).map(<[u8]>::to_vec).collect()).await;
    assert!(outputs.iter().all(Result::is_ok), "{outputs:?}");
    assert_eq!(joined(&outputs).matches("[DONE]").count(), 1);
    let outputs = translate(vec![bytes[..bytes.len() - 1].to_vec()]).await;
    assert_eq!(outputs.iter().filter(|output| output.is_err()).count(), 1);
    assert!(!joined(&outputs).contains("[DONE]"));
}

#[tokio::test]
async fn accio_mixed_and_reused_source_tool_indexes_are_distinct_downstream() {
    let mut chunks = vec![fixture::event(
        json!({"tool_calls":[{"id":"a","function":{"name":"first","arguments":"{}"}}]}),
    )];
    for id in ["b", "c"] {
        chunks.push(fixture::event(json!({"contentBlockStart":{"contentBlockIndex":0,"start":{"toolUse":{"toolUseId":id,"name":"next"}}}})));
        chunks.push(fixture::event(
            json!({"contentBlockDelta":{"contentBlockIndex":0,"delta":{"partial_json":"{}"}}}),
        ));
        chunks.push(fixture::event(
            json!({"contentBlockStop":{"contentBlockIndex":0}}),
        ));
    }
    chunks.push(fixture::end());
    let outputs = translate(chunks.clone()).await;
    assert!(outputs.iter().all(Result::is_ok), "{outputs:?}");
    let indexes = |text: String| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .filter_map(|value| {
                value
                    .pointer("/choices/0/delta/tool_calls/0/index")
                    .and_then(Value::as_u64)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(indexes(joined(&outputs)), [0, 1, 2]);
    // The shared renderer remains valid for non-Accio callers too; it must not apply Accio EOF rules.
    let shared: Vec<_> = accio::translate_anthropic_like_stream_to_openai_with_error(
        futures::stream::iter(
            chunks
                .into_iter()
                .map(|bytes| Ok::<_, Infallible>(Bytes::from(bytes))),
        ),
        fixture::MODEL.into(),
    )
    .collect()
    .await;
    assert_eq!(
        indexes(
            shared
                .into_iter()
                .map(Result::unwrap)
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .collect()
        ),
        [0, 1, 2]
    );
}

#[tokio::test]
async fn accio_supported_gemini_stop_reason_is_canonical_for_model_probes() {
    let wire = fixture::event(
        json!({"candidates":[{"content":{"parts":[{"text":"OK"}]},"finishReason":"STOP"}]}),
    );
    let response = rquest::Response::from(axum::http::Response::new(wire));
    let response = accio::accumulate_accio_stream(response, fixture::MODEL)
        .await
        .unwrap();
    assert_eq!(response.finish_reason.as_deref(), Some("stop"));
}
