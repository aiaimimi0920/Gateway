//! Service output must split large deltas without applying these limits to P04 client decoding.
#[path = "bedrock_native_outbound_contract/fixture.rs"]
mod fixture;
use fixture::{decode, output, sse};
use serde_json::json;

#[tokio::test]
async fn service_output_splits_large_utf8_and_json_escaped_text_without_losing_content() {
    for text in ["雪".repeat(10 * 1024 * 1024), "\0".repeat(5 * 1024 * 1024)] {
        let wire = sse(&[
            json!({"choices":[{"delta":{"content":text}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
        ]);
        let frames = output(&wire, wire.len()).await;
        let mut actual = String::new();
        for frame in &frames {
            let frame = frame.as_ref().unwrap();
            let headers = u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize;
            assert!(headers <= 128 * 1024);
            assert!(frame.len() - headers - 16 <= 24 * 1024 * 1024,
                "Gateway is the service encoder, so its emitted payload must respect Smithy's service limit");
            let (kind, body) = decode(frame);
            if kind == "contentBlockDelta" {
                actual.push_str(body["delta"]["text"].as_str().unwrap());
            }
        }
        assert_eq!(actual, text);
    }
}

#[tokio::test]
async fn nullable_optional_tool_delta_fields_do_not_replace_a_declared_identity() {
    let wire = sse(&[
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-a","type":"function",
            "function":{"name":"lookup","arguments":"{"}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":null,"type":null,
            "function":{"name":null,"arguments":"}"}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":null}]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
    ]);
    let frames = output(&wire, 7).await;
    assert!(
        frames.iter().all(Result::is_ok),
        "optional null fields are absence, not new identities"
    );
    assert_eq!(
        decode(frames.last().unwrap().as_ref().unwrap()).0,
        "metadata"
    );
}

#[tokio::test]
async fn native_tool_identities_obey_bedrock_patterns_and_length_without_silent_renaming() {
    for (id, name, valid) in [
        ("a".repeat(64), "n".repeat(64), true),
        ("a.b:c-d_e".into(), "n-a_m-e".into(), true),
        ("a".repeat(65), "valid".into(), false),
        ("valid".into(), "n".repeat(65), false),
        ("has space".into(), "valid".into(), false),
        ("valid".into(), "has.dot".into(), false),
        ("雪".into(), "valid".into(), false),
    ] {
        let wire = sse(&[
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":id,
                "function":{"name":name,"arguments":"{}"}}]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],
                "usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}}),
        ]);
        let frames = output(&wire, wire.len()).await;
        assert_eq!(
            frames.iter().all(Result::is_ok),
            valid,
            "id={id:?}, name={name:?}"
        );
        if valid {
            let start = frames
                .iter()
                .map(|v| decode(v.as_ref().unwrap()))
                .find(|v| v.0 == "contentBlockStart")
                .unwrap()
                .1;
            assert_eq!(start["start"]["toolUse"]["toolUseId"], id);
            assert_eq!(start["start"]["toolUse"]["name"], name);
        }
    }
}
