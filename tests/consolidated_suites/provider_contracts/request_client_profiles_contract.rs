//! Selected client defaults must reach the packed request without replacing user choices.
use neuro_gateway::protocol::request_profile::RequestClientProfile;
use neuro_gateway::protocol::responses::normalize_responses;
use neuro_gateway::routing::candidate::ProviderAccountPayload;
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::upstream::client::UpstreamClient;
use neuro_gateway::upstream::headers::build_inference_headers;
use serde_json::json;
use std::collections::HashMap;

fn payload(profile: &str, adapter: &str) -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":adapter, "base_url":"https://relay.example",
        "api_key":"selected-fixture", "request_client_profile":profile}),
    )
    .unwrap()
}

#[test]
fn request_client_profiles_codex_responses_defaults() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "base_url":"https://relay.example",
        "api_key":"fixture", "request_client_profile":"codex_cli"
    }))
    .unwrap();
    let req = normalize_responses(json!({"model":"gpt-fixture", "input":"hello"})).unwrap();
    let plan = UpstreamClient::build_request_plan(&payload, &req, "gpt-fixture", false).unwrap();
    let body = plan.body.unwrap();
    assert_eq!(body["store"], false);
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(body["parallel_tool_calls"], true);
    assert_eq!(body["stream"], false);
}

#[test]
fn request_client_profiles_selection_and_auth_are_independent() {
    for model in ["gpt-fixture", "claude-sonnet-4-6", "gemini-fixture"] {
        let req = normalize_responses(json!({"model":model,"input":"hello"})).unwrap();
        for profile in ["auto", "codex_cli", "claude_cli", "cpa"] {
            for adapter in ["openai_compatible", "anthropic_compatible"] {
                let payload = payload(profile, adapter);
                let extra = HashMap::from([
                    ("User-Agent".into(), "inbound-private-device".into()),
                    ("originator".into(), "other-client".into()),
                    ("x-stainless-os".into(), "inbound-private-os".into()),
                    ("authorization".into(), "Bearer inbound-secret".into()),
                    ("x-request-id".into(), "trace-fixture".into()),
                    ("x-codex-turn-state".into(), "opaque-continuation".into()),
                ]);
                let headers = build_inference_headers(&payload, &req, model, Some(&extra));
                let claude = profile == "claude_cli"
                    || (profile != "codex_cli" && model.starts_with("claude-"));
                assert!(headers["user-agent"]
                    .to_str()
                    .unwrap()
                    .starts_with(if claude {
                        "claude-cli/"
                    } else if profile == "cpa" {
                        "codex-tui/"
                    } else {
                        "codex_exec/"
                    }));
                if profile == "cpa" {
                    assert!(!headers.contains_key("x-request-id"));
                } else {
                    assert_eq!(headers["x-request-id"], "trace-fixture");
                }
                if profile == "cpa" && claude {
                    assert!(!headers.contains_key("x-codex-turn-state"));
                } else {
                    assert_eq!(headers["x-codex-turn-state"], "opaque-continuation");
                }
                if adapter == "openai_compatible" {
                    assert_eq!(headers["authorization"], "Bearer selected-fixture");
                    assert!(!headers.contains_key("x-api-key"));
                } else {
                    assert_eq!(headers["x-api-key"], "selected-fixture");
                    assert!(!headers.contains_key("authorization"));
                }
                assert_eq!(headers.contains_key("originator"), !claude);
                assert_eq!(headers.contains_key("x-stainless-os"), claude);
                if claude && adapter == "anthropic_compatible" {
                    let beta = headers["anthropic-beta"].to_str().unwrap();
                    assert!(beta.contains("claude-code-20250219"));
                    assert!(!beta.contains("oauth-"));
                }
            }
        }
    }
}

