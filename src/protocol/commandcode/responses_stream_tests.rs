//! Validate framing independently of chunk boundaries while preserving opaque native items.
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};

fn terminal() -> Value {
    json!({"type":"response.completed","response":{"id":"resp","object":"response","status":"completed","model":"upstream","output":[],"usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}})
}
fn frame(value: &Value) -> String {
    format!("data: {value}\n\n")
}

async fn collect(raw: &str, size: usize) -> (Vec<Value>, bool) {
    let chunks = raw
        .as_bytes()
        .chunks(size)
        .map(|chunk| {
            Ok::<_, crate::protocol::stream_error::ProtocolStreamError>(Bytes::copy_from_slice(
                chunk,
            ))
        })
        .collect::<Vec<_>>();
    let mut stream = Box::pin(super::guard(futures::stream::iter(chunks), "public".into()));
    let mut values = Vec::new();
    let mut failed = false;
    while let Some(result) = stream.next().await {
        match result {
            Ok(bytes) => {
                let text = std::str::from_utf8(&bytes).unwrap();
                values.extend(
                    text.lines()
                        .filter_map(|line| line.strip_prefix("data: "))
                        .map(|data| serde_json::from_str::<Value>(data).unwrap()),
                );
            }
            Err(_) => failed = true,
        }
    }
    (values, failed)
}

#[tokio::test]
async fn commandcode_responses_sse_extensions_survive_fragmentation() {
    let extension = json!({"type":"response.future","response":{"model":"opaque-model"},"item":{"model":"tool-data"},"delta":"推理"});
    let end = terminal();
    let raw = frame(&extension) + &frame(&end) + "data: [DONE]\n\n";
    for size in [1, 47, 8192] {
        let (actual, failed) = collect(&raw, size).await;
        let mut end = end.clone();
        end["response"]["model"] = "public".into();
        assert!(!failed);
        assert_eq!(actual, vec![extension.clone(), end]);
    }
}

#[tokio::test]
async fn commandcode_responses_sse_invalid_frames_never_publish_terminal() {
    let end = frame(&terminal());
    let mut mismatch = terminal();
    mismatch["response"]["status"] = "incomplete".into();
    for raw in [
        format!("event: response.failed\n{end}"),
        frame(&json!({"type":"response.future\nevent: response.completed"})) + &end,
        frame(&mismatch),
        end.clone() + "event: error\ndata: [DONE]\n\n",
        end.clone() + "data: {\"type\":\"error\",\"message\":\"secret\"}\n\n",
        end.clone() + "data: {",
        end.clone() + &end,
        "data: [DONE]\n\n".to_owned(),
    ] {
        let (values, failed) = collect(&raw, 7).await;
        assert!(failed);
        assert!(!values
            .iter()
            .any(|value| value["type"] == "response.completed"));
        assert!(!serde_json::to_string(&values).unwrap().contains("secret"));
    }
}
