//! Planner and final sender gates cover canonical flattening and plan mutation.
use super::*;
use crate::{protocol::sensenova, upstream::client::UpstreamClient};
use serde_json::json;

fn payload() -> crate::routing::candidate::ProviderAccountPayload {
    let document = json!({"providers":[{"id":"media","preset":sensenova::PRESET,
        "base_url":sensenova::BASE,"explicit_credentials_only":true,
        "credentials":[{"id":"exact","api_key":"synthetic-content-key",
        "supported_models":["deepseek-flash"]}]}]});
    crate::routing::config::RouteConfigStore::from_document(
        serde_json::from_value(document).unwrap(),
    )
    .unwrap()
    .snapshot()
    .select_credential_probe_target("exact")
    .unwrap()
    .payload
}

fn request(raw: Value) -> CanonicalRelayRequest {
    crate::protocol::openai::normalize_chat_completions(raw).unwrap()
}

#[test]
fn sensenova_chat_content_rejects_unknown_blocks_before_generic_flattening() {
    let account = payload();
    for role in ["user", "assistant", "system", "tool"] {
        for part in [
            json!({"type":"video_url","video_url":{"url":"https://example.invalid/movie"}}),
            json!({"type":"input_video","video_url":"https://example.invalid/movie"}),
            json!({"type":"input_audio","input_audio":{"data":"AAAB","format":"wav"}}),
            json!({"type":"file","file":{"file_id":"file-unknown"}}),
            json!({"type":"document","source":{"type":"url","url":"https://example.invalid/doc"}}),
            json!({"type":"future_media","secret":"not-in-errors"}),
            json!({"type":"text","text":17}),
            json!(123),
        ] {
            let req = request(json!({"messages":[{"role":role,"tool_call_id":"call-1",
                "content":[{"type":"text","text":"inspect"}, part]}]}));
            let error = UpstreamClient::build_request_plan(&account, &req, "deepseek-flash", false)
                .unwrap_err();
            assert_eq!(
                error.code.as_deref(),
                Some("sensenova_chat_content_unsupported"),
                "{role}"
            );
            assert!(!error.message.contains("not-in-errors"));
        }
    }
}

#[test]
fn sensenova_chat_content_keeps_raw_text_tools_and_reasoning_history() {
    let account = payload();
    let req = request(json!({"reasoning_effort":"high","messages":[
        {"role":"user","content":[{"type":"text","text":"hi","cache_control":{"type":"ephemeral"}},
            {"type":"text","text":"there"}]},
        {"role":"assistant","content":null,"reasoning_content":"prior reasoning",
            "tool_calls":[{"id":"call-1","type":"function","function":{"name":"lookup","arguments":"{}"}}]},
        {"role":"tool","tool_call_id":"call-1","content":"result"}],
        "tools":[{"type":"function","function":{"name":"lookup","parameters":{"type":"object"}}}]}));
    assert!(matches!(
        req.messages[0].content[0],
        ContentPart::Raw { .. }
    ));
    let plan = UpstreamClient::build_request_plan(&account, &req, "deepseek-flash", false).unwrap();
    let body = plan.body.as_ref().unwrap();
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert_eq!(body["messages"][1]["reasoning_content"], "prior reasoning");
    assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call-1");
    assert_eq!(body["reasoning_effort"], "high");
    sensenova::finalize_headers(&account, &plan, &mut rquest::header::HeaderMap::new()).unwrap();
}

#[test]
fn sensenova_chat_content_final_plan_and_sender_reject_injected_blocks() {
    let account = payload();
    let req = request(json!({"messages":[{"role":"user","content":"hi"}]}));
    for content in [
        json!([{"type":"input_audio"}]),
        json!([{"type":"text","text":false}]),
        json!([{"text":"missing type"}]),
        json!([42]),
        json!({"type":"video_url","video_url":{"url":"https://example.invalid/clip"}}),
        json!(42),
        json!(false),
    ] {
        let mut plan =
            UpstreamClient::build_request_plan(&account, &req, "deepseek-flash", false).unwrap();
        plan.body.as_mut().unwrap()["messages"][0]["content"] = content;
        let mut headers = rquest::header::HeaderMap::new();
        headers.insert("authorization", "Bearer synthetic".parse().unwrap());
        for error in [
            sensenova::validate_plan(&account, &plan).unwrap_err(),
            sensenova::finalize_headers(&account, &plan, &mut headers).unwrap_err(),
        ] {
            assert_eq!(
                error.code.as_deref(),
                Some("sensenova_chat_content_unsupported")
            );
        }
    }
    for messages in [json!({}), json!(null), json!([42])] {
        let mut plan =
            UpstreamClient::build_request_plan(&account, &req, "deepseek-flash", false).unwrap();
        plan.body.as_mut().unwrap()["messages"] = messages;
        assert_eq!(
            sensenova::validate_plan(&account, &plan)
                .unwrap_err()
                .code
                .as_deref(),
            Some("sensenova_chat_content_unsupported")
        );
    }
}
