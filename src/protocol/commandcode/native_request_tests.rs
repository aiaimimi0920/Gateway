//! Native request fidelity and canonical edits are tested independently of provider transport.
use super::*;
use crate::protocol::{
    anthropic, canonical::CanonicalRelayRequest, responses as response_protocol,
};
use serde_json::{json, Value};

pub(super) fn raw(endpoint: &str) -> Value {
    if endpoint == "responses" {
        json!({"model":"alias","stream":false,"store":false,"instructions":"policy",
            "reasoning":{"effort":"high"},"max_output_tokens":2048,"metadata":{"trace":"synthetic"},
            "input":[{"type":"reasoning","id":"rs-1","encrypted_content":"opaque","summary":[]},
                {"type":"custom_tool_call","call_id":"c-1","name":"grammar","input":"native command"},
                {"type":"custom_tool_call_output","call_id":"c-1","output":"result"},
                {"role":"user","content":[{"type":"input_text","text":"question"},{"type":"input_image","image_url":"https://example.com/input.png","detail":"high"}]}],
            "tools":[{"type":"custom","name":"grammar","format":{"type":"text"}},{"type":"web_search"}],
            "tool_choice":"auto","provider_extension":{"opaque":"keep"}})
    } else {
        json!({"model":"alias","stream":false,"max_tokens":2048,"thinking":{"type":"enabled","budget_tokens":1024},
            "system":[{"type":"text","text":"policy","cache_control":{"type":"ephemeral"}}],
            "messages":[{"role":"assistant","content":[{"type":"thinking","thinking":"reason","signature":"signed"},
                {"type":"tool_use","id":"c-1","name":"lookup","input":{"x":1}}]},
                {"role":"user","content":[{"type":"tool_result","tool_use_id":"c-1","is_error":true,"content":"failed"},
                    {"type":"image","source":{"type":"url","url":"https://example.com/input.png"}},{"type":"text","text":"question"}]}],
            "tools":[{"name":"lookup","input_schema":{"type":"object"}},{"type":"web_search_20250305","name":"web_search","max_uses":2}],
            "tool_choice":{"type":"auto","disable_parallel_tool_use":true},"context_management":{"edits":[]},
            "metadata":{"user_id":"synthetic"},"provider_extension":{"opaque":"keep"}})
    }
}
fn normalize(endpoint: &str, raw: Value) -> CanonicalRelayRequest {
    if endpoint == "responses" {
        response_protocol::normalize_responses(raw).unwrap()
    } else {
        anthropic::normalize_messages(raw).unwrap()
    }
}
fn payload(endpoint: &str) -> ProviderAccountPayload {
    let doc = json!({"providers":[{"id":"p","preset":PRESET,"base_url":BASE,"explicit_credentials_only":true,
        "credentials":[{"id":"key","api_key":"synthetic-key","supported_models":["upstream"],
        "model_catalog":{"sourceUrl":SOURCE,"checkedAt":"2026-11-13T00:00:00Z","models":["upstream"],
            "supportedEndpoints":{"upstream":[format!("/{endpoint}")]}}}]}]});
    let store = crate::routing::config::RouteConfigStore::from_document(
        serde_json::from_value(doc).unwrap(),
    )
    .unwrap();
    let mut p = store
        .snapshot()
        .select_credential_probe_target("key")
        .unwrap()
        .payload;
    if endpoint == "responses" {
        p.responses_path = Some("/responses".into());
    } else {
        p.messages_path = Some("/messages".into());
    }
    p.chat_completions_path = None;
    p
}

#[test]
fn commandcode_native_request_preserves_rich_input_without_relaxing_chat_bridge() {
    for endpoint in ["responses", "messages"] {
        let mut expected = raw(endpoint);
        let req = normalize(endpoint, expected.clone());
        let p = payload(endpoint);
        let body = runtime::plan(&p, &req, "upstream", true)
            .unwrap()
            .body
            .unwrap();
        expected["model"] = "upstream".into();
        expected["stream"] = true.into();
        assert_eq!(body, expected);
        let mut chat = p.clone();
        chat.responses_path = None;
        chat.messages_path = None;
        assert!(runtime::plan(&chat, &req, "upstream", true).is_err());
    }
}

