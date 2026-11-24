//! The declared o3-mini deployment must work through the public relay, with no secret replay.
use super::*;

#[tokio::test]
async fn sap_aicore_reasoning_uses_explicit_deployment_and_exact_chat_wire() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::with_reasoning(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            for stream in [false, true] {
                let mut request = body("chat/completions", stream);
                request["reasoning_effort"] = json!(if stream { "high" } else { "low" });
                if stream { request["max_tokens"] = json!(512); }
                else { request["max_completion_tokens"] = json!(1024); }
                let response = fixture.open("chat/completions", request).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains("合成答案"), "{text}");
                if stream { assert!(text.contains("[DONE]"), "{text}"); }
            }
            assert_eq!(peer.oauth.lock().unwrap().len(), 1);
            {
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), 2);
                for ((path, headers, wire), (effort, cap)) in calls.iter().zip([("low", 1024), ("high", 512)]) {
                    assert_eq!(path, "/v2/inference/deployments/deployment1/chat/completions?api-version=2024-12-01-preview");
                    assert_eq!(headers["AI-Resource-Group"], "group-a");
                    assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
                    assert_eq!(wire["reasoning_effort"], effort);
                    assert_eq!(wire["max_completion_tokens"], cap);
                    for name in ["max_tokens", "model", "sap_o3_mini", "sap_api_version"] {
                        assert!(wire.get(name).is_none(), "{wire}");
                    }
                }
            }
            for audit in fixture.audits(2).await {
                assert_identity(&audit);
                assert_eq!(audit.status, "completed");
                assert_eq!(audit.total_tokens, Some(5));
            }
            assert_eq!(fixture.remaining().await, 0);
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn sap_aicore_reasoning_rejects_unrepresentable_controls_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let disabled = Box::pin(Fixture::new(&peer)).await;
        let enabled = Box::pin(Fixture::with_reasoning(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut effort = body("chat/completions", false);
            effort["reasoning_effort"] = json!("low");
            assert!(!disabled
                .open("chat/completions", effort)
                .await
                .status()
                .is_success());
            for invalid in [
                json!({"reasoning_effort":"xhigh"}),
                json!({"temperature":0.5}),
                json!({"max_tokens":512,"max_completion_tokens":1024}),
                json!({"max_completion_tokens":0}),
            ] {
                let mut req = body("chat/completions", false);
                req.as_object_mut()
                    .unwrap()
                    .extend(invalid.as_object().unwrap().clone());
                let response = enabled.open("chat/completions", req).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert!(!status.is_success(), "{invalid}: {text}");
                assert_eq!(enabled.remaining().await, 2);
            }
            assert!(peer.calls.lock().unwrap().is_empty());
            assert!(peer.oauth.lock().unwrap().is_empty());
            assert_eq!(disabled.remaining().await, 2);
        })
        .await;
        enabled.finish().await;
        disabled.finish().await;
        peer.stop().await;
    })
    .await;
}

#[test]
fn sap_aicore_reasoning_configuration_is_deployment_owned() {
    use crate::routing::config::RouteConfigStore;
    for (version, flag, override_scope, valid) in [
        ("2024-12-01-preview", json!(true), false, true),
        ("2024-12-01-preview", json!(true), true, false),
        ("2024-12-01-preview", json!(false), true, false),
        ("2024-10-21", json!(true), false, false),
        ("2024-12-01-preview", json!("true"), false, false),
    ] {
        let mut doc = super::super::tests::document();
        let provider = &mut doc["providers"][0];
        provider["extra_body"]["sap_api_version"] = json!(version);
        if override_scope {
            provider["credentials"][0]["extra_body"] = json!({"sap_o3_mini":flag});
        } else {
            provider["extra_body"]["sap_o3_mini"] = flag;
        }
        assert_eq!(
            RouteConfigStore::from_document(serde_json::from_value(doc).unwrap()).is_ok(),
            valid
        );
    }
}

#[test]
fn sap_aicore_reasoning_bridge_preserves_length_without_claiming_native_thinking() {
    use crate::protocol::{anthropic, canonical::EndpointKind, responses};
    let mut payload = super::super::tests::payload();
    let settings = payload.extra_body.as_mut().unwrap();
    settings.insert("sap_api_version".into(), json!("2024-12-01-preview"));
    settings.insert("sap_o3_mini".into(), json!(true));
    let requests = [
        anthropic::normalize_messages(body("messages", false)).unwrap(),
        responses::normalize_responses(
            json!({"model":MODEL,"input":"test","max_output_tokens":32}),
        )
        .unwrap(),
    ];
    for request in requests {
        let plan = super::super::runtime::plan(&payload, &request, UPSTREAM_MODEL, false).unwrap();
        assert_eq!(plan.response_kind, EndpointKind::ChatCompletions);
        let wire = plan.body.unwrap();
        assert_eq!(wire["max_completion_tokens"], 32);
        assert!(wire.get("reasoning_effort").is_none());
    }
    let request = responses::normalize_responses(
        json!({"model":MODEL,"input":"test","reasoning":{"effort":"low"}}),
    )
    .unwrap();
    assert!(super::super::runtime::plan(&payload, &request, UPSTREAM_MODEL, false).is_err());
}