#[test]
fn request_client_profiles_preserve_explicit_body_and_protocol() {
    let mut payload = payload("codex_cli", "openai_compatible");
    payload.extra_body = Some(HashMap::from([(
        "parallel_tool_calls".into(),
        json!(false),
    )]));
    let original = json!({"model":"gpt-fixture", "input":"hello", "instructions":"business prompt", "store":true,
        "include":[],"parallel_tool_calls":false, "previous_response_id":"prior", "temperature":0.3,
        "metadata":{"business":"unchanged"},"reasoning":{"effort":"low"}});
    let req = normalize_responses(original.clone()).unwrap();
    for stream in [false, true] {
        let plan =
            UpstreamClient::build_request_plan(&payload, &req, "gpt-fixture", stream).unwrap();
        let mut expected = original.clone();
        expected["stream"] = json!(stream);
        assert_eq!(plan.body.unwrap(), expected);
        assert_eq!(plan.url, "https://relay.example/v1/responses");
    }
    let chat = neuro_gateway::protocol::openai::normalize_chat_completions(
        json!({"model":"claude-sonnet", "messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    let plan = UpstreamClient::build_request_plan(&payload, &chat, "claude-sonnet", false).unwrap();
    assert!(plan.url.ends_with("/chat/completions"));
    assert!(plan.body.as_ref().unwrap().get("include").is_none());
    assert!(plan.body.as_ref().unwrap().get("store").is_none());
}

#[test]
fn request_client_profiles_inherit_override_roundtrip_and_reject_invalid_values() {
    let doc = json!({"providers":[{"id":"relay", "preset":"openai", "base_url":"https://relay.example",
        "request_client_profile":"cpa", "credentials":[
            {"id":"inherit", "api_key":"one", "supported_models":["inherit"]},
            {"id":"explicit", "api_key":"two", "supported_models":["explicit"],"request_client_profile":"claude_cli"},
            {"id":"automatic", "api_key":"three", "supported_models":["automatic"],"request_client_profile":"auto"}]}]});
    let typed: neuro_gateway::routing::config::RouteConfigYaml =
        serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&typed).unwrap()["providers"][0]["request_client_profile"],
        "cpa"
    );
    let store = RouteConfigStore::from_document(typed).unwrap();
    for (model, expected) in [
        ("inherit", RequestClientProfile::Cpa),
        ("explicit", RequestClientProfile::ClaudeCli),
        ("automatic", RequestClientProfile::Auto),
    ] {
        assert_eq!(
            store.resolve_candidates(Some(model))[0]
                .payload
                .request_client_profile,
            Some(expected)
        );
    }
    let mut invalid = doc;
    invalid["providers"][0]["request_client_profile"] = json!("typo");
    assert!(
        serde_json::from_value::<neuro_gateway::routing::config::RouteConfigYaml>(invalid).is_err()
    );
    let old: ProviderAccountPayload = serde_json::from_value(
        json!({"adapter":"openai_compatible","base_url":"https://relay.example","api_key":"old"}),
    )
    .unwrap();
    assert_eq!(old.request_client_profile, None);
    assert!(serde_json::to_value(old)
        .unwrap()
        .get("request_client_profile")
        .is_none());
}

#[test]
fn request_client_profiles_skip_media_browser_and_basispoints() {
    let mut req = normalize_responses(json!({"model":"gpt-fixture","input":"hello"})).unwrap();
    for adapter in [
        "basispoints_compatible",
        "kiro_compatible",
        "gemini_web_compatible",
    ] {
        let mut payload = payload("claude_cli", adapter);
        payload
            .headers
            .insert("user-agent".into(), "product-owned".into());
        let headers = build_inference_headers(&payload, &req, "gpt-fixture", None);
        assert_eq!(headers["user-agent"], "product-owned");
    }
    req.endpoint_kind = neuro_gateway::protocol::canonical::EndpointKind::Embeddings;
    let headers = build_inference_headers(
        &payload("codex_cli", "openai_compatible"),
        &req,
        "gpt-fixture",
        None,
    );
    assert!(!headers.contains_key("originator"));
}

#[test]
fn request_client_profiles_preserve_custom_authentication_header_collisions() {
    let req = normalize_responses(json!({"model":"claude-sonnet", "input":"hello"})).unwrap();
    for adapter in [
        "custom_http",
        "gemini_api_compatible",
        "bedrock_converse_compatible",
    ] {
        for name in ["X-App", "Version", "User-Agent", "X-Stainless-OS"] {
            for profile in ["auto", "codex_cli", "claude_cli", "cpa"] {
                let mut payload = payload(profile, adapter);
                payload.auth_header_name = Some(name.into());
                payload.auth_token = Some("selected-auth-token".into());
                let headers = build_inference_headers(&payload, &req, "claude-sonnet", None);
                assert_eq!(
                    headers.get(name).unwrap(),
                    "selected-auth-token",
                    "{adapter} {profile} {name}"
                );
            }
        }
    }
}

#[test]
fn request_client_profiles_cover_the_official_codex_execution_bypass() {
    let req = normalize_responses(json!({"model":"gpt-fixture", "input":"hello"})).unwrap();
    let mut payload = payload("cpa", "openai_compatible");
    payload.base_url = "https://chatgpt.com/backend-api/codex".into();
    payload.responses_path = Some("/responses".into());
    payload
        .headers
        .insert("Originator".into(), "codex_cli_rs".into());
    for stream in [false, true] {
        let context = neuro_gateway::upstream::chatgpt::official_api::prepare_execute_context(
            &payload,
            &req,
            "gpt-fixture",
            None,
            stream,
        )
        .unwrap();
        assert_eq!(context.headers["originator"], "codex-tui");
        assert_eq!(
            context.headers["accept"],
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            }
        );
        assert_eq!(context.headers["authorization"], "Bearer selected-fixture");
        assert_eq!(context.plan.body.as_ref().unwrap()["store"], false);
        assert_eq!(context.plan.body.as_ref().unwrap()["stream"], stream);
    }
}

#[test]
fn request_client_profiles_claude_beta_negotiation_is_conditional_and_additive() {
    let mut payload = payload("claude_cli", "anthropic_compatible");
    payload.beta_headers = Some(vec!["operator-beta".into(), "claude-code-20250219".into()]);
    for tools in [
        json!([]),
        json!([{"name":"lookup", "input_schema":{"type":"object"}}]),
    ] {
        let req = neuro_gateway::protocol::anthropic::normalize_messages(json!({
            "model":"claude-sonnet", "messages":[{"role":"user","content":"hello"}],
            "max_tokens":8, "tools":tools,
        }))
        .unwrap();
        let headers = build_inference_headers(&payload, &req, "claude-sonnet", None);
        let beta = headers["anthropic-beta"]
            .to_str()
            .unwrap()
            .split(',')
            .collect::<Vec<_>>();
        assert!(beta.contains(&"operator-beta"));
        assert_eq!(
            beta.iter()
                .filter(|value| **value == "claude-code-20250219")
                .count(),
            1
        );
        assert_eq!(
            beta.contains(&"fine-grained-tool-streaming-2025-05-14"),
            !req.tools.is_empty()
        );
        for forbidden in ["oauth-", "redact-", "fast-mode-", "context-1m-"] {
            assert!(!beta.iter().any(|value| value.starts_with(forbidden)));
        }
    }
}
