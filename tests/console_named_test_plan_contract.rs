//! Loopback proof of independent plans, exact auth/attribution, durable results and schedule slots.
use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{
    console::document::validate_route_document, local_runtime::LocalRuntime,
    provider_credential_probe_scheduler::sweep_scheduled_provider_credentials_once,
};
use serde_json::{json, Value};
use std::{
    future::IntoFuture,
    sync::{Arc, Mutex},
};
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

type Calls = Arc<Mutex<Vec<(String, String)>>>;
fn policy(prompt: &str, automatic: bool) -> Value {
    json!({"automaticEnabled":automatic,"intervalMinutes":15,"modelSelection":"selected","models":["a"],
        "cases":[{"id":"ok","name":"OK","prompt":prompt,"expectedAnswer":"OK","difficulty":2,"enabled":true}]})
}
fn plan(id: &str, prompt: &str) -> Value {
    json!({"id":id,"name":id,"scopes":[{"kind":"account","id":"one"}],"policy":policy(prompt,true)})
}
async fn completion(
    State(calls): State<Calls>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let prompt = body["messages"][0]["content"].as_str().unwrap();
    calls.lock().unwrap().push((
        headers["authorization"].to_str().unwrap().into(),
        prompt.into(),
    ));
    Json(
        json!({"choices":[{"message":{"content":if prompt == "second" {"wrong"} else {"OK"}},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":1,"completion_tokens":1}}),
    )
}
async fn request(
    fixture: &ConsoleStateFixture,
    grant: &str,
    read: bool,
    body: Value,
) -> axum::response::Response {
    send_console_json(
        &fixture.state,
        Method::POST,
        if read {
            "/v1/internal/gateway/console/providers/pool/probe/results"
        } else {
            "/v1/internal/gateway/console/providers/pool/probe"
        },
        body,
        Some(grant),
        None,
    )
    .await
}

#[tokio::test]
async fn two_plans_in_one_scope_keep_separate_results_and_slots_across_restart_and_deletion() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route("/v1/chat/completions", post(completion))
                .with_state(calls.clone()),
        )
        .into_future(),
    );
    let mut document = json!({"providers":[{"id":"pool","preset":"openai","base_url":format!("http://{address}"),
        "supported_models":["a"],"test_plans":[plan("first","first"),plan("second","second")],
        "test_policy":policy("legacy",false),"credentials":[{"id":"one","api_key":"one-secret","test_policy":policy("override",false)},
            {"id":"two","api_key":"two-secret"}]}]});
    assert!(validate_route_document(serde_json::from_value(document.clone()).unwrap()).is_ok());
    let mut fixture =
        ConsoleStateFixture::new(serde_json::from_value(document.clone()).unwrap(), false);
    let root = std::env::temp_dir().join(format!("gateway-named-plans-{}", uuid::Uuid::new_v4()));
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    let grant = grant_console_secret_access(&fixture.state).await;
    assert_eq!(
        request(&fixture, "invalid", false, json!({"planId":"first"}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    for body in [
        json!({"planId":"foreign"}),
        json!({"planId":"first","credentialIds":["two"]}),
        json!({"planId":"first","scope":{"kind":"pool"}}),
        json!({"planId":"first","testPlan":policy("bad",false)}),
    ] {
        assert_eq!(
            request(&fixture, &grant, false, body).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    let summary = sweep_scheduled_provider_credentials_once(&fixture.state)
        .await
        .unwrap();
    assert_eq!(summary.due_count, 2);
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[
            ("Bearer one-secret".into(), "first".into()),
            ("Bearer one-secret".into(), "second".into())
        ]
    );
    assert_eq!(
        sweep_scheduled_provider_credentials_once(&fixture.state)
            .await
            .unwrap()
            .due_count,
        0
    );
    let all = parse_json(request(&fixture, &grant, true, json!({"includePlans":true})).await).await;
    assert_eq!(all["result"]["totalCount"], 2);
    assert_eq!(all["result"]["results"][0]["assessment"]["planId"], "first");
    assert_eq!(
        all["result"]["results"][1]["assessment"]["models"][0]["score"],
        0
    );
    assert_eq!(
        all["result"]["results"][1]["assessment"]["models"][0]["callable"],
        true
    );
    assert!(!all.to_string().contains("one-secret"));
    assert_eq!(
        parse_json(request(&fixture, &grant, true, json!({})).await).await["result"]["totalCount"],
        0
    );
    let second =
        parse_json(request(&fixture, &grant, true, json!({"planId":"second"})).await).await;
    assert_eq!(second["result"]["totalCount"], 1);
    assert_eq!(
        second["result"]["results"][0]["assessment"]["policySource"],
        "plan:second"
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
    let manual = parse_json(
        request(
            &fixture,
            &grant,
            false,
            json!({"planId":"first","credentialIds":["one"]}),
        )
        .await,
    )
    .await;
    assert_eq!(
        manual["result"]["results"][0]["assessment"]["planId"],
        "first"
    );
    assert_eq!(calls.lock().unwrap().len(), 3);
    let old = parse_json(request(&fixture,&grant,false,json!({"testPlan":policy("temporary",false),"scope":{"kind":"pool"},"credentialIds":["one"]})).await).await;
    assert_eq!(
        old["result"]["results"][0]["assessment"]["policySource"],
        "account"
    );
    assert!(old["result"]["results"][0]["assessment"]
        .get("planId")
        .is_none());
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
        parse_json(request(&fixture, &grant, true, json!({"includePlans":true})).await).await
            ["result"]["totalCount"],
        3
    );
    document["providers"][0]["test_plans"] = json!([plan("second", "second")]);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    assert_eq!(
        request(&fixture, &grant, false, json!({"planId":"first"}))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(&fixture, &grant, true, json!({"planId":"first"}))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        parse_json(request(&fixture, &grant, true, json!({"includePlans":true})).await).await
            ["result"]["totalCount"],
        2
    );
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
}

#[test]
fn named_plan_validation_rejects_bad_collections_and_old_documents_remain_additive() {
    let base = json!({"providers":[{"id":"pool","preset":"openai","base_url":"https://example.invalid", "credentials":[{"id":"one","api_key":"secret"}],
        "test_plans":[plan("first","first")]}]});
    for plans in [
        json!([plan("first", "first"), plan("first", "second")]),
        json!([{ "id":"bad","name":" ","scopes":[{"kind":"pool"}],"policy":policy("ok",false)}]),
        json!([{ "id":"bad","name":"bad","scopes":[{"kind":"account","id":"foreign"}],"policy":policy("ok",false)}]),
        json!([{ "id":"bad","name":"bad","scopes":[{"kind":"pool"},{"kind":"account","id":"one"}],"policy":policy("ok",false)}]),
    ] {
        let mut bad = base.clone();
        bad["providers"][0]["test_plans"] = plans;
        let error = validate_route_document(serde_json::from_value(bad).unwrap()).unwrap_err();
        assert!(error
            .diagnostics
            .iter()
            .any(|error| error.code == "test_policy_invalid"));
    }
    let mut old = base;
    old["providers"][0]
        .as_object_mut()
        .unwrap()
        .remove("test_plans");
    let document: neuro_gateway::routing::config::RouteConfigYaml =
        serde_json::from_value(old).unwrap();
    assert!(serde_json::to_value(document).unwrap()["providers"][0]
        .get("test_plans")
        .is_none());
}

#[tokio::test]
async fn exhausted_short_interval_plan_cannot_starve_the_next_plan() {
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route("/v1/chat/completions", post(completion))
                .with_state(calls.clone()),
        )
        .into_future(),
    );
    let mut heavy = plan("heavy", "heavy");
    heavy["policy"]["intervalMinutes"] = json!(1);
    heavy["policy"]["models"] = json!((0..128)
        .map(|index| format!("m{index}"))
        .collect::<Vec<_>>());
    let mut fixture = ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[{"id":"pool","preset":"openai",
        "base_url":format!("http://{address}"),"test_plans":[heavy,plan("second","second")],
        "credentials":[{"id":"one","api_key":"one-secret"}]}]}))
        .unwrap(),
        false,
    );
    let root = std::env::temp_dir().join(format!("gateway-plan-fairness-{}", uuid::Uuid::new_v4()));
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    assert_eq!(
        sweep_scheduled_provider_credentials_once(&fixture.state)
            .await
            .unwrap()
            .due_count,
        1
    );
    assert_eq!(calls.lock().unwrap().len(), 128);
    // Make the first plan due again without sleeping or resetting any production slot.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(root.join("runtime.sqlite3")),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE credential_test_slots SET next=0")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    assert_eq!(
        sweep_scheduled_provider_credentials_once(&fixture.state)
            .await
            .unwrap()
            .due_count,
        2
    );
    let observed = calls.lock().unwrap();
    assert_eq!(observed[128].1, "second");
    assert_eq!(observed.len(), 256);
    drop(observed);
    fixture.state.local_runtime.as_ref().unwrap().close().await;
    server.abort();
    let _ = server.await;
}
