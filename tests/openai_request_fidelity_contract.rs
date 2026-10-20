//! Exercise the public normalize -> canonical -> pack seams, not private helpers.
use neuro_gateway::protocol::{
    openai::{normalize_chat_completions, pack_openai},
    responses::{normalize_responses, pack_responses, pack_responses_bridge},
};
use serde_json::{json, Value};

fn responses(format: Value) -> Value {
    json!({"model":"alias","input":"hello","text":{"format":format}})
}

#[test]
fn structured_schema_round_trip_preserves_false_and_raw_schema() {
    let format = json!({"type":"json_schema","name":"result","description":"fixture",
        "strict":false,"schema":{"type":"object","properties":{"n":{"type":["number","null"],"default":null}},"additionalProperties":false}});
    let req = normalize_responses(responses(format.clone())).unwrap();
    let chat = pack_openai(&req, "upstream", false);
    assert_eq!(chat["response_format"]["json_schema"]["strict"], false);
    assert_eq!(
        chat["response_format"]["json_schema"]["schema"],
        format["schema"]
    );
    assert!(chat.get("text").is_none());
    let req = normalize_chat_completions(chat).unwrap();
    let result = pack_responses(&req, "upstream", false);
    assert_eq!(result["text"]["format"], format);
    assert!(result.get("response_format").is_none());
}

#[test]
fn simple_output_formats_are_translated_without_inventing_strictness() {
    for kind in ["json_object", "text"] {
        let req = normalize_responses(responses(json!({"type":kind}))).unwrap();
        let chat = pack_openai(&req, "upstream", false);
        assert_eq!(chat["response_format"], json!({"type":kind}));
        let req = normalize_chat_completions(chat).unwrap();
        assert_eq!(
            pack_responses(&req, "upstream", false)["text"]["format"],
            json!({"type":kind})
        );
    }
}

#[test]
fn responses_custom_tool_input_and_output_keep_their_wire_kind() {
    let body = json!({"model":"alias","input":[
        {"type":"custom_tool_call","id":"item-1","call_id":"call-1","name":"shell","input":"echo '&quot;'","namespace":"ops"},
        {"type":"custom_tool_call_output","call_id":"call-1","output":"ok"}],
        "tools":[{"type":"custom","name":"shell","description":"fixture","format":{"type":"text"}}],
        "tool_choice":{"type":"custom","name":"shell"}});
    let req = normalize_responses(body).unwrap();
    let packed = pack_responses(&req, "upstream", false);
    assert_eq!(packed["input"][0]["type"], "custom_tool_call");
    assert_eq!(packed["input"][0]["input"], "echo '&quot;'");
    assert_eq!(packed["input"][0]["namespace"], "ops");
    assert_eq!(packed["input"][1]["type"], "custom_tool_call_output");
    let chat = pack_openai(&req, "upstream", false);
    assert_eq!(chat["tools"][0]["custom"]["name"], "shell");
    assert_eq!(
        chat["messages"][0]["tool_calls"][0]["custom"]["input"],
        "echo '&quot;'"
    );
    assert_eq!(
        chat["tool_choice"],
        json!({"type":"custom","custom":{"name":"shell"}})
    );
}

