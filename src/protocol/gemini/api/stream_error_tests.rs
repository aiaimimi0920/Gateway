//! Gemini wire 投影保留类型、错误对象和取消所有权，不扩展 parser 行为。
use super::*;
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use crate::protocol::stream_error_test_support::probe;
use std::sync::atomic::Ordering;

fn frame(value: Value) -> Bytes {
    Bytes::from(format!("data: {value}\r\n\r\n"))
}

#[tokio::test]
async fn fragmented_text_tools_usage_and_finish_wire_remain_unchanged() {
    let text = frame(json!({"choices": [{"delta": {"content": "你好"}}]}));
    let split = text
        .windows(3)
        .position(|part| part == "你".as_bytes())
        .unwrap()
        + 1;
    let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
        Ok(text.slice(..split)),
        Ok(text.slice(split..)),
        Ok(frame(
            json!({"choices": [{"delta": {"tool_calls": [{"index": 0,
            "id": "weather-1", "function": {"name": "weather", "arguments": "{\"city\":"}}]}}], "usage": null}),
        )),
        Ok(frame(
            json!({"choices": [{"delta": {"tool_calls": [{"index": 0,
            "function": {"arguments": "\"杭州\"}"}}]}}], "usage": null}),
        )),
        Ok(frame(json!({"choices": [{"finish_reason": "stop"}],
            "usage": {"prompt_tokens": 4, "completion_tokens": 2, "total_tokens": 6}}))),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ];
    let outputs: Vec<_> = translate_openai_sse_to_gemini_stream_with_error(
        futures::stream::iter(chunks),
        "gemini-test".into(),
    )
    .collect()
    .await;
    assert_eq!(outputs.len(), 3);
    let payloads: Vec<Value> = outputs
        .into_iter()
        .map(|output| {
            let output = output.unwrap();
            let text = std::str::from_utf8(&output).unwrap();
            serde_json::from_str(text.strip_prefix("data: ").unwrap().trim()).unwrap()
        })
        .collect();
    assert_eq!(
        payloads[0]["candidates"][0]["content"]["parts"][0]["text"],
        "你好"
    );
    let call = &payloads[1]["candidates"][0]["content"]["parts"][0]["functionCall"];
    assert_eq!(call["id"], "weather-1");
    assert_eq!(call["name"], "weather");
    assert_eq!(call["args"], json!({"city": "杭州"}));
    assert_eq!(payloads[2]["candidates"][0]["finishReason"], "STOP");
    assert_eq!(
        payloads[2]["usageMetadata"],
        json!({
            "promptTokenCount": 4, "candidatesTokenCount": 2, "totalTokenCount": 6,
        })
    );
}

#[derive(Debug)]
struct Marker(u64);

#[tokio::test]
async fn trailing_usage_is_attached_to_one_terminal_candidate_at_done_or_eof() {
    for done in [false, true] {
        let mut chunks = vec![
            Ok::<_, std::io::Error>(frame(json!({"choices":[{"finish_reason":"length"}],
                "usage":{"prompt_tokens":7,"completion_tokens":1,"total_tokens":8}}))),
            Ok(frame(json!({"choices":[],"usage":null}))),
            Ok(frame(
                json!({"choices":[],"usage":{"completion_tokens":3,"total_tokens":10}}),
            )),
            Ok(frame(
                json!({"choices":[],"usage":{"prompt_tokens":"invalid"}}),
            )),
        ];
        if done {
            chunks.push(Ok(Bytes::from_static(b"data: [DONE]\n\n")));
        }
        // In the DONE case the connection remains open; the final frame must not wait for TCP EOF.
        let (source, _) = probe(chunks, done);
        let mut stream = Box::pin(translate_openai_sse_to_gemini_stream_with_error(
            source,
            "test".into(),
        ));
        let std::task::Poll::Ready(Some(Ok(bytes))) = futures::poll!(stream.next()) else {
            panic!("expected terminal candidate");
        };
        let data = std::str::from_utf8(&bytes)
            .unwrap()
            .strip_prefix("data: ")
            .unwrap();
        let payload: Value = serde_json::from_str(data.trim()).unwrap();
        assert_eq!(
            payload,
            json!({
                "candidates":[{"index":0,"finishReason":"MAX_TOKENS"}],
                "usageMetadata":{"promptTokenCount":7,"candidatesTokenCount":3,"totalTokenCount":10}
            })
        );
        if done {
            assert!(futures::poll!(stream.next()).is_pending());
        } else {
            assert!(stream.next().await.is_none());
        }
    }
}

