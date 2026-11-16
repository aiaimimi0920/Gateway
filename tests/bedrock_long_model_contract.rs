//! Long Bedrock identities keep their bytes through configuration, probes and routing.
#[path = "bedrock_long_model_contract/configuration.rs"]
mod configuration;
#[path = "bedrock_long_model_contract/fixture.rs"]
mod fixture;
#[path = "bedrock_long_model_contract/negative.rs"]
mod negative;
#[path = "console_contract_support/mod.rs"]
mod support;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use neuro_gateway::{
    http::router::build_router, local_runtime::LocalRuntime,
    provider_credential_probe_scheduler::sweep_scheduled_provider_credentials_once,
};
use serde_json::{json, Value};
use std::sync::Arc;
use support::*;
use tower::ServiceExt;

async fn probe(f: &ConsoleStateFixture, grant: &str, body: Value) -> Value {
    let response = send_console_json(
        &f.state,
        Method::POST,
        "/v1/internal/gateway/console/providers/bedrock/probe",
        body,
        Some(grant),
        None,
    )
    .await;
    let status = response.status();
    let body = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

#[test]
fn long_models_reach_exact_wire_via_prompt_saved_temporary_named_and_automatic_tests() {
    // Console commit + generation futures need a larger stack on the Windows test runner.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_long_model_tests());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn check_long_model_tests() {
    let upstream = fixture::Upstream::start().await;
    let arn = configuration::model(2048);
    let mut document = upstream.document(&arn);
    let mut f = ConsoleStateFixture::new(serde_json::from_value(document.clone()).unwrap(), true);
    let local = LocalRuntime::open(
        &std::env::temp_dir().join(format!("bedrock-long-{}", uuid::Uuid::new_v4())),
    )
    .await
    .unwrap();
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&f.state).await;
    let body = probe(
        &f,
        &grant,
        json!({"prompt":"Reply OK","model":arn,"credentialIds":["one"]}),
    )
    .await;
    assert_eq!(body["result"]["results"][0]["status"], "passed", "{body}");
    let selected = configuration::policy(&arn);
    let mut all = selected.clone();
    all["modelSelection"] = json!("all");
    all["models"] = json!([]);
    for policy in [&selected, &all] {
        let body = probe(
            &f,
            &grant,
            json!({"testPlan":policy,"scope":{"kind":"account","id":"one"}}),
        )
        .await;
        assert_eq!(body["result"]["results"][0]["status"], "passed", "{body}");
        assert_eq!(
            body["result"]["results"][0]["assessment"]["cases"][0]["model"],
            arn
        );
    }
    document["providers"][0]["test_policy"] = all;
    document["providers"][0]["test_plans"] = json!([{"id":"named","name":"Named","scopes":[{"kind":"account","id":"one"}],"policy":selected}]);
    f.commit_route_document(serde_json::from_value(document).unwrap(), None)
        .await;
    assert_eq!(
        f.state.route_config.get_providers()[0].credential_pool[0].supported_models,
        [arn.clone()]
    );
    // Legacy saved-policy execution is an empty-body request, not an invalid prompt-less JSON.
    let saved_request = Request::post("/v1/internal/gateway/console/providers/bedrock/probe")
        .header("x-management-token", MANAGEMENT_TOKEN)
        .header("x-secret-grant", &grant)
        .body(Body::empty())
        .unwrap();
    let saved_response = build_router(f.state.clone())
        .oneshot(saved_request)
        .await
        .unwrap();
    assert_eq!(saved_response.status(), StatusCode::OK);
    let saved_body = parse_json(saved_response).await;
    assert_eq!(
        saved_body["result"]["results"][0]["status"], "passed",
        "{saved_body}"
    );
    for request in [json!({"planId":"named","credentialIds":["one"]})] {
        let body = probe(&f, &grant, request).await;
        assert_eq!(body["result"]["results"][0]["status"], "passed", "{body}");
    }
    let sweep = sweep_scheduled_provider_credentials_once(&f.state)
        .await
        .unwrap();
    assert_eq!((sweep.scheduled_count, sweep.passed_count), (2, 2));
    let calls = upstream.calls.lock().unwrap();
    assert_eq!(calls.len(), 7);
    for (auth, model, body) in calls.iter() {
        assert_eq!(auth, "synthetic-only");
        assert_eq!(model, &arn);
        assert_eq!(body["inferenceConfig"]["maxTokens"], 32);
    }
    drop(calls);
    let audits = local
        .list_audits(&neuro_gateway::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 7);
    assert!(audits.iter().all(|audit| audit.status == "completed"
        && audit.resolved_model.as_deref() == Some(arn.as_str())
        && audit.provider_account_id.as_deref() == Some("bedrock")));
    let states = local
        .list_model_states(neuro_gateway::db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(states.iter().any(|state| state.model == arn
        && state.status == "active"
        && state.provider_credential_ref.as_deref() == Some("one")));
    drop(f);
    local.close().await;
}

#[test]
fn long_mapped_arn_passes_real_routing_circuit_and_capacity_admission() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let upstream = fixture::Upstream::start().await;
                    let arn = configuration::model(2048);
                    let f = ConsoleStateFixture::new(
                        serde_json::from_value(upstream.document(&arn)).unwrap(),
                        false,
                    );
                    let request = Request::post("/v1/chat/completions")
                        .header("authorization", "Bearer public-api-contract-secret")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"model":"alias","stream":true,
                "messages":[{"role":"user","content":"Reply OK"}]})
                            .to_string(),
                        ))
                        .unwrap();
                    let response = build_router(f.state.clone())
                        .oneshot(request)
                        .await
                        .unwrap();
                    assert_eq!(response.status(), StatusCode::OK);
                    let bytes = response.into_body().collect().await.unwrap().to_bytes();
                    let body = std::str::from_utf8(&bytes).unwrap();
                    assert!(body.contains("OK") && body.contains("[DONE]"), "{body}");
                    let calls = upstream.calls.lock().unwrap();
                    assert_eq!(calls.len(), 1);
                    assert_eq!(calls[0].1, arn);
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
