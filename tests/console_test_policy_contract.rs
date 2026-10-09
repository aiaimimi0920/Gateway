//! Loopback-only vertical proof: inheritance, exact attribution, quality and durable scheduling.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{
    http::router::build_router, local_runtime::LocalRuntime,
    provider_credential_probe_scheduler::sweep_scheduled_provider_credentials_once,
};
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
    let prompt = body["messages"][0]["content"].as_str().unwrap();
    calls.lock().unwrap().push((
        headers["authorization"].to_str().unwrap().into(),
        body["model"].as_str().unwrap().into(),
        prompt.into(),
    ));
    let answer = if prompt == "account case" { "43" } else { "OK" };
    Json(
        json!({"choices":[{"message":{"content":answer},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1}}),
    )
}
fn plan(prompt: &str, answer: Option<&str>, model: &str, enabled: bool) -> Value {
    json!({"automaticEnabled":enabled,"intervalMinutes":15,"modelSelection":"selected","models":[model],
        "cases":[{"id":"one","name":"one","prompt":prompt,"expectedAnswer":answer,"difficulty":3,"enabled":true}]})
}
async fn send(
    fixture: &ConsoleStateFixture,
    grant: &str,
    request: Value,
) -> axum::response::Response {
    send_console_json(
        &fixture.state,
        Method::POST,
        "/v1/internal/gateway/console/providers/pool/probe",
        request,
        Some(grant),
        None,
    )
    .await
}
async fn read(fixture: &ConsoleStateFixture, grant: &str) -> axum::response::Response {
    build_router(fixture.state.clone())
        .oneshot(
            Request::get("/v1/internal/gateway/console/providers/pool/probe")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("x-secret-grant", grant)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn manual_and_automatic_share_overrides_quality_and_sqlite_results_without_redis() {
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
    let mut fixture = ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{
        "id":"pool","preset":"openai","base_url":format!("http://{address}"),"supported_models":["a","b"],
        "test_policy":plan("pool case",Some("OK"),"a",true),
        "subpool_test_policies":{"free":plan("group case",Some("OK"),"a",true)},
        "credentials":[{"id":"one","api_key":"one-secret","credential_identity_category_id":"free"},
            {"id":"two","api_key":"two-secret","credential_identity_category_id":"free","test_policy":plan("account case",Some("42"),"b",true)},
            {"id":"off","api_key":"off-secret","test_policy":plan("off",Some("OK"),"a",false)}]}]})).unwrap(), false);
    let root = std::env::temp_dir().join(format!(
        "gateway-local-test-policy-{}",
        uuid::Uuid::new_v4()
    ));
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    let grant = grant_console_secret_access(&fixture.state).await;
    let temporary = plan("temporary pool case", Some("OK"), "b", false);
    let request =
        json!({"testPlan":temporary,"scope":{"kind":"pool"},"credentialIds":["one","two"]});
    assert_eq!(
        send(&fixture, "invalid", request.clone()).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        read(&fixture, "invalid").await.status(),
        StatusCode::FORBIDDEN
    );
    for invalid in [
        json!({"testPlan":temporary,"scope":{"kind":"account","id":"foreign"}}),
        json!({"testPlan":temporary,"credentialIds":["foreign"]}),
        json!({"testPlan":{ "modelSelection":"selected","models":["*"],"cases":temporary["cases"]}}),
    ] {
        assert_eq!(
            send(&fixture, &grant, invalid).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    let response = send(&fixture, &grant, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let result = parse_json(response).await;
    assert_eq!(result["result"]["passedCount"], 2);
    let one = &result["result"]["results"][0]["assessment"];
    let two = &result["result"]["results"][1]["assessment"];
    assert_eq!(one["policySource"], "subpool:free");
    assert_eq!(one["models"][0]["score"], 100);
    assert_eq!(two["policySource"], "account");
    assert_eq!(two["models"][0]["score"], 0);
    assert_eq!(two["models"][0]["callable"], true);
    assert_eq!(two["models"][0]["capabilityLevel"], "below-standard");
    assert!(!result.to_string().contains("one-secret"));
    assert!(!result.to_string().contains("two-secret"));
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[
            ("Bearer one-secret".into(), "a".into(), "group case".into()),
            (
                "Bearer two-secret".into(),
                "b".into(),
                "account case".into()
            )
        ]
    );
    let health = fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(Default::default())
        .await
        .unwrap();
    assert_eq!(health.len(), 2);
    assert!(health
        .iter()
        .all(|row| row.status == "active" && row.failure_count == 0));

    let sweep = sweep_scheduled_provider_credentials_once(&fixture.state)
        .await
        .unwrap();
    assert_eq!(sweep.scheduled_count, 2);
    assert_eq!(sweep.due_count, 2);
    assert_eq!(sweep.passed_count, 2);
    assert_eq!(calls.lock().unwrap().len(), 4);
    assert_eq!(
        sweep_scheduled_provider_credentials_once(&fixture.state)
            .await
            .unwrap()
            .due_count,
        0
    );
    assert_eq!(calls.lock().unwrap().len(), 4);
    let stored = parse_json(read(&fixture, &grant).await).await;
    assert_eq!(
        stored["result"]["results"][0]["assessment"]["mode"],
        "automatic"
    );
    let origin = "http://127.0.0.1:1437";
    let browser_grant = build_router(fixture.state.clone())
        .oneshot(
            Request::post("/v1/internal/gateway/console/session/confirm-secret-access")
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("origin", origin)
                .header("content-type", "application/json")
                .body(Body::from(json!({"token": MANAGEMENT_TOKEN}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(browser_grant.status(), StatusCode::OK);
    let browser_grant = parse_json(browser_grant).await["grant"]
        .as_str()
        .unwrap()
        .to_string();
    let calls_before_read = calls.lock().unwrap().len();
    for (request_origin, expected) in [
        (origin, StatusCode::OK),
        ("http://localhost:1437", StatusCode::FORBIDDEN),
    ] {
        let response = build_router(fixture.state.clone())
            .oneshot(
                Request::post("/v1/internal/gateway/console/providers/pool/probe/results")
                    .header("x-management-token", MANAGEMENT_TOKEN)
                    .header("x-secret-grant", &browser_grant)
                    .header("origin", request_origin)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"scope":{"kind":"account","id":"one"}}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        if expected == StatusCode::OK {
            assert_eq!(parse_json(response).await["result"]["totalCount"], 1);
        }
    }
    assert_eq!(calls.lock().unwrap().len(), calls_before_read);
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    assert_eq!(
        sweep_scheduled_provider_credentials_once(&fixture.state)
            .await
            .unwrap()
            .due_count,
        0
    );
    assert_eq!(
        parse_json(read(&fixture, &grant).await).await["result"]["totalCount"],
        2
    );
    assert_eq!(calls.lock().unwrap().len(), 4);

    let ungraded = plan("temporary account case", None, "b", false);
    let scoped = send(
        &fixture,
        &grant,
        json!({"testPlan":ungraded,"scope":{"kind":"account","id":"one"},"credentialIds":["one"]}),
    )
    .await;
    let result = parse_json(scoped).await;
    assert_eq!(result["result"]["totalCount"], 1);
    assert_eq!(
        result["result"]["results"][0]["assessment"]["models"][0]["capabilityLevel"],
        "unrated"
    );
    assert_eq!(calls.lock().unwrap().last().unwrap().0, "Bearer one-secret");
    assert_eq!(
        calls.lock().unwrap().last().unwrap().2,
        "temporary account case"
    );
    let document = parse_json(
        send_console_json(
            &fixture.state,
            Method::GET,
            "/v1/internal/gateway/console/route-config",
            json!({}),
            Some(&grant),
            None,
        )
        .await,
    )
    .await;
    assert!(!document.to_string().contains("temporary account case"));
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    drop(fixture);
    for attempt in 0..40 {
        match std::fs::remove_dir_all(&root) {
            Ok(()) => break,
            Err(error) if attempt < 39 && matches!(error.raw_os_error(), Some(32 | 33)) => {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await
            }
            Err(error) => panic!("Cannot clean owned SQLite test directory: {error}"),
        }
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