#[tokio::test]
async fn accounting_without_a_finish_reason_does_not_complete_a_candidate() {
    let usage = frame(json!({"choices":[], "usage":{"total_tokens":10}}));
    let source = futures::stream::iter([
        Ok::<_, std::io::Error>(usage),
        Ok(Bytes::from_static(b"data: [DONE]\n\n")),
    ]);
    assert_eq!(
        translate_openai_sse_to_gemini_stream_with_error(source, "test".into())
            .count()
            .await,
        0
    );
}

#[tokio::test]
async fn complete_tool_is_emitted_once_at_done_or_clean_eof() {
    for done in [false, true] {
        let mut chunks = vec![Ok::<_, std::io::Error>(frame(json!({"choices":[{"delta":{
            "tool_calls":[{"function":{"name":"lookup","arguments":"{\"k\":1}"}}]
        }}]})))];
        if done {
            chunks.push(Ok(Bytes::from_static(b"data: [DONE]\n\n")));
        }
        let outputs: Vec<_> = translate_openai_sse_to_gemini_stream_with_error(
            futures::stream::iter(chunks),
            "test".into(),
        )
        .collect()
        .await;
        assert_eq!(outputs.len(), 1);
        let bytes = outputs[0].as_ref().unwrap();
        assert!(std::str::from_utf8(bytes)
            .unwrap()
            .contains("\"args\":{\"k\":1}"));
    }
}

#[tokio::test]
async fn nonzero_error_object_is_terminal_after_partial_output() {
    // Box 的非零大小对象地址可证明移动的是原对象；E 不要求任何 From impl。
    let error = Box::new(Marker(73));
    let identity = &*error as *const Marker;
    let (source, state) = probe(
        vec![
            Ok(frame(
                json!({"choices": [{"delta": {"content": "partial"}, "finish_reason": "stop"}]}),
            )),
            Err(error),
            Ok(frame(json!({"choices": [{"finish_reason": "stop"}]}))),
        ],
        false,
    );
    let mut stream = Box::pin(translate_openai_sse_to_gemini_stream_with_error(
        source,
        "test".into(),
    ));
    let output = stream.next().await.unwrap().unwrap();
    assert!(std::str::from_utf8(&output).unwrap().contains("partial"));
    assert!(!std::str::from_utf8(&output)
        .unwrap()
        .contains("finishReason"));
    let received = stream.next().await.unwrap().unwrap_err();
    assert_eq!(received.0, 73);
    assert_eq!(&*received as *const Marker, identity);
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    assert!(stream.next().await.is_none());
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn existing_protocol_variant_is_forwarded_without_a_transport_conversion() {
    let (source, state) = probe(
        vec![Err(StreamError::<std::io::Error>::Protocol(
            ProtocolStreamError::invalid_data("upstream local limit"),
        ))],
        false,
    );
    let mut stream = Box::pin(translate_openai_sse_to_gemini_stream_with_error(
        source,
        "test".into(),
    ));
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(matches!(error, StreamError::Protocol(_)));
    assert!(error.to_string().contains("upstream local limit"));
    assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    assert!(stream.next().await.is_none());
    assert_eq!(state.polls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_releases_unpolled_pending_and_buffered_sources() {
    for phase in 0..3 {
        let chunks = if phase == 2 {
            vec![Ok(frame(
                json!({"choices": [{"delta": {"content": "partial"},
                "finish_reason": "stop"}]}),
            ))]
        } else {
            vec![]
        };
        let (source, state) = probe::<std::io::Error>(chunks, true);
        let mut stream = Box::pin(translate_openai_sse_to_gemini_stream_with_error(
            source,
            "test".into(),
        ));
        if phase == 1 {
            assert!(futures::poll!(stream.next()).is_pending());
        } else if phase == 2 {
            assert!(stream.next().await.unwrap().is_ok());
        }
        drop(stream);
        assert_eq!(state.polls.load(Ordering::SeqCst), usize::from(phase != 0));
        assert_eq!(state.drops.load(Ordering::SeqCst), 1);
    }
}
