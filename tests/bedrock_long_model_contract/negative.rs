//! Structural parsing never grants an ordinary product access to extended model IDs.
use super::{configuration, fixture, support::*};
use axum::http::{Method, StatusCode};
use neuro_gateway::local_runtime::LocalRuntime;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn ordinary_product_rejects_long_prompt_and_matrix_models_before_network() {
    let upstream = fixture::Upstream::start().await;
    let mut document = configuration::document();
    document["providers"][0]["preset"] = json!("openai");
    document["providers"][0]["base_url"] = json!(upstream.base);
    document["providers"][0]["supported_models"] = json!(["short"]);
    let mut f = ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), false);
    let local = LocalRuntime::open(
        &std::env::temp_dir().join(format!("bedrock-long-negative-{}", uuid::Uuid::new_v4())),
    )
    .await
    .unwrap();
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&f.state).await;
    let arn = configuration::model(1500);
    for request in [
        json!({"prompt":"Reply OK","model":arn}),
        json!({"testPlan":configuration::policy(&arn),"scope":{"kind":"account","id":"one"}}),
    ] {
        let response = send_console_json(
            &f.state,
            Method::POST,
            "/v1/internal/gateway/console/providers/bedrock/probe",
            request,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(
            body["result"]["results"][0]["status"], "unsupported",
            "{body}"
        );
    }
    let mut invalid_case = configuration::policy("short");
    invalid_case["cases"][0]["id"] = json!("a".repeat(257));
    for request in [
        json!({"prompt":"Reply OK","model":configuration::model(2049)}),
        json!({"planId":"a".repeat(257)}),
        json!({"testPlan":configuration::policy("short"),"scope":{"kind":"account","id":"a".repeat(257)}}),
        json!({"testPlan":invalid_case}),
    ] {
        let response = send_console_json(
            &f.state,
            Method::POST,
            "/v1/internal/gateway/console/providers/bedrock/probe",
            request,
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(upstream.calls.lock().unwrap().is_empty());
    drop(f);
    local.close().await;
}
