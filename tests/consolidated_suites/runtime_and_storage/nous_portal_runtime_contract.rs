//! Public request preparation must preserve the Portal Chat wire without borrowing CLI identity.
use neuro_gateway::{
    protocol::{call_flow, canonical::EndpointKind, responses},
    routing::candidate::ProviderAccountPayload,
    upstream::client::UpstreamClient,
};
use serde_json::json;

fn payload() -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter":"openai_compatible", "auth_mode":"nous_portal_oauth",
        "base_url":"https://inference-api.nousresearch.com/v1", "api_key":"synthetic-access"
    }))
    .unwrap()
}

#[test]
#[cfg(feature = "line-nous-portal-oauth")]
fn nous_responses_uses_chat_messages_and_preserves_function_history() {
    let req = responses::normalize_responses(json!({
        "model":"org/model", "input":[
            {"role":"user","content":"check"},
            {"type":"function_call","call_id":"call-1","name":"lookup","arguments":"{\"id\":1}"},
            {"type":"function_call_output","call_id":"call-1","output":"found"}
        ], "tools":[{"type":"function","name":"lookup","parameters":{"type":"object"}}]
    }))
    .unwrap();
    let plan = UpstreamClient::build_request_plan(&payload(), &req, "org/model", false).unwrap();
    assert_eq!(
        plan.url,
        "https://inference-api.nousresearch.com/v1/chat/completions"
    );
    let body = plan.body.unwrap();
    assert!(body.get("input").is_none());
    assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call-1");
    assert_eq!(body["messages"][2]["tool_call_id"], "call-1");
    assert_eq!(body["tools"][0]["function"]["name"], "lookup");
    assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
}

#[test]
#[cfg(feature = "line-nous-portal-oauth")]
fn nous_chat_bridge_rejects_remote_responses_state_and_hosted_tools() {
    for patch in [
        json!({"previous_response_id":"foreign-state"}),
        json!({"conversation":"foreign-conversation"}),
        json!({"conversation":{"id":"foreign-conversation"}}),
        json!({"store":true}),
        json!({"background":true}),
        json!({"tools":[{"type":"web_search"}]}),
    ] {
        let mut body = json!({"model":"org/model", "input":"hello"});
        body.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let req = responses::normalize_responses(body).unwrap();
        assert!(
            UpstreamClient::build_request_plan(&payload(), &req, "org/model", false).is_err(),
            "accepted {patch}"
        );
    }
}

#[test]
fn nous_rejects_foreign_transport_and_identity_before_shortcuts() {
    for patch in [
        json!({"base_url":"https://attacker.example/v1"}),
        json!({"adapter":"anthropic_compatible"}),
        json!({"request_client_profile":"codex_cli"}),
        json!({"headers":{"cookie":"foreign-session"}}),
        json!({"responses_path":"/responses"}),
        json!({"extra_body":{"provider":{"order":["other"]}}}),
    ] {
        let mut value = serde_json::to_value(payload()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let changed = serde_json::from_value(value).unwrap();
        assert!(
            call_flow::prepare(&changed).is_err(),
            "accepted mutation: {patch}"
        );
    }
}
