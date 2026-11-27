use super::*;
use serde_json::json;

#[test]
fn cursor_remote_text_output_parts_preserve_bytes_and_enforce_total_bound() {
    assert_eq!(
        parse(
            &json!([
                {"type":"input_text","text":" \r\n"},{"type":"input_text","text":"中&amp;\t"}
            ]),
            "input_text"
        )
        .unwrap(),
        " \r\n中&amp;\t"
    );
    assert_eq!(parse(&json!([]), "input_text").unwrap(), "");
    assert_eq!(parse(&json!(""), "input_text").unwrap(), "");
    let exact = json!([{"type":"input_text","text":"a".repeat(32768)},
        {"type":"input_text","text":"b".repeat(32768)}]);
    assert_eq!(parse(&exact, "input_text").unwrap().len(), 65536);
    for value in [
        json!([{"type":"text","text":"wrong alias"}]),
        json!([{"type":"input_text","text":"ok","extra":false}]),
        json!([{"type":"input_text","text":null}]),
        json!([{"type":"input_image","image_url":"x"}]),
        json!([{"type":"input_text","text":"a".repeat(32768)},
            {"type":"input_text","text":"b".repeat(32769)}]),
        json!((0..129)
            .map(|_| json!({"type":"input_text","text":""}))
            .collect::<Vec<_>>()),
        json!("a".repeat(65537)),
    ] {
        assert!(parse(&value, "input_text").is_err());
    }
}

#[test]
fn cursor_remote_array_and_string_results_have_identical_continuation_history() {
    use crate::protocol::cursor_remote::tool_request;
    let base = json!({"model":"cursor-personal","tools":[{
        "type":"function","name":"echo","parameters":{"type":"object"}}],
        "input":[{"role":"user","content":"hello"},
            {"type":"function_call","name":"echo","call_id":"call_x","arguments":"{}"},
            {"type":"function_call_output","call_id":"call_x","output":" \r\n中"}]});
    let a = tool_request::parse(
        &crate::protocol::responses::normalize_responses(base.clone()).unwrap(),
    )
    .unwrap();
    let mut parts = base;
    parts["input"][2]["output"] = json!([
        {"type":"input_text","text":" \r\n"},{"type":"input_text","text":"中"}]);
    let b = tool_request::parse(&crate::protocol::responses::normalize_responses(parts).unwrap())
        .unwrap();
    assert_eq!(a.history, b.history);
    assert_eq!(a.signature, b.signature);
    assert_eq!(a.output.unwrap().text, b.output.unwrap().text);
}

#[test]
fn cursor_remote_tool_result_ipc_checks_escaped_size_before_live_claim() {
    assert!(check_ipc("call", &"x".repeat(65536)).is_ok());
    assert!(parse(&json!("\0".repeat(22000)), "input_text").is_ok());
    assert!(check_ipc("call", &"\0".repeat(22000)).is_err());
    let base = json!({"model":"cursor-personal","tools":[{
        "type":"function","name":"echo","parameters":{"type":"object"}}],
        "input":[{"role":"user","content":"hello"},
            {"type":"function_call","name":"echo","call_id":"call","arguments":"{}"},
            {"type":"function_call_output","call_id":"call","output":"\0".repeat(22000)}]});
    assert!(crate::protocol::cursor_remote::tool_request::parse(
        &crate::protocol::responses::normalize_responses(base).unwrap(),
    )
    .is_err());
}
