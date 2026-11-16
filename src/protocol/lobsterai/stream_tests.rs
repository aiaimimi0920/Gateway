//! The stream owns both tool-tail ordering and immediate release on a local limit error.
use super::*;
#[tokio::test]
async fn lobsterai_tool_delta_precedes_finish_usage_and_done() {
    let raw = [
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call","function":{"name":"lookup","arguments":"{}"}}]}}]}),
        json!({"choices":[],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}),
    ].into_iter().map(|v|format!("data: {v}\n\n")).collect::<String>()+"data: [DONE]\n\n";
    let input = futures::stream::iter([Ok::<_, ProtocolStreamError>(Bytes::from(raw))]);
    let mut stream = Box::pin(validated(input));
    let mut data = String::new();
    while let Some(chunk) = stream.next().await {
        data.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
    }
    let tool = data.find("\"name\":\"lookup\"").unwrap();
    let finish = data.find("\"finish_reason\":\"tool_calls\"").unwrap();
    let usage = data.find("\"usage\"").unwrap();
    assert!(
        tool < finish && finish < usage && usage < data.find("[DONE]").unwrap(),
        "{data}"
    );
}
#[tokio::test]
async fn lobsterai_tool_limit_error_releases_pending_http_before_returning_error() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    struct Probe(Arc<AtomicBool>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let probe = Probe(dropped.clone());
    let raw = format!(
        "data: {}\n\n",
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call","function":{"name":"lookup","arguments":"x".repeat(64)}}]}}]})
    );
    let input = futures::stream::iter([Ok::<_, ProtocolStreamError>(Bytes::from(raw))])
        .chain(futures::stream::pending())
        .map(move |value| {
            let _ = &probe;
            value
        });
    let mut stream = Box::pin(with_limit(input, 32));
    assert!(stream.next().await.unwrap().is_err());
    assert!(
        dropped.load(Ordering::SeqCst),
        "source retained after local error"
    );
    assert!(stream.next().await.is_none());
}
