//! Verify native admission and bounded tool assembly through the public Messages conversion path.
use crate::protocol::stream_error::StreamError;
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{json, Value};
fn frames(values: Vec<Value>) -> Bytes {
    Bytes::from(
        values
            .into_iter()
            .map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap()))
            .collect::<String>(),
    )
}
fn start() -> Value {
    json!({"type":"message_start","message":{"type":"message","role":"assistant","content":[]}})
}
fn finish() -> Value {
    json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}})
}
fn stop() -> Value {
    json!({"type":"message_stop"})
}
async fn read(values: Vec<Value>) -> Result<String, StreamError<rquest::Error>> {
    let input =
        futures::stream::once(async { Ok::<_, StreamError<rquest::Error>>(frames(values)) });
    let mut stream = Box::pin(super::stream::from_messages(
        super::messages::translate(input, "model".into()),
        None,
    ));
    let mut out = String::new();
    while let Some(chunk) = stream.next().await {
        out.push_str(std::str::from_utf8(&chunk?).unwrap());
    }
    Ok(out)
}
#[tokio::test]
async fn lobsterai_resolved_messages_rejects_truncation_unknown_blocks_and_error_tails() {
    for values in [
        vec![start()],
        vec![start(), finish()],
        vec![start(), stop()],
        vec![
            start(),
            finish(),
            stop(),
            json!({"type":"error","error":{"message":"secret"}}),
        ],
        vec![
            start(),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"secret"}}),
            finish(),
            stop(),
        ],
        vec![
            start(),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"orphan"}}),
            finish(),
            stop(),
        ],
        vec![
            start(),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":"x"}}),
            finish(),
            stop(),
        ],
    ] {
        let error = read(values).await.unwrap_err().to_string();
        assert!(!error.contains("secret"));
    }
}
#[tokio::test]
async fn lobsterai_resolved_messages_empty_tool_arguments_and_unknown_usage() {
    let values = vec![
        start(),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call","name":"ping","input":{}}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}}),
        stop(),
    ];
    let out = read(values).await.unwrap();
    assert!(out.contains("\"arguments\":\"{}\""));
    assert!(!out.contains("usage"));
    assert!(out.ends_with("data: [DONE]\n\n"));
}
#[tokio::test]
async fn lobsterai_resolved_messages_preserves_cache_usage_and_terminal_order() {
    let mut beginning = start();
    beginning["message"]["usage"] = json!({"input_tokens":2,"output_tokens":0,"cache_read_input_tokens":4,"cache_creation_input_tokens":6});
    let mut ending = finish();
    ending["usage"] = json!({"output_tokens":3});
    let out = read(vec![beginning, ending, stop()]).await.unwrap();
    assert!(out.contains("\"total_tokens\":5"));
    assert!(out.contains("\"cache_read_input_tokens\":4"));
    assert!(out.contains("\"cache_creation_input_tokens\":6"));
    assert!(out.find("\"finish_reason\":\"stop\"").unwrap() < out.find("\"usage\"").unwrap());
}

#[tokio::test]
async fn lobsterai_resolved_messages_missing_final_usage_is_unknown_and_bad_tools_fail() {
    let mut beginning = start();
    beginning["message"]["usage"] = json!({"input_tokens":2,"output_tokens":0});
    let out = read(vec![beginning, finish(), stop()]).await.unwrap();
    assert!(!out.contains("usage"));
    assert!(read(vec![
        start(),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}}),
        stop()
    ])
    .await
    .is_err());
    for args in ["{", "[]", "{\"x\":1,\"x\":2}"] {
        let values = vec![
            start(),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call","name":"ping","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":args}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"}}),
            stop(),
        ];
        assert!(read(values).await.is_err(), "{args}");
    }
}
