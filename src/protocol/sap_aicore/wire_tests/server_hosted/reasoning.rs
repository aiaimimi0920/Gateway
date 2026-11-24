//! Public Hosted admission and usage against a non-forwarding SAP fixture.
use super::*;

const VERSION: &str = "2024-12-01-preview";
const CHAT: &str = "chat/completions";

#[test]
#[ignore = "Requires isolated Hosted PostgreSQL schema and Redis; retains scoped fixture data"]
fn sap_aicore_server_hosted_reasoning_public_json_and_sse() {
    run_hosted(hosted_reasoning);
}

async fn hosted_reasoning() {
    let scope = format!("sap-reasoning-{}", uuid::Uuid::new_v4().simple());
    let inference = peer::Peer::chat_with_unique_ids(scope.clone()).await;
    let writer = build(&scope, "writer", &inference).await;
    let ordinary_alias = format!("{scope}-ordinary");
    let (enabled_account, enabled_key) = provision(
        &writer,
        &scope,
        json!({"sap_api_version":VERSION,"sap_o3_mini":true}),
        json!({
            "extraBody":{"sap_o3_mini":false},"api_key":KEY
        }),
    )
    .await;
    provision(
        &writer,
        &ordinary_alias,
        json!({"sap_api_version":VERSION,"sap_o3_mini":false}),
        json!({
            "extra_body":{"sap_o3_mini":true},"api_key":KEY
        }),
    )
    .await;
    let reader = build(&scope, "reader", &inference).await;
    test_support::origin(inference.origin.clone(), async {
        let (status, _) = call(
            &reader,
            "/v1/chat/completions",
            PUBLIC,
            request(&ordinary_alias, false),
        )
        .await;
        assert!(
            !status.is_success(),
            "credential overlay must not activate o3-mini"
        );
        assert!(inference.calls.lock().unwrap().is_empty());
        assert!(inference.oauth.lock().unwrap().is_empty());

        for stream in [false, true] {
            let (status, text) = call(
                &reader,
                "/v1/chat/completions",
                PUBLIC,
                request(&scope, stream),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "stream={stream}: {text}");
            assert!(text.contains("合成答案"), "{text}");
            if stream {
                assert!(text.contains("[DONE]"), "{text}");
            }
        }
    })
    .await;
    assert_eq!(inference.oauth.lock().unwrap().len(), 1);
    assert_wire(&inference);
    assert_audits(&writer, &enabled_account, &enabled_key, &scope, 2).await;
    finish(writer, reader, inference).await;
}

fn request(alias: &str, stream: bool) -> Value {
    let mut value = body(CHAT, stream);
    value["model"] = json!(alias);
    value["reasoning_effort"] = json!(if stream { "high" } else { "low" });
    if stream {
        value["max_completion_tokens"] = json!(128);
    } else {
        value["max_tokens"] = json!(64);
    }
    value
}

fn assert_wire(peer: &peer::Peer) {
    let calls = peer.calls.lock().unwrap();
    assert_eq!(calls.len(), 2, "exactly two sends and no retries");
    for ((path, headers, raw), (effort, cap)) in calls.iter().zip([("low", 64), ("high", 128)]) {
        assert_eq!(
            path,
            &format!(
                "/v2/inference/deployments/{UPSTREAM_MODEL}/chat/completions?api-version={VERSION}"
            )
        );
        assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
        assert_eq!(headers["AI-Resource-Group"], "group-a");
        assert!(!headers.contains_key("cookie"));
        assert!(!headers.contains_key("x-management-token"));
        assert_eq!(raw["reasoning_effort"], effort);
        assert_eq!(raw["max_completion_tokens"], cap);
        for local in ["sap_o3_mini", "sap_api_version", "model", "max_tokens"] {
            assert!(raw.get(local).is_none(), "{local}");
        }
    }
}
