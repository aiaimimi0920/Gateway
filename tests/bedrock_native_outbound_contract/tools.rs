use super::fixture::{decode, output, sse};
use serde_json::json;

#[tokio::test]
async fn native_tools_wait_for_late_identity_and_replay_early_input_exactly_once() {
    let wire = sse(&[
        json!({"choices":[{"delta":{"tool_calls":[{"index":7,"function":{"arguments":"{\"city\":\""}}]}}]}),
        json!({"choices":[{"delta":{"content":"calling"}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":7,"id":"call-a","function":{"arguments":"雪"}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":7,"function":{"name":"lookup","arguments":"\"}"}}]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}),
    ]);
    let frames = output(&wire, 1).await;
    let events: Vec<_> = frames.iter().map(|v| decode(v.as_ref().unwrap())).collect();
    let start = events
        .iter()
        .position(|v| v.0 == "contentBlockStart")
        .unwrap();
    assert_eq!(events[start].1["contentBlockIndex"], 1);
    assert!(events[..start]
        .iter()
        .all(|v| v.1.pointer("/delta/toolUse").is_none()));
    let inputs: Vec<_> = events
        .iter()
        .filter_map(|v| v.1.pointer("/delta/toolUse/input").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(inputs.concat(), "{\"city\":\"雪\"}");
    assert_eq!(
        events.iter().filter(|v| v.0 == "contentBlockStart").count(),
        1
    );
}

#[tokio::test]
async fn native_tools_keep_interleaved_identity_and_wire_indices_separate() {
    let wire = sse(&[
        json!({"choices":[{"delta":{"tool_calls":[
            {"index":9,"id":"call-a","function":{"name":"first","arguments":"{\"a\":"}},
            {"index":4,"id":"call-b","function":{"name":"second","arguments":"["}}
        ]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[
            {"index":4,"function":{"arguments":"2]"}}, {"index":9,"function":{"arguments":"1}"}}
        ]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}),
    ]);
    let frames = output(&wire, 7).await;
    let events: Vec<_> = frames.iter().map(|v| decode(v.as_ref().unwrap())).collect();
    for (index, id, expected) in [(0, "call-a", "{\"a\":1}"), (1, "call-b", "[2]")] {
        let start = events
            .iter()
            .find(|v| v.0 == "contentBlockStart" && v.1["contentBlockIndex"] == index)
            .unwrap();
        assert_eq!(start.1["start"]["toolUse"]["toolUseId"], id);
        let input: String = events
            .iter()
            .filter(|v| v.1["contentBlockIndex"] == index)
            .filter_map(|v| v.1.pointer("/delta/toolUse/input").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(input, expected);
    }
    assert_eq!(
        events.iter().filter(|v| v.0 == "contentBlockStop").count(),
        2
    );
}

#[tokio::test]
async fn native_tools_without_arguments_normalize_to_an_empty_json_object() {
    for arguments in [None, Some(""), Some(" \n ")] {
        let mut call = json!({"index":0,"id":"call-a","function":{"name":"lookup"}});
        if let Some(value) = arguments {
            call["function"]["arguments"] = json!(value);
        }
        let wire = sse(&[
            json!({"choices":[{"delta":{"tool_calls":[call]}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],
                "usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}}),
        ]);
        let frames = output(&wire, 1).await;
        let events: Vec<_> = frames.iter().map(|v| decode(v.as_ref().unwrap())).collect();
        let input: String = events
            .iter()
            .filter_map(|v| v.1.pointer("/delta/toolUse/input").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&input).unwrap(),
            json!({})
        );
    }
}

#[tokio::test]
async fn native_blank_optional_identity_is_absence_until_the_real_identity_arrives() {
    let wire = sse(&[
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":" ",
            "function":{"name":"\t","arguments":null}}]}}]}),
        json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-a",
            "function":{"name":"lookup","arguments":"{}"}}]}}]}),
        json!({"choices":[{"delta":{},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":0,"completion_tokens":0,"total_tokens":0}}),
    ]);
    let frames = output(&wire, 1).await;
    let start = frames
        .iter()
        .map(|v| decode(v.as_ref().unwrap()))
        .find(|v| v.0 == "contentBlockStart")
        .unwrap()
        .1;
    assert_eq!(
        start["start"]["toolUse"],
        json!({"toolUseId":"call-a","name":"lookup"})
    );
}
