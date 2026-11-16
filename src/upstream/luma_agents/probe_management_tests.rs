//! Existing management authentication, grant and exact-account admission precede paid generation.
use super::{
    pipeline_fixture::{support::*, *},
    probe_tests::options,
    test_support::*,
};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};

const ENDPOINT: &str = "/v1/internal/gateway/console/providers/luma-pipeline/probe";

#[tokio::test]
async fn luma_agents_probe_management_exact_account_has_native_audit_without_text_assessment() {
    Box::pin(async {
        let server = Server::start(vec![
            Reply::json(201, job("queued")),
            Reply::json(200, job("completed")),
            Reply::png(),
        ])
        .await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        let grant = grant_console_secret_access(&fixture.state).await;
        within(&server.origin, async {
            let response = send_console_json(
                &fixture.state,
                Method::POST,
                ENDPOINT,
                options("image", IMAGE),
                Some(&grant),
                None,
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = parse_json(response).await;
            let results = body["result"]["results"].as_array().unwrap();
            assert_eq!(results.len(), 1, "{body}");
            assert_eq!(results[0]["credentialId"], "exact");
            assert_eq!(results[0]["status"], "passed", "{body}");
            assert!(results[0]["assessment"].is_null());
            let row = fixture.audit().await;
            assert_eq!(row.requested_model.as_deref(), Some(IMAGE));
            assert_eq!(row.resolved_model.as_deref(), Some("uni-1"));
            assert_eq!(row.model_alias.as_deref(), Some(IMAGE));
            assert_eq!(row.response_id, ID);
            let trace = row.route_trace.as_ref().unwrap();
            assert_eq!(
                trace["consoleMediaProbe"],
                json!({"operation":"image", "chargeConfirmed":true})
            );
            assert!(
                row.access_key_id.is_none()
                    && row.prompt_tokens.is_none()
                    && row.total_tokens.is_none()
            );
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            assert_eq!(server.calls().len(), 3);
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}

#[tokio::test]
async fn luma_agents_probe_management_denies_missing_grant_origin_scope_and_unconfirmed_cost_without_network(
) {
    Box::pin(async {
        let server = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        let grant = grant_console_secret_access(&fixture.state).await;
        let valid = options("image", IMAGE);
        let mut foreign = valid.clone();
        foreign["credentialIds"] = json!(["backup-exact"]);
        let mut disabled = valid.clone();
        disabled["credentialIds"] = json!(["disabled"]);
        let mut wrong_model = valid.clone();
        wrong_model["model"] = json!("not-declared");
        let mut unconfirmed = valid.clone();
        unconfirmed["media"]["confirmCharge"] = json!(false);
        let mut batch = valid.clone();
        batch["credentialIds"] = json!(["exact", "unrelated"]);
        let mut bad_parameters = valid.clone();
        bad_parameters["media"]["parameters"] = json!({"source":{"generation_id":"invalid"}});
        let mut mismatch = valid.clone();
        mismatch["media"]["operation"] = json!("video");
        within(&server.origin, async {
            for (input, authorization, origin, expected) in [
                (valid.clone(), None, None, StatusCode::FORBIDDEN),
                (
                    valid,
                    Some(grant.as_str()),
                    Some("https://other.example.test"),
                    StatusCode::FORBIDDEN,
                ),
                (foreign, Some(grant.as_str()), None, StatusCode::BAD_REQUEST),
                (
                    unconfirmed,
                    Some(grant.as_str()),
                    None,
                    StatusCode::BAD_REQUEST,
                ),
                (batch, Some(grant.as_str()), None, StatusCode::BAD_REQUEST),
                (disabled, Some(grant.as_str()), None, StatusCode::OK),
                (wrong_model, Some(grant.as_str()), None, StatusCode::OK),
                (bad_parameters, Some(grant.as_str()), None, StatusCode::OK),
                (mismatch, Some(grant.as_str()), None, StatusCode::OK),
            ] {
                let response = send_console_json(
                    &fixture.state,
                    Method::POST,
                    ENDPOINT,
                    input,
                    authorization,
                    origin,
                )
                .await;
                assert_eq!(response.status(), expected);
                let body = parse_json(response).await;
                if expected == StatusCode::OK {
                    assert_eq!(
                        body["result"]["results"][0]["status"], "unsupported",
                        "{body}"
                    );
                }
            }
            assert!(server.calls().is_empty());
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}

fn policy() -> Value {
    json!({"automaticEnabled":true,"intervalMinutes":15,"modelSelection":"selected","models":[IMAGE],
        "cases":[{"id":"ok","name":"OK","prompt":"OK","expectedAnswer":"OK","difficulty":1,"enabled":true}]})
}
#[tokio::test]
async fn luma_agents_probe_saved_temporary_named_and_automatic_text_plans_never_generate() {
    Box::pin(async {
        let server = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        let mut document = serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        document["providers"][0]["credentials"][1]["test_policy"] = policy();
        document["providers"][0]["test_plans"] = json!([{"id":"text","name":"Text","scopes":[{"kind":"account","id":"exact"}],"policy":policy()}]);
        fixture.state.route_config.replace_document(serde_json::from_value(document).unwrap()).unwrap();
        let grant = grant_console_secret_access(&fixture.state).await;
        within(&server.origin, async {
            for input in [json!({"prompt":"OK","model":IMAGE,"credentialIds":["exact"]}),
                json!({"testPlan":policy(),"scope":{"kind":"account","id":"exact"},"credentialIds":["exact"]}),
                json!({"planId":"text","credentialIds":["exact"]})] {
                let response = send_console_json(&fixture.state, Method::POST, ENDPOINT, input, Some(&grant), None).await;
                assert_eq!(response.status(), StatusCode::OK);
                let body = parse_json(response).await;
                assert_eq!(body["result"]["results"][0]["status"], "unsupported", "{body}");
            }
            let body = parse_json(probe_credential(&fixture.state, &grant, "exact").await).await;
            assert_eq!(body["result"]["status"], "unsupported", "{body}");
            let target = fixture.state.route_config.snapshot().select_credential_probe_target("exact").unwrap();
            let mut budget = crate::provider_runtime::test_execution::RoundBudget::new(fixture.state.route_config.snapshot().revision().id());
            let result = crate::provider_runtime::test_execution::run(&fixture.state, &target, &serde_json::from_value(policy()).unwrap(), "account", "automatic", &mut budget).await;
            assert_eq!(result.status, crate::provider_runtime::ProviderPayloadProbeStatus::Unsupported);
            assert!(server.calls().is_empty());
        }).await;
        fixture.finish().await; server.stop().await;
    }).await;
}
