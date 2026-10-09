//! Manual prompts use the selected credential and mapped model, never pool fallback.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::http::router::build_router;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

type Calls = Arc<Mutex<Vec<(String, String, String)>>>;
async fn completion(
    State(calls): State<Calls>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    calls.lock().unwrap().push((
        headers["authorization"].to_str().unwrap().into(),
        body["model"].as_str().unwrap().into(),
        body["messages"][0]["content"].as_str().unwrap().into(),
    ));
    Json(json!({"choices":[{"message":{"content":"model reply"},"finish_reason":"stop"}]}))
}

#[tokio::test]
async fn manual_pool_probe_forwards_custom_prompt_and_enforces_scope_and_grant() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_calls = calls.clone();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/v1/chat/completions", post(completion))
                .with_state(server_calls),
        )
        .await
        .unwrap();
    });
    let fixture = ConsoleStateFixture::new(
        serde_json::from_value(json!({
        "providers":[{"id":"pool","preset":"openai","base_url":format!("http://{address}"),
            "supported_models":["b","c"],"model_map_targets":{"a":["b","c"]},
            "credentials":[{"id":"one","api_key":"one-secret","supported_models":["b"]},
                {"id":"two","api_key":"two-secret","supported_models":["c"]}]}],
        "model_routes":[],"aliases":{}}))
        .unwrap(),
        false,
    );
    let grant = grant_console_secret_access(&fixture.state).await;
    let send = |body: Value, grant: String| {
        build_router(fixture.state.clone()).oneshot(
            Request::post("/v1/internal/gateway/console/providers/pool/probe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("x-secret-grant", grant)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
    };
    let request = json!({"prompt":"请返回手动测试结果","model":"a","credentialIds":["one"]});
    assert_eq!(
        send(request.clone(), "invalid-grant".into())
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert!(calls.lock().unwrap().is_empty());
    for body in [
        json!({"prompt":""}),
        json!({"prompt":"x","credentialIds":["foreign-account"]}),
        json!({"prompt":"x".repeat(8193)}),
    ] {
        assert_eq!(
            send(body, grant.clone()).await.unwrap().status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    let response = send(request, grant.clone()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = parse_json(response).await;
    assert_eq!(result["result"]["totalCount"], 1);
    assert_eq!(result["result"]["passedCount"], 1);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[(
            "Bearer one-secret".into(),
            "b".into(),
            "请返回手动测试结果".into()
        )]
    );
    let unsupported = send(
        json!({"prompt":"x","model":"not-supported","credentialIds":["one"]}),
        grant,
    )
    .await
    .unwrap();
    assert_eq!(
        parse_json(unsupported).await["result"]["unsupportedCount"],
        1
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