#[test]
fn commandcode_native_request_canonical_edits_cannot_revive_raw_values() {
    for endpoint in ["responses", "messages"] {
        let raw = if endpoint == "responses" {
            json!({"model":"alias","input":"old","instructions":"old policy","tools":[{"type":"function","name":"old","parameters":{}}],"tool_choice":"auto","reasoning":{"effort":"high"},"previous_response_id":"old-id","max_output_tokens":100,"metadata":{"old":true}})
        } else {
            json!({"model":"alias","messages":[{"role":"user","content":"old"}],"system":"old policy","tools":[{"name":"old","input_schema":{}}],"tool_choice":{"type":"auto"},"thinking":{"type":"enabled","budget_tokens":10},"max_tokens":100,"metadata":{"old":true}})
        };
        let mut req = normalize(endpoint, raw);
        req.messages = normalize(
            endpoint,
            if endpoint == "responses" {
                json!({"input":"new","instructions":"new policy"})
            } else {
                json!({"messages":[{"role":"user","content":"new"}],"system":"new policy"})
            },
        )
        .messages;
        req.tools.clear();
        req.tool_choice = None;
        req.reasoning = None;
        req.previous_response_id = None;
        req.metadata = None;
        req.extra.remove("metadata");
        req.extra.insert(
            if endpoint == "responses" {
                "max_output_tokens"
            } else {
                "max_tokens"
            }
            .into(),
            json!(77),
        );
        req.extra
            .insert("gateway_private".into(), json!("internal"));
        let body = runtime::plan(&payload(endpoint), &req, "upstream", false)
            .unwrap()
            .body
            .unwrap();
        let text = body.to_string();
        assert!(!text.contains("old") && !text.contains("gateway_private"));
        assert!(text.contains("new policy") && text.contains("new"));
        for key in [
            "tools",
            "tool_choice",
            "reasoning",
            "thinking",
            "previous_response_id",
            "metadata",
        ] {
            assert!(body.get(key).is_none(), "{body}");
        }
        assert_eq!(
            body[if endpoint == "responses" {
                "max_output_tokens"
            } else {
                "max_tokens"
            }],
            77
        );
    }
}

#[test]
fn commandcode_native_request_policy_only_edit_preserves_rich_history() {
    for endpoint in ["responses", "messages"] {
        let input = raw(endpoint);
        let mut req = normalize(endpoint, input.clone());
        req.messages[0].content = vec![crate::protocol::canonical::ContentPart::Text {
            text: "new policy".into(),
        }];
        let body = runtime::plan(&payload(endpoint), &req, "upstream", false)
            .unwrap()
            .body
            .unwrap();
        if endpoint == "responses" {
            assert_eq!(
                body["input"].as_array().unwrap()[1..],
                input["input"].as_array().unwrap()[..]
            );
            assert!(body.get("instructions").is_none());
        } else {
            assert_eq!(body["messages"], input["messages"]);
            assert_eq!(body["system"][0]["text"], "new policy");
        }
        req.messages.pop();
        assert!(runtime::plan(&payload(endpoint), &req, "upstream", false).is_err());
    }
}

#[test]
fn commandcode_native_request_controls_and_tool_edits_stay_local() {
    for endpoint in ["responses", "messages"] {
        let mut input = raw(endpoint);
        input["gateway_auto_cache"] = json!(true);
        input["provider_extension"]["gateway_private"] = json!("user data");
        input["tools"][0] = if endpoint == "responses" {
            json!({"type":"function","name":"old","description":"old description","parameters":{"type":"object"},"strict":true})
        } else {
            json!({"name":"old","description":"old description","input_schema":{"type":"object"},"cache_control":{"type":"ephemeral"}})
        };
        let mut req = normalize(endpoint, input);
        req.tools[0].name = Some("renamed".into());
        req.tools[0].description = None;
        req.tools[0].input_schema = None;
        req.extra
            .insert("tools".into(), json!([{"name":"injected"}]));
        req.extra.insert("tool_choice".into(), json!("injected"));
        req.tool_choice = None;
        let body = runtime::plan(&payload(endpoint), &req, "upstream", false)
            .unwrap()
            .body
            .unwrap();
        assert!(body.get("gateway_auto_cache").is_none());
        assert!(body.get("tool_choice").is_none());
        assert_eq!(body["provider_extension"]["gateway_private"], "user data");
        assert_eq!(body["tools"][0]["name"], "renamed");
        assert!(body["tools"][0].get("parameters").is_none());
        assert!(body["tools"][0].get("input_schema").is_none());
        assert!(body["tools"][0].get("description").is_none());
        assert_eq!(
            body["tools"][0][if endpoint == "responses" {
                "strict"
            } else {
                "cache_control"
            }],
            if endpoint == "responses" {
                json!(true)
            } else {
                json!({"type":"ephemeral"})
            }
        );
        assert!(!body.to_string().contains("injected"));
    }
    let mut req = normalize("messages", raw("messages"));
    req.extra.remove("max_tokens");
    assert_eq!(
        runtime::plan(&payload("messages"), &req, "upstream", false)
            .unwrap()
            .body
            .unwrap()["max_tokens"],
        4096
    );
}

#[test]
fn commandcode_native_request_unimplemented_lifecycle_is_not_silent() {
    for (key, value) in [
        ("background", json!(true)),
        ("store", json!(true)),
        ("previous_response_id", json!("r")),
        ("conversation", json!("c")),
        ("gateway_vertex_history", json!({})),
        ("gateway_bedrock_history", json!({})),
    ] {
        let mut input = raw("responses");
        input[key] = value;
        let req = normalize("responses", input);
        assert!(
            runtime::plan(&payload("responses"), &req, "upstream", false).is_err(),
            "{key}"
        );
    }
    let mut input = raw("responses");
    input["tools"] = json!([{"type":"mcp","server_url":"https://example.com"}]);
    assert!(runtime::plan(
        &payload("responses"),
        &normalize("responses", input),
        "upstream",
        false
    )
    .is_err());
}
