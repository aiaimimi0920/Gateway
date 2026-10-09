use super::*;
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};
use futures::StreamExt;

#[test]
fn dashscope_native_preserves_vendor_fields() {
    let body = json!({"model":"qwen-vl-ocr","input":{"messages":[{"role":"user","content":[{"image":"https://example.com/a.png","enable_rotate":true}]}]},"parameters":{"ocr_options":{"task":"advanced_recognition"}}});
    let req = normalize(body.clone(), true, false).unwrap();
    assert_eq!(pack(&req, "qwen-vl-ocr", false, true).unwrap(), body);
    assert!(validate_bridge(&req).is_err());
}

#[test]
fn dashscope_text_and_image_bridge() {
    let req = normalize(json!({"model":"qwen","input":{"messages":[{"role":"user","content":[{"text":"describe"},{"image":"https://example.com/a.png"}]}]},"parameters":{"max_tokens":321,"result_format":"message"}}),true,true).unwrap();
    validate_bridge(&req).unwrap();
    let chat = openai::pack_openai(&req, "target", true);
    assert_eq!(chat["max_tokens"], 321);
    assert_eq!(
        chat["messages"][0]["content"][1]["image_url"]["url"],
        "https://example.com/a.png"
    );
    assert!(chat.get("result_format").is_none());
    let chat_req = openai::normalize_chat_completions(chat).unwrap();
    let packed = pack(&chat_req, "qwen", true, true).unwrap();
    assert_eq!(packed["parameters"]["incremental_output"], true);
    assert_eq!(
        packed["input"]["messages"][0]["content"][1]["image"],
        "https://example.com/a.png"
    );
}

