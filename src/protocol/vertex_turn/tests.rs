use super::*;

fn value() -> Value {
    json!({"source":"vertex_express_api","model":"model","parts":[
        {"text":"summary","thought":true,"thought_signature":"YWJj"},
        {"text":"A","thought":false},{"function_call":{"name":"lookup","args":{}},"public_id":"c1"},
        {"text":"","thought_signature":"ZGVm"}]})
}
#[test]
fn vertex_express_signed_schema_roundtrips_google_parts_and_redacts_debug() {
    let turn = wire::read_turn(&value()).unwrap();
    assert_eq!(turn.visible_text(), "A");
    assert_eq!(
        turn.google_parts(),
        json!([
        {"text":"summary","thought":true,"thoughtSignature":"YWJj"},{"text":"A","thought":false},
        {"functionCall":{"name":"lookup","args":{}}},{"text":"","thoughtSignature":"ZGVm"}])
    );
    let debug = format!("{turn:?}");
    assert!(!debug.contains("YWJj") && !debug.contains("ZGVm"));
    assert_eq!(serde_json::to_value(turn).unwrap(), value());
}
#[test]
fn vertex_express_signed_schema_rejects_unknown_fields_ambiguous_parts_and_invalid_base64() {
    let mut mutations = Vec::new();
    let mut v = value();
    v["source"] = json!("aistudio");
    mutations.push(v);
    let mut v = value();
    v["extra"] = json!("not-allowed");
    mutations.push(v);
    let mut v = value();
    v["parts"][0]["extra"] = json!(1);
    mutations.push(v);
    let mut v = value();
    v["parts"][0]["function_call"] = json!({"name":"lookup","args":{}});
    mutations.push(v);
    let mut v = value();
    v["parts"][2]["function_call"]["args"] = json!([]);
    mutations.push(v);
    let mut v = value();
    v["parts"][2]["thought"] = json!(true);
    mutations.push(v);
    let mut v = value();
    v["parts"][2]["public_id"] = json!("");
    mutations.push(v);
    let mut v = value();
    let duplicate = v["parts"][2].clone();
    v["parts"].as_array_mut().unwrap().push(duplicate);
    mutations.push(v);
    for sig in [
        "!private",
        "skip_thought_signature_validator",
        "context_engineering_is_the_way_to_go",
        "A",
    ] {
        let mut v = value();
        v["parts"][0]["thought_signature"] = json!(sig);
        mutations.push(v);
    }
    for name in ["text", "thought", "thought_signature", "function_call"] {
        let mut v = value();
        v["parts"][0][name] = Value::Null;
        mutations.push(v);
    }
    for v in mutations {
        let error = wire::read_turn(&v).unwrap_err();
        assert!(!error.retryable);
        assert_eq!(error.message, INVALID);
    }
}
#[test]
fn vertex_express_signed_schema_bounds_parts_signature_and_total_bytes() {
    let mut v = value();
    v["parts"] = json!(vec![json!({"text":""}); 4097]);
    assert!(wire::read_turn(&v).is_err());
    let mut v = value();
    v["parts"][0]["thought_signature"] = json!("Y".repeat(1024 * 1024 + 1));
    assert!(wire::read_turn(&v).is_err());
    let mut v = value();
    v["parts"][0]["text"] = json!("a".repeat(MAX_TURN_BYTES));
    assert!(wire::read_turn(&v).is_err());
    assert!(wire::within_budget(&json!({"x":"abc"}), 3).is_err());
}
#[test]
fn vertex_express_signed_schema_preserves_base64_alphabets_and_padding_verbatim() {
    for sig in ["YWJj", "YQ==", "YQ", "+/8=", "-_8=", ""] {
        let mut v = value();
        v["parts"][0]["thought_signature"] = json!(sig);
        assert_eq!(
            wire::read_turn(&v).unwrap().google_parts()[0]["thoughtSignature"],
            sig
        );
    }
}
#[test]
fn vertex_express_signed_schema_terminal_slot_rejects_early_duplicate_or_nonterminal_metadata() {
    let mut terminal = json!({"choices":[{"delta":{},"finish_reason":"stop"}]});
    terminal[WIRE_FIELD] = value();
    let mut slot = wire::TerminalTurn::default();
    slot.accept(&json!({"choices":[{"delta":{"content":"A","tool_calls":[{"index":0,"id":"c1","function":{"name":"lookup","arguments":"{}"}}]}}]})).unwrap();
    slot.accept(&terminal).unwrap();
    assert!(slot.accept(&terminal).is_err());
    for bad in [
        json!({"choices":[{"delta":{},"finish_reason":null}]}),
        json!({"choices":[{"delta":{"content":"late"},"finish_reason":"stop"}]}),
        json!({}),
    ] {
        let mut bad = bad;
        bad[WIRE_FIELD] = value();
        assert!(wire::TerminalTurn::default().accept(&bad).is_err());
    }
}
#[test]
fn vertex_express_signed_schema_legacy_canonical_without_extension_remains_unchanged() {
    let value = json!({"model":"m","text":"a","usage":null,"tool_calls":[],"upstream_status":200,"finish_reason":"stop"});
    let response: super::super::canonical::CanonicalRelayResponse =
        serde_json::from_value(value).unwrap();
    assert!(response.vertex_turn.is_none());
    assert!(serde_json::to_value(response)
        .unwrap()
        .get("vertex_turn")
        .is_none());
}

#[test]
fn vertex_express_signed_json_parser_preserves_entities_and_literal_tool_markup() {
    let literal="<tool_calls><tool_call><tool_name>lookup</tool_name><parameters>{}</parameters></tool_call></tool_calls>";
    let turn = json!({"source":"vertex_express_api","model":"model","parts":[{"text":literal,"thought_signature":"YWJj"}]});
    let mut body =
        json!({"model":"model","choices":[{"message":{"content":literal},"finish_reason":"stop"}]});
    body[WIRE_FIELD] = turn;
    let parsed = super::super::openai::unpack_openai_response(&body).unwrap();
    assert_eq!(parsed.text, literal);
    assert!(parsed.tool_calls.is_empty());
    let args = json!({"s":"&amp; &quot;"});
    body["choices"][0]["message"] = json!({"content":"","tool_calls":[{"id":"c1","type":"function","function":{"name":"lookup","arguments":args.to_string()}}]});
    body[WIRE_FIELD] = json!({"source":"vertex_express_api","model":"model","parts":[{"function_call":{"name":"lookup","args":args},"public_id":"c1","thought_signature":"YWJj"}]});
    let parsed = super::super::openai::unpack_openai_response(&body).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(parsed.tool_calls[0].arguments.as_ref().unwrap()).unwrap(),
        args
    );
    let mut legacy = body;
    legacy.as_object_mut().unwrap().remove(WIRE_FIELD);
    assert_ne!(
        super::super::openai::unpack_openai_response(&legacy)
            .unwrap()
            .tool_calls[0]
            .arguments
            .as_ref()
            .unwrap(),
        parsed.tool_calls[0].arguments.as_ref().unwrap()
    );
}
