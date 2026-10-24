//! Raw Responses forwarding must obey the function-call string contract without repacking history.
use neuro_gateway::protocol::responses::normalize_responses;
use neuro_gateway::routing::candidate::ProviderAccountPayload;
use neuro_gateway::upstream::openai_compatible_official_api_common::build_request_plan;
use serde_json::{json, Value};

fn payload(profile: &str) -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"openai_compatible", "base_url":"https://fixture.invalid",
        "api_key":"synthetic-secret", "request_client_profile":profile}),
    )
    .unwrap()
}

#[test]
fn responses_input_arguments_at_104_are_strings_on_the_actual_request_plan() {
    for stream in [false, true] {
        for profile in ["auto", "codex_cli"] {
            let mut input = vec![json!({"role":"user", "content":"history"}); 104];
            input.push(
                json!({"type":"function_call", "id":"fc_1", "call_id":"call_1",
                "name":"lookup", "arguments":{"city":"杭州", "nested":{"enabled":true}}}),
            );
            let raw = json!({"model":"gpt-fixture", "stream":stream, "input":input,
                "store":false, "include":["reasoning.encrypted_content"], "parallel_tool_calls":true,
                "instructions":"", "client_metadata":{"opaque":"keep"}, "future_field":{"keep":1}});
            let req = normalize_responses(raw.clone()).unwrap();
            let body = build_request_plan(&payload(profile), &req, "gpt-fixture", stream)
                .unwrap()
                .body
                .unwrap();
            let arguments = body["input"][104]["arguments"]
                .as_str()
                .expect("arguments must be a string");
            assert_eq!(
                serde_json::from_str::<Value>(arguments).unwrap(),
                raw["input"][104]["arguments"]
            );
            let mut expected = raw.clone();
            expected["input"][104]["arguments"] = json!(arguments);
            assert_eq!(body, expected);
            assert_eq!(
                req.raw_body, raw,
                "planning must not mutate retained request evidence"
            );
        }
    }
}

#[test]
fn responses_input_arguments_preserve_strings_and_unrelated_opaque_fields() {
    let input = json!([
        {"type":"function_call", "call_id":"call_1", "name":"a", "arguments":" {\"x\": 1} \n"},
        {"type":"function_call", "call_id":"call_2", "name":"b", "arguments":""},
        {"type":"function_call", "call_id":"call_3", "name":"c", "arguments":"not-json"},
        {"type":"function_call", "call_id":"call_4", "name":"d"},
        {"type":"custom_tool_call", "call_id":"call_5", "name":"custom", "input":{"opaque":true}},
        {"type":"function_call_output", "call_id":"call_1", "output":{"arguments":{"keep":true}}},
        {"type":"reasoning", "id":"rs_1", "encrypted_content":"opaque", "summary":[]}
    ]);
    let req = normalize_responses(json!({"model":"gpt-fixture", "input":input})).unwrap();
    let body = build_request_plan(&payload("codex_cli"), &req, "gpt-fixture", false)
        .unwrap()
        .body
        .unwrap();
    assert_eq!(body["input"], input);
}

#[test]
fn responses_input_arguments_serialize_all_existing_non_string_values_once() {
    for value in [
        json!({}),
        json!([1, "two"]),
        json!(null),
        json!(true),
        json!(42),
    ] {
        let raw = json!({"model":"gpt-fixture", "input":[
            {"type":"function_call", "call_id":"call_1", "name":"lookup", "arguments":value}
        ]});
        let req = normalize_responses(raw).unwrap();
        let body = build_request_plan(&payload("codex_cli"), &req, "gpt-fixture", false)
            .unwrap()
            .body
            .unwrap();
        assert_eq!(body["input"][0]["arguments"], json!(value.to_string()));
        let replay = normalize_responses(body.clone()).unwrap();
        assert_eq!(
            build_request_plan(&payload("codex_cli"), &replay, "gpt-fixture", false)
                .unwrap()
                .body
                .unwrap(),
            body
        );
    }
}