#[test]
fn dashscope_response_and_usage() {
    let native = json!({"request_id":"r1","output":{"choices":[{"message":{"role":"assistant","content":[{"text":"hello"}]},"finish_reason":"stop"}]},"usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5}});
    let chat = response::to_openai(&native, "qwen", false).unwrap();
    assert_eq!(chat["choices"][0]["message"]["content"], "hello");
    assert_eq!(
        response::unpack(&native, "qwen")
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        5
    );
    let roundtrip = response::from_openai(&chat, true, true).unwrap();
    assert_eq!(roundtrip["output"], native["output"]);
    assert_eq!(roundtrip["usage"], native["usage"]);
    let mut without_usage = chat.clone();
    without_usage.as_object_mut().unwrap().remove("usage");
    assert!(response::from_openai(&without_usage, false, true)
        .unwrap()
        .get("usage")
        .is_none());
}

#[test]
fn dashscope_rejects_provider_owned_state() {
    let mut req = openai::normalize_chat_completions(
        json!({"model":"fixture","messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    req.previous_response_id = Some("provider-response".into());
    assert!(pack(&req, "qwen", false, false).is_err());
}

#[test]
fn dashscope_rejects_invalid_and_lossy_output() {
    assert!(normalize(
        json!({"model":"qwen","input":{},"parameters":[]}),
        false,
        false
    )
    .is_err());
    assert!(response::to_openai(&json!({"code":"InvalidApiKey"}), "qwen", false).is_err());
    let native = json!({"output":{"choices":[{"message":{"content":[{"ocr_result":{"x":1}}]}}]}});
    assert!(response::to_openai(&native, "qwen", false).is_err());
    assert_eq!(response::from_openai(&native, true, true).unwrap(), native);
}

#[tokio::test]
async fn dashscope_fragmented_stream_both_directions() {
    let event = "data: {\"output\":{\"text\":\"你好\",\"finish_reason\":\"stop\"},\"usage\":{\"input_tokens\":2,\"output_tokens\":1}}\n\n";
    let chunks = event
        .as_bytes()
        .chunks(3)
        .map(|s| Ok::<_, StreamError<std::io::Error>>(bytes::Bytes::copy_from_slice(s)))
        .collect::<Vec<_>>();
    let translated = stream::translate(
        futures::stream::iter(chunks),
        "qwen".into(),
        false,
        false,
        true,
        true,
    );
    let result = translated.collect::<Vec<_>>().await;
    let text = String::from_utf8(
        result
            .into_iter()
            .flat_map(|r| r.unwrap().to_vec())
            .collect(),
    )
    .unwrap();
    assert!(text.contains("你好"));
    assert!(text.contains("prompt_tokens"));
    let source = futures::stream::iter(vec![Ok::<_, StreamError<std::io::Error>>(
        bytes::Bytes::from(text),
    )]);
    let result = stream::translate(source, "qwen".into(), true, false, true, true)
        .collect::<Vec<_>>()
        .await;
    let bytes = result
        .into_iter()
        .flat_map(|r| r.unwrap().to_vec())
        .collect::<Vec<_>>();
    assert!(String::from_utf8(bytes).unwrap().contains("event: result"));
}

#[test]
fn dashscope_stream_error_is_typed() {
    let _: StreamError<std::io::Error> = ProtocolStreamError::invalid_data("bad frame").into();
}

#[test]
fn dashscope_unknown_message_fields_and_text_tools_are_not_lost() {
    let req = normalize(json!({"model":"qwen","input":{"messages":[{"role":"user","content":"hi","native_extension":true}]}}),false,false).unwrap();
    assert!(validate_bridge(&req).is_err());
    let chat = json!({"choices":[{"message":{"tool_calls":[{"id":"tool_1"}]},"finish_reason":"tool_calls"}]});
    assert!(response::from_openai(&chat, false, false).is_err());
    assert!(response::from_openai(&chat, false, true).is_ok());
}

#[tokio::test]
async fn dashscope_native_stream_preserves_sse_envelope_and_errors() {
    let wire = "id: 7\nretry: 1000\nevent: error\ndata: {\"code\":\"InvalidParameter\",\"message\":\"fixture\"}\n\n";
    let source = futures::stream::iter(
        wire.as_bytes()
            .chunks(2)
            .map(|s| Ok::<_, StreamError<std::io::Error>>(bytes::Bytes::copy_from_slice(s)))
            .collect::<Vec<_>>(),
    );
    let output = ingress_stream::native_or_translate(source, "qwen".into(), false, true, true)
        .collect::<Vec<_>>()
        .await;
    let bytes = output
        .into_iter()
        .flat_map(|r| r.unwrap().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(bytes, wire.as_bytes());
}

#[tokio::test]
async fn dashscope_cumulative_tools_and_final_usage_are_preserved() {
    let bodies = [
        json!({"choices":[{"index":0,"delta":{"content":"A","tool_calls":[{"index":0,"id":"t1","function":{"name":"weather","arguments":"{\"city\":"}}]},"finish_reason":null}]}),
        json!({"choices":[{"index":0,"delta":{"content":"B","tool_calls":[{"index":0,"function":{"arguments":"\"北京\"}"}}]},"finish_reason":"tool_calls"}]}),
        json!({"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":3,"total_tokens":10}}),
    ];
    let source = futures::stream::iter(bodies.into_iter().map(|v| {
        Ok::<_, StreamError<std::io::Error>>(bytes::Bytes::from(format!("data: {v}\n\n")))
    }));
    let output = stream::translate(source, "qwen".into(), true, false, true, false)
        .collect::<Vec<_>>()
        .await;
    let last = String::from_utf8(output.last().unwrap().as_ref().unwrap().to_vec()).unwrap();
    let json: Value =
        serde_json::from_str(last.strip_prefix("event: result\ndata: ").unwrap().trim()).unwrap();
    assert_eq!(json["output"]["choices"][0]["message"]["content"], "AB");
    assert_eq!(
        json["output"]["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        "{\"city\":\"北京\"}"
    );
    assert_eq!(json["usage"]["total_tokens"], 10);
}

#[tokio::test]
async fn dashscope_truncated_stream_fails() {
    let source = futures::stream::iter(vec![Ok::<_, StreamError<std::io::Error>>(
        bytes::Bytes::from_static(
            b"data: {\"output\":{\"text\":\"partial\",\"finish_reason\":\"null\"}}\n\n",
        ),
    )]);
    let output = stream::translate(source, "qwen".into(), false, false, true, true)
        .collect::<Vec<_>>()
        .await;
    assert!(output.last().unwrap().is_err());
}
