//! Loopback-only aliases must count one logical call under its concrete operation and account.
use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{
    db::request_model_totals::load_local_retained_model_totals, http::router::build_router,
    local_runtime::LocalRuntime,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;

async fn upstream(State(succeed): State<bool>) -> (StatusCode, Json<Value>) {
    if succeed {
        (
            StatusCode::OK,
            Json(json!({"organic":[{"url":"https://example.com"}],
            "code":200,"data":{"url":"https://example.com","content":"Example"}})),
        )
    } else {
        // A request-side rejection is terminal; it still belongs to the account actually called.
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"synthetic invalid query"})),
        )
    }
}

#[tokio::test]
async fn tool_alias_success_and_failure_count_concrete_operation_once() {
    for succeed in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/ai-search", post(upstream))
            .route("/scrape", post(upstream))
            .with_state(succeed);
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut fixture = support::ConsoleStateFixture::new(serde_json::from_value(json!({
            "providers":[{"id":"websearchapi","preset":"websearchapi",
                "base_url":format!("http://{address}"),
                "supported_models":["websearchapi-search","websearchapi-fetch","search","fetch"],
                "credentials":[{"id":"tool-account","api_key":"synthetic",
                    "supported_models":["websearchapi-search","websearchapi-fetch"]}]}],
            "model_routes":[],"aliases":{}
        })).unwrap(), false);
        let root =
            std::env::temp_dir().join(format!("gateway-tool-audit-{}", uuid::Uuid::new_v4()));
        let local = LocalRuntime::open(&root).await.unwrap();
        Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
        for operation in ["search", "fetch"] {
            let response = build_router(fixture.state.clone())
                .oneshot(
                    Request::post(format!("/v1/{operation}"))
                        .header("authorization", "Bearer public-api-contract-secret")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"model":operation,"query":"Rust",
                    "url":"https://example.com"})
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let result = support::parse_json(response).await;
            assert_eq!(status.is_success(), succeed, "{operation}: {result}");
        }
        let rows = load_local_retained_model_totals(&local).await.unwrap();
        let audits = local.list_audits(&Default::default()).await.unwrap();
        let summary = serde_json::to_value(
            neuro_gateway::db::request_audits::summarize_request_audit_rows(&audits),
        )
        .unwrap();
        for dimension in ["providerAccounts", "credentials"] {
            assert_eq!(summary[dimension][0]["totalRequests"], 2);
            for model in summary[dimension][0]["models"].as_array().unwrap() {
                assert!(["websearchapi-search", "websearchapi-fetch"]
                    .contains(&model["model"].as_str().unwrap()));
                assert_eq!(model["totalRequests"], 1);
                assert_eq!(model["windows"][0]["totalRequests"], 1);
            }
        }
        for audit in audits {
            assert!(["search", "fetch"].contains(&audit.requested_model.as_deref().unwrap()));
            assert_eq!(
                audit.route_trace.unwrap()["realCredentialRef"],
                "tool-account"
            );
        }
        assert_eq!(
            rows.len(),
            2,
            "each operation has exactly one logical request"
        );
        for row in rows {
            assert_eq!(row.provider_account_id, "websearchapi");
            assert_eq!(row.credential_ref.as_deref(), Some("tool-account"));
            assert!(["websearchapi-search", "websearchapi-fetch"].contains(&row.model.as_str()));
            assert_eq!(row.request_count, 1);
            assert_eq!(row.success_count, i64::from(succeed));
        }
        local.close().await;
        server.abort();
        let _ = server.await;
    }
}

type Attempts = Arc<Mutex<Vec<String>>>;

async fn fail_first(
    State(attempts): State<Attempts>,
    Path(provider): Path<String>,
) -> (StatusCode, Json<Value>) {
    let failed = {
        let mut attempts = attempts.lock().unwrap();
        let first = attempts
            .first()
            .cloned()
            .unwrap_or_else(|| provider.clone());
        attempts.push(provider.clone());
        first == provider
    };
    if failed {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"synthetic unavailable"})),
        )
    } else {
        upstream(State(true)).await
    }
}

#[tokio::test]
async fn tool_alias_fallback_keeps_one_winning_model_and_account_total() {
    let attempts = Attempts::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/:provider/ai-search", post(fail_first))
        .with_state(attempts.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let providers: Vec<Value> = ["first", "second"].into_iter().map(|id| json!({
        "id":id,"preset":"websearchapi","base_url":format!("http://{address}/{id}"),
        "supported_models":["websearchapi-search"],"credentials":[{"id":format!("{id}-account"),
            "api_key":"synthetic","supported_models":["websearchapi-search"]}]
    })).collect();
    let mut fixture = support::ConsoleStateFixture::new(
        serde_json::from_value(json!({
            "providers":providers,"model_routes":[],"aliases":{}
        }))
        .unwrap(),
        false,
    );
    let root = std::env::temp_dir().join(format!(
        "gateway-tool-fallback-audit-{}",
        uuid::Uuid::new_v4()
    ));
    let local = LocalRuntime::open(&root).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let response = build_router(fixture.state.clone())
        .oneshot(
            Request::post("/v1/search")
                .header("authorization", "Bearer public-api-contract-secret")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model":"search","query":"synthetic"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let result = support::parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let rows = load_local_retained_model_totals(&local).await.unwrap();
    let winner = attempts.lock().unwrap().last().cloned().unwrap();
    assert!(attempts
        .lock()
        .unwrap()
        .iter()
        .any(|provider| provider != &winner));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].provider_account_id, winner);
    assert_eq!(rows[0].credential_ref, Some(format!("{winner}-account")));
    assert_eq!(rows[0].model, "websearchapi-search");
    assert_eq!(rows[0].request_count, 1);
    assert_eq!(rows[0].success_count, 1);
    local.close().await;
    server.abort();
    let _ = server.await;
}
