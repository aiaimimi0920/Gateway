use neuro_gateway::protocol::{
    openai::{build_chat_completions_success, unpack_openai_response},
    responses::{build_responses_success, unpack_responses_response},
};
use serde_json::json;

#[test]
fn buffered_responses_custom_call_keeps_input_identity_and_namespace() {
    let upstream = json!({"model":"upstream","status":"completed","output":[
        {"type":"custom_tool_call","id":"item-1","call_id":"call-1","name":"shell","input":"echo '&amp;'","namespace":"ops"}]});
    let response = unpack_responses_response(&upstream).unwrap();
    let chat = build_chat_completions_success(
        "reply",
        0,
        "alias",
        &response.text,
        None,
        &response.tool_calls,
        response.finish_reason.as_deref(),
    );
    assert_eq!(
        chat["choices"][0]["message"]["tool_calls"][0],
        json!({"id":"call-1","type":"custom","custom":{"name":"shell","input":"echo '&amp;'"}})
    );
    let native = build_responses_success(
        "reply",
        "alias",
        &response.text,
        None,
        &response.tool_calls,
        response.finish_reason.as_deref(),
    );
    assert_eq!(native["output"][0]["input"], "echo '&amp;'");
    assert_eq!(native["output"][0]["namespace"], "ops");
    assert_eq!(native["output"][0]["id"], "item-1");
    assert_eq!(native["output"][0]["call_id"], "call-1");
}

#[test]
fn buffered_chat_custom_call_is_not_rewritten_as_a_function() {
    let upstream = json!({"model":"upstream","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[
        {"id":"call-1","type":"custom","custom":{"name":"shell","input":"a &quot; b"}}]},"finish_reason":"tool_calls"}]});
    let response = unpack_openai_response(&upstream).unwrap();
    let output = build_responses_success(
        "reply",
        "alias",
        &response.text,
        None,
        &response.tool_calls,
        response.finish_reason.as_deref(),
    );
    assert_eq!(output["output"][0]["type"], "custom_tool_call");
    assert_eq!(output["output"][0]["input"], "a &quot; b");
    assert_eq!(output["output"][0]["call_id"], "call-1");
}
