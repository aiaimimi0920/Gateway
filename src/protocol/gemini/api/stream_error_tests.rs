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
            "id": "weather-1", "function": {"name": "weather", "arguments": "{\"city\":"}}]}}]}),
        )),
        Ok(frame(
            json!({"choices": [{"delta": {"tool_calls": [{"index": 0,
            "function": {"arguments": "\"杭州\"}"}}]}}]}),
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
    assert_eq!(outputs.len(), 4);
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
    let call = &payloads[2]["candidates"][0]["content"]["parts"][0]["functionCall"];
    assert_eq!(call["id"], "weather-1");
    assert_eq!(call["name"], "weather");
    assert_eq!(call["args"], json!({"city": "杭州"}));
    assert_eq!(payloads[3]["candidates"][0]["finishReason"], "STOP");
    assert_eq!(
        payloads[3]["usageMetadata"],
        json!({
            "promptTokenCount": 4, "candidatesTokenCount": 2, "totalTokenCount": 6,
        })
    );
}

#[derive(Debug)]
struct Marker(u64);

#[tokio::test]
async fn nonzero_error_object_is_terminal_after_partial_output() {
    // Box 的非零大小对象地址可证明移动的是原对象；E 不要求任何 From impl。
    let error = Box::new(Marker(73));
    let identity = &*error as *const Marker;
    let (source, state) = probe(
        vec![
            Ok(frame(
                json!({"choices": [{"delta": {"content": "partial"}}]}),
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
