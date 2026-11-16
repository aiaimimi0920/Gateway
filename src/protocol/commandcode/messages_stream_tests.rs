//! Streamed function arguments need the same complete-object boundary as buffered Messages.
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;

pub(super) fn tool_reply(arguments: &str) -> Vec<u8> {
    [
        json!({"type":"message_start","message":{"type":"message","id":"m","role":"assistant","model":"upstream","content":[],"usage":{"input_tokens":2,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call","name":"lookup","input":{}}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":arguments}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":3}}),
        json!({"type":"message_stop"}),
    ].iter().map(|value| format!("data: {value}\n\n")).collect::<String>().into_bytes()
}

#[tokio::test]
async fn commandcode_native_messages_tool_json_must_be_a_complete_unique_object() {
    for (arguments, valid) in [
        ("{", false),
        ("[1]", false),
        ("null", false),
        (r#"{"x":1,"x":2}"#, false),
        (r#"{"x":{"a":1,"a":2}}"#, false),
        (r#"{"city":"杭州","nested":[1,2]}"#, true),
        ("", true),
    ] {
        let input = futures::stream::iter(tool_reply(arguments).into_iter().map(|b| {
            Ok::<_, crate::protocol::stream_error::ProtocolStreamError>(Bytes::from(vec![b]))
        }));
        let mut output = Box::pin(super::messages_stream::guard(input, "public-model".into()));
        let mut bytes = Vec::new();
        let mut failed = false;
        while let Some(result) = output.next().await {
            match result {
                Ok(chunk) => bytes.extend_from_slice(&chunk),
                Err(_) => failed = true,
            }
        }
        assert_eq!(!failed, valid, "{arguments}");
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text.contains("event: message_stop"), valid, "{text}");
        assert!(text.contains("public-model"));
    }
}
