//! Model declarations are observations, never aliases or proof of model identity.
use axum::{body::Body, http::Request, response::IntoResponse, routing::post, Json, Router};
use neuro_gateway::{
    http::router::build_router,
    protocol::{anthropic, openai, responses},
    routing::config::RouteConfigYaml,
};
use serde_json::{json, Value};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

#[test]
fn parsers_keep_raw_declaration_separate_from_missing_model_fallback() {
    for declared in [Some("provider-model"), None] {
        let mut chat = json!({"choices":[{"message":{"content":"ok"}}]});
        let mut response = json!({"output":[]});
        let mut messages = json!({"content":[{"type":"text","text":"ok"}]});
        for body in [&mut chat, &mut response, &mut messages] {
            if let Some(model) = declared {
                body["model"] = json!(model);
            }
        }
        for canonical in [
            openai::unpack_openai_response(&chat).unwrap(),
            responses::unpack_responses_response(&response).unwrap(),
            anthropic::unpack_anthropic_response(&messages).unwrap(),
        ] {
            let encoded = serde_json::to_value(canonical).unwrap();
            assert_eq!(encoded["model_declaration"]["value"].as_str(), declared);
            if declared.is_some() {
                assert_eq!(encoded["model_declaration"]["source"], "model");
            }
        }
    }
}

#[test]
fn old_canonical_json_and_synthetic_dashscope_model_stay_unknown() {
    use neuro_gateway::protocol::canonical::CanonicalRelayResponse;
    let old = json!({"model":"fallback","text":"ok","usage":null,
        "tool_calls":[],"upstream_status":200,"finish_reason":"stop"});
    let canonical: CanonicalRelayResponse = serde_json::from_value(old).unwrap();
    assert!(canonical.model_declaration.is_none());
    let raw = json!({"output":{"text":"ok"}});
    let canonical =
        neuro_gateway::protocol::dashscope::response::unpack(&raw, "injected-fallback").unwrap();
    assert_eq!(canonical.model, "injected-fallback");
    assert!(canonical.model_declaration.is_none());
}

#[tokio::test]
async fn raw_buffered_and_sse_declarations_reach_management_without_body_or_secret() {
    let mock = Router::new().route("/v1/chat/completions", post(|Json(body): Json<Value>| async move {
        let present = body["model"] != "missing";
        if body["stream"] == true {
            let model = if present { ",\"model\":\"declared-model\"" } else { "" };
            let wire = format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"private-reply\"}},\"finish_reason\":null}}]{model}}}\n\ndata: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\ndata: [DONE]\n\n");
            return ([("content-type","text/event-stream")],wire).into_response();
        }
        let mut reply = json!({"choices":[{"message":{"content":"private-reply"},"finish_reason":"stop"}]});
        if present { reply["model"] = json!("declared-model"); }
        Json(reply).into_response()
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, mock).await.unwrap();
    });
    let config: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"evidence-fixture","base_url":base,"adapter":"openai_compatible","api_key":"private-provider-key","supported_models":["sent-model","missing"],"model_map":{"alias":"sent-model","unknown-alias":"missing"}}],"model_routes":[]})).unwrap();
    let fixture = support::ConsoleStateFixture::new(config, true);
    let router = build_router(fixture.state.clone());
    for stream in [false, true] {
        for (requested, sent, declared) in [
            ("alias", "sent-model", Some("declared-model")),
            ("unknown-alias", "missing", None),
        ] {
            let response = router.clone().oneshot(Request::post("/v1/chat/completions").header("authorization","Bearer public-api-contract-secret").header("content-type","application/json").body(Body::from(json!({"model":requested,"stream":stream,"messages":[{"role":"user","content":"private-prompt"}]}).to_string())).unwrap()).await.unwrap();
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap();
            assert!(
                status.is_success(),
                "{status}: {}",
                String::from_utf8_lossy(&body)
            );
            if !stream {
                let reply: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(
                    reply["model"], requested,
                    "observations must not change caller aliases"
                );
            }
            let summary = router
                .clone()
                .oneshot(
                    Request::get("/v1/internal/gateway/operations/summary")
                        .header("x-management-token", support::MANAGEMENT_TOKEN)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(summary.status().is_success());
            let value: Value = serde_json::from_slice(
                &axum::body::to_bytes(summary.into_body(), 1024 * 1024)
                    .await
                    .unwrap(),
            )
            .unwrap();
            let observations = value["summary"]["modelObservations"]
                .as_array()
                .expect("bounded model observations must be visible to management");
            let row = observations
                .iter()
                .find(|row| {
                    row["providerId"] == "evidence-fixture"
                        && row["requestedModel"] == requested
                        && row["stream"] == stream
                })
                .expect("completed request observation");
            assert_eq!(row["upstreamModel"], sent);
            assert_eq!(row["declaredModel"].as_str(), declared);
            assert_eq!(
                row["relation"],
                if declared.is_some() {
                    "declared_differs"
                } else {
                    "unknown"
                }
            );
            assert_eq!(row["verified"], false);
            let encoded = serde_json::to_string(observations).unwrap();
            for secret in ["private-prompt", "private-reply", "private-provider-key"] {
                assert!(!encoded.contains(secret));
            }
        }
    }
    let denied = router
        .clone()
        .oneshot(
            Request::get("/v1/internal/gateway/operations/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!denied.status().is_success());
    let public_key = router
        .oneshot(
            Request::get("/v1/internal/gateway/operations/summary")
                .header("authorization", "Bearer public-api-contract-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(!public_key.status().is_success());
    task.abort();
}