#[test]
fn chat_custom_tool_and_function_strictness_survive_responses_round_trip() {
    let body = json!({"model":"alias","messages":[
        {"role":"assistant","content":null,"tool_calls":[{"id":"call-1","type":"custom","custom":{"name":"shell","input":"a &amp; b"}}]},
        {"role":"tool","tool_call_id":"call-1","content":"ok"}],
        "tools":[{"type":"custom","custom":{"name":"shell","format":{"type":"text"}}},
          {"type":"function","function":{"name":"fn","parameters":{"type":"object"},"strict":false}}]});
    let req = normalize_chat_completions(body).unwrap();
    let responses = pack_responses(&req, "upstream", false);
    assert_eq!(responses["input"][0]["type"], "custom_tool_call");
    assert_eq!(responses["input"][0]["input"], "a &amp; b");
    assert_eq!(responses["tools"][0]["name"], "shell");
    assert_eq!(responses["tools"][1]["strict"], false);
    let req = normalize_responses(responses).unwrap();
    let chat = pack_openai(&req, "upstream", false);
    assert_eq!(chat["tools"][1]["function"]["strict"], false);
    let assistant = chat["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "assistant")
        .unwrap();
    assert_eq!(assistant["tool_calls"][0]["custom"]["input"], "a &amp; b");
}

#[test]
fn allowed_tools_is_a_subset_not_an_expansion_of_the_tool_catalog() {
    let body = json!({"model":"alias","input":"hello","tools":[
        {"type":"function","name":"read","parameters":{}},
        {"type":"function","name":"write","parameters":{}}],
        "tool_choice":{"type":"allowed_tools","mode":"required","tools":[{"type":"function","name":"read"}]}});
    let req = normalize_responses(body).unwrap();
    let chat = pack_openai(&req, "upstream", false);
    assert_eq!(
        chat["tool_choice"],
        json!({"type":"allowed_tools","allowed_tools":{"mode":"required","tools":[{"type":"function","function":{"name":"read"}}]}})
    );
    assert_eq!(chat["tools"].as_array().unwrap().len(), 2);
    let req = normalize_chat_completions(chat).unwrap();
    assert_eq!(
        pack_responses(&req, "upstream", false)["tool_choice"]["tools"],
        json!([{"type":"function","name":"read"}])
    );
}

#[test]
fn foreign_tool_wire_fields_are_not_added_to_openai_function_definitions() {
    let body = json!({"model":"fixture","max_tokens":32,"messages":[{"role":"user","content":"hello"}],
        "tools":[{"name":"read","input_schema":{"type":"object"},"cache_control":{"type":"ephemeral"}}]});
    let req = neuro_gateway::protocol::anthropic::normalize_messages(body).unwrap();
    let chat = pack_openai(&req, "upstream", false);
    let function = &chat["tools"][0]["function"];
    assert_eq!(function["name"], "read");
    assert_eq!(function["parameters"], json!({"type":"object"}));
    assert!(function.get("input_schema").is_none());
    assert!(function.get("cache_control").is_none());
}

#[test]
fn verbosity_uses_target_layout_without_leaking_responses_only_parameters() {
    for format in [None, Some(json!({"type":"json_object"}))] {
        let mut body = json!({"model":"alias","input":"hello","text":{"verbosity":"low"},
            "include":["reasoning.encrypted_content"]});
        if let Some(format) = format {
            body["text"]["format"] = format;
        }
        let req = normalize_responses(body).unwrap();
        let chat = pack_openai(&req, "upstream", false);
        assert_eq!(chat["verbosity"], "low");
        assert!(chat.get("text").is_none());
        assert!(chat.get("include").is_none());
        let req = normalize_chat_completions(chat).unwrap();
        let result = pack_responses(&req, "upstream", false);
        assert_eq!(result["text"]["verbosity"], "low");
        assert!(result.get("verbosity").is_none());
    }
}

#[test]
fn actual_responses_bridge_retains_structured_output_contract() {
    let format = json!({"type":"json_schema","json_schema":{"name":"answer","strict":true,
        "schema":{"type":"object","properties":{"answer":{"type":"string"}},
            "required":["answer"],"additionalProperties":false}}});
    let req = normalize_chat_completions(json!({"model":"alias","messages":[],
        "response_format":format,"verbosity":"high"}))
    .unwrap();
    let packed = pack_responses_bridge(&req, "upstream", false);
    assert_eq!(packed["text"]["format"]["strict"], true);
    assert_eq!(
        packed["text"]["format"]["schema"],
        req.extra["response_format"]["json_schema"]["schema"]
    );
    assert_eq!(packed["text"]["verbosity"], "high");
    assert!(packed.get("response_format").is_none());
}
