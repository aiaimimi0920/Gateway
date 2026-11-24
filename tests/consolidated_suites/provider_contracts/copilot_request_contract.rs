use neuro_gateway::error::{FallbackHint, GatewayError};
use neuro_gateway::protocol::canonical::{
    CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use neuro_gateway::protocol::copilot::single_turn;
use neuro_gateway::protocol::{openai::normalize_chat_completions, responses::normalize_responses};
use serde_json::{json, Value};

fn chat() -> Value {
    json!({"model":"public-alias", "messages":[{"role":"user","content":"你好"}]})
}

#[test]
fn accepts_real_normalizers_and_preserves_selected_model_and_filtered_text() {
    let bodies = [
        chat(),
        json!({"messages":[{"role":"user","content":[{"type":"text","text":"你好"}]}],"stream":false}),
        json!({"input":"你好"}),
        json!({"input":[{"type":"message","role":"user","content":"你好"}]}),
        json!({"input":[{"role":"user","content":[{"type":"input_text","text":"你好"}]}]}),
    ];
    for body in bodies {
        let mut request = normalize(body).unwrap();
        let packed = single_turn(&request, "selected-model").unwrap();
        assert_eq!(packed.model, "selected-model");
        assert_eq!(packed.prompt, "你好");
        request.messages[0].content = vec![ContentPart::Text {
            text: "filtered".into(),
        }];
        assert_eq!(
            single_turn(&request, "selected-model").unwrap().prompt,
            "filtered"
        );
    }
}

fn normalize(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    if body.get("input").is_some() {
        normalize_responses(body)
    } else {
        normalize_chat_completions(body)
    }
}

fn rejects(body: Value) {
    if let Ok(request) = normalize(body) {
        let error = single_turn(&request, "selected-model").unwrap_err();
        assert!(!error.retryable);
        assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
    }
}

#[test]
fn rejects_unsupported_top_level_options_even_when_normalizer_discards_them() {
    for base in [chat(), json!({"input":"hello"})] {
        for key in [
            "tools",
            "tool_choice",
            "reasoning",
            "metadata",
            "user",
            "previous_response_id",
            "instructions",
            "temperature",
            "max_tokens",
            "max_output_tokens",
            "store",
            "session_id",
            "unknown",
        ] {
            let mut body = base.clone();
            body[key] = Value::Null;
            rejects(body);
        }
        for stream in [json!(true), json!("false"), Value::Null, json!(0)] {
            let mut body = base.clone();
            body["stream"] = stream;
            rejects(body);
        }
    }
}

#[test]
fn rejects_nested_options_history_tools_and_media_without_lossy_projection() {
    for content in [
        Value::Null,
        json!([]),
        json!([{"type":"text","text":"hello","cache_control":{}}]),
        json!([{"type":"text","text":"a"},{"type":"text","text":"b"}]),
        json!([{"type":"image_url","image_url":{"url":"https://example.invalid"}}]),
    ] {
        let mut body = chat();
        body["messages"][0]["content"] = content;
        rejects(body);
    }
    for key in ["name", "tool_calls", "tool_call_id", "refusal", "unknown"] {
        let mut body = chat();
        body["messages"][0][key] = Value::Null;
        rejects(body);
    }
    rejects(json!({"input":[{"role":"user","content":"hello","id":"foreign"}]}));
    rejects(
        json!({"input":[{"role":"user","content":[{"type":"input_text","text":"hello","unknown":true}]}]}),
    );
    rejects(json!({"input":[{"role":"assistant","content":"hello"}]}));
    rejects(json!({"input":[{"role":"user","content":"a"},{"role":"user","content":"b"}]}));
    rejects(json!({"input":[{"type":"function_call_output","call_id":"x","output":"hello"}]}));
}

#[test]
fn bounds_are_bytes_and_errors_do_not_echo_inputs() {
    for text in [" ".into(), "a".repeat(65537), "你".repeat(21846)] {
        rejects(json!({"input":text}));
    }
    let request = normalize(json!({"input":"a".repeat(65536)})).unwrap();
    assert!(single_turn(&request, &"m".repeat(512)).is_ok());
    for model in [" ".into(), "m".repeat(513)] {
        assert!(single_turn(&request, &model).is_err());
    }
    let mut request = normalize(chat()).unwrap();
    request.raw_body = Value::Null;
    let error = single_turn(&request, "secret-model-sentinel").unwrap_err();
    assert!(!format!("{error:?}").contains("secret-model-sentinel"));
}

#[test]
fn rejects_canonical_mutations_independently_of_raw_body() {
    let original = normalize(chat()).unwrap();
    let mutations: &[fn(&mut CanonicalRelayRequest)] = &[
        |r| r.stream = true,
        |r| r.protocol_family = ProtocolFamily::Anthropic,
        |r| r.endpoint_kind = EndpointKind::Messages,
        |r| r.messages[0].role = MessageRole::System,
        |r| r.messages[0].name = Some("name".into()),
        |r| r.messages[0].tool_call_id = Some("id".into()),
        |r| r.messages.clear(),
        |r| r.reasoning = Some(Value::Null),
        |r| r.tool_choice = Some(Value::Null),
        |r| r.metadata = Some(Default::default()),
        |r| r.previous_response_id = Some("id".into()),
        |r| r.explicit_session_key = Some("id".into()),
        |r| {
            r.extra.insert("unknown".into(), Value::Null);
        },
    ];
    for mutate in mutations {
        let mut request = original.clone();
        mutate(&mut request);
        assert!(single_turn(&request, "model").is_err());
    }
}
