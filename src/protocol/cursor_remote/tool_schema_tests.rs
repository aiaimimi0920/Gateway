use super::*;
use crate::protocol::cursor_remote::tool_request::{parse, ToolRequest};

fn request(tools: Value, input: Value) -> Result<ToolRequest, GatewayError> {
    parse(
        &crate::protocol::responses::normalize_responses(json!({
            "model":"cursor-personal","store":false,"input":input,"tools":tools,
        }))
        .unwrap(),
    )
}

#[test]
fn cursor_remote_custom_text_definition_is_mapped_without_changing_its_identity() {
    let original = json!([{"type":"custom","name":"edit","description":"原始文本",
        "format":{"type":"text"}}]);
    let req = request(original.clone(), json!("hello")).unwrap();
    assert_eq!(req.tools[0].kind, ToolKind::CustomText);
    assert_eq!(req.tools[0].wire["name"], "edit");
    assert_eq!(req.tools[0].wire["description"], "原始文本");
    assert_eq!(
        req.tools[0].wire["parameters"]["required"],
        json!(["input"])
    );
    assert_eq!(
        req.tools[0].wire["parameters"]["additionalProperties"],
        false
    );
    let default = request(json!([{"type":"custom","name":"edit"}]), json!("hello")).unwrap();
    assert_eq!(default.tools[0].kind, ToolKind::CustomText);
    // Original declarations, not the MCP projection, bind every continuation.
    assert_ne!(req.signature, default.signature);
    let mut changed = original;
    changed[0]["description"] = json!("different");
    assert_ne!(
        req.signature,
        request(changed, json!("hello")).unwrap().signature
    );
}

#[test]
fn cursor_remote_custom_text_rejects_grammar_strict_unknown_fields_and_duplicate_names() {
    for tool in [
        json!({"type":"custom","name":"edit","format":{"type":"grammar","syntax":"lark","definition":"start: /.+/"}}),
        json!({"type":"custom","name":"edit","format":null}),
        json!({"type":"custom","name":"edit","format":{"type":"text","grammar":"ignored"}}),
        json!({"type":"custom","name":"edit","parameters":{"type":"object"}}),
        json!({"type":"custom","name":"edit","strict":true}),
        json!({"type":"custom","name":"edit","async":true}),
        json!({"type":"custom","name":"-bad"}),
    ] {
        assert!(request(json!([tool]), json!("hello")).is_err());
    }
    assert!(request(
        json!([
            {"type":"custom","name":"edit"},
            {"type":"function","name":"edit","parameters":{"type":"object"}},
        ]),
        json!("hello")
    )
    .is_err());
}

#[test]
fn cursor_remote_custom_text_native_envelope_is_exact_and_inner_text_is_opaque() {
    for input in ["", " \r\n补丁\t\"<&amp;>\"\n ", "{\"not\":\"arguments\"}"] {
        let native = json!({"input":input}).to_string();
        assert_eq!(ToolKind::CustomText.native_input(native).unwrap(), input);
    }
    for native in [
        "{}",
        "{\"input\":1}",
        "{\"input\":null}",
        "{\"input\":{}}",
        "{\"input\":\"a\",\"extra\":\"b\"}",
        "{\"input\":\"a\",\"input\":\"b\"}",
        "\"plain text\"",
    ] {
        assert!(
            ToolKind::CustomText.native_input(native.into()).is_err(),
            "{native}"
        );
    }
}

#[test]
fn cursor_remote_custom_history_keeps_input_call_id_and_output_type() {
    let tools = json!([{"type":"custom","name":"edit"}]);
    let input = " \r\n补丁\t\"<&amp;>\"\n ";
    let history = json!([
        {"role":"user","content":"hello"},
        {"type":"custom_tool_call","id":"item_not_call","call_id":"call_edit","name":"edit","input":input},
        {"type":"custom_tool_call_output","id":"output_not_call","call_id":"call_edit","output":" \n结果\n "},
    ]);
    let req = request(tools.clone(), history.clone()).unwrap();
    assert_eq!(req.history[1]["input"], input);
    assert!(req.history[1].get("arguments").is_none());
    let output = req.output.unwrap();
    assert_eq!(output.call_id, "call_edit");
    assert_eq!(output.kind, ToolKind::CustomText);
    assert_eq!(output.kind.output_type(), "custom_tool_call_output");
    assert_eq!(output.text, " \n结果\n ");
    assert!(req.prompt.is_empty());
    for bad in [
        json!({"type":"function_call","name":"edit","call_id":"call_edit","arguments":"{}"}),
        json!({"type":"custom_tool_call","name":"edit","call_id":" ","input":input}),
        json!({"type":"custom_tool_call","name":"edit","call_id":"call_edit","input":input,"arguments":"{}"}),
    ] {
        let mut altered = history.clone();
        altered[1] = bad;
        assert!(request(tools.clone(), altered).is_err());
    }
    let mut unsupported = history;
    unsupported
        .as_array_mut()
        .unwrap()
        .push(json!({"role":"user","content":"new task"}));
    assert!(request(tools, unsupported).is_err());
}

#[test]
fn cursor_remote_tool_history_defaults_only_absent_message_type() {
    for tools in [
        json!([{"type":"function","name":"echo","parameters":{"type":"object"}}]),
        json!([{"type":"custom","name":"edit"}]),
    ] {
        let message = json!({"role":"user","content":"hello"});
        let implicit = request(tools.clone(), json!([message.clone()])).unwrap();
        let mut explicit = message.clone();
        explicit["type"] = json!("message");
        let explicit = request(tools.clone(), json!([explicit])).unwrap();
        assert_eq!(implicit.history, explicit.history);
        assert_eq!(implicit.prompt, explicit.prompt);
        for bad_type in [
            Value::Null,
            json!(0),
            json!(false),
            json!([]),
            json!({}),
            json!(""),
            json!("unknown"),
        ] {
            let mut malformed = message.clone();
            malformed["type"] = bad_type;
            let error = request(tools.clone(), json!([malformed])).unwrap_err();
            assert_eq!(
                error.code.as_deref(),
                Some("cursor_remote_unsupported_tool_request")
            );
        }
        for (key, value) in [
            ("role", Value::Null),
            ("content", json!(1)),
            ("id", Value::Null),
            ("id", json!("")),
        ] {
            let mut malformed = message.clone();
            malformed[key] = value;
            assert!(request(tools.clone(), json!([malformed])).is_err());
        }
    }
}
