//! Normal management Grant/Origin and plan owners must precede explicitly charged media work.
use super::{
    pipeline_fixture::{support::*, *},
    probe_tests::{completed, options},
    test_support::*,
};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/chataibot-pipeline/probe";

#[tokio::test]
async fn chataibot_probe_management_preserves_exact_alias_audit_and_no_text_assessment() {
    Box::pin(async {
        let server = Server::start(completed()).await;
        let fixture = Box::pin(Fixture::new(5)).await;
        let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
        within(
            &server.origin,
            Box::pin(async {
                let response = Box::pin(send_console_json(
                    &fixture.state,
                    Method::POST,
                    ENDPOINT,
                    options(),
                    Some(&grant),
                    None,
                ))
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                let body = parse_json(response).await;
                let results = body["result"]["results"].as_array().unwrap();
                assert_eq!(results.len(), 1, "{body}");
                assert_eq!(results[0]["credentialId"], "exact");
                assert_eq!(results[0]["status"], "passed", "{body}");
                assert!(results[0]["assessment"].is_null());
                let row = fixture.audit().await;
                assert_eq!(row.requested_model.as_deref(), Some(MODEL));
                assert_eq!(row.resolved_model.as_deref(), Some(RESOLVED));
                assert_eq!(
                    row.route_trace.as_ref().unwrap()["consoleMediaProbe"],
                    json!({"operation":"image","chargeConfirmed":true})
                );
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                let states = fixture
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_model_states(Default::default())
                    .await
                    .unwrap();
                assert_eq!(states.len(), 1, "{states:?}");
                assert_eq!(states[0].model, RESOLVED);
                assert_eq!(states[0].provider_credential_ref.as_deref(), Some("exact"));
                assert_eq!(states[0].failure_count, 0);
                assert_eq!(server.calls().len(), 3);
            }),
        )
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
#[tokio::test]
async fn chataibot_probe_management_rejects_foreign_grant_origin_account_model_parameters_and_cost_without_network(
) {
    Box::pin(async {
        let server=Server::start(vec![]).await;let fixture=Box::pin(Fixture::new(5)).await;
        let grant=Box::pin(grant_console_secret_access(&fixture.state)).await;
        within(&server.origin,Box::pin(async {
            for (change,has_grant,origin,status) in [
                (json!({}),false,None,StatusCode::FORBIDDEN),
                (json!({}),true,Some("https://foreign.example.test"),StatusCode::FORBIDDEN),
                (json!({"credentialIds":["backup"]}),true,None,StatusCode::BAD_REQUEST),
                (json!({"credentialIds":["exact","unrelated"]}),true,None,StatusCode::BAD_REQUEST),
                (json!({"credentialIds":["disabled"]}),true,None,StatusCode::OK),
                (json!({"model":"undeclared"}),true,None,StatusCode::OK),
                (json!({"media":{"operation":"image","confirmCharge":false}}),true,None,StatusCode::BAD_REQUEST),
                (json!({"media":{"operation":"image","confirmCharge":true,"parameters":{"model":"other"}}}),true,None,StatusCode::BAD_REQUEST),
                (json!({"media":{"operation":"image","confirmCharge":true,"parameters":{"size":"1024x1024"}}}),true,None,StatusCode::OK),
                (json!({"media":{"operation":"video","confirmCharge":true}}),true,None,StatusCode::OK),
                (json!({"media":{"operation":"image_edit","confirmCharge":true}}),true,None,StatusCode::OK),
                (json!({"media":{"operation":"image_edit","confirmCharge":true,"parameters":{"images":["https://foreign.test/x.png"]}}}),true,None,StatusCode::OK),
            ] {
                let mut input=options();input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
                let response=Box::pin(send_console_json(&fixture.state,Method::POST,ENDPOINT,input,has_grant.then_some(grant.as_str()),origin)).await;
                assert_eq!(response.status(),status,"{change}");
                let body=parse_json(response).await;
                if status==StatusCode::OK { assert_eq!(body["result"]["results"][0]["status"],"unsupported","{body}"); }
            }
            assert!(server.calls().is_empty());assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        })).await;
        fixture.finish().await;server.stop().await;
    }).await;
}
fn policy() -> Value {
    json!({"automaticEnabled":true,"intervalMinutes":15,"modelSelection":"selected","models":[MODEL],
        "cases":[{"id":"ok","name":"OK","prompt":"OK","expectedAnswer":"OK","difficulty":1,"enabled":true}]})
}
#[tokio::test]
async fn chataibot_probe_saved_temporary_named_and_automatic_text_plans_never_generate() {
    Box::pin(async {
        let server=Server::start(vec![]).await;let fixture=Box::pin(Fixture::new(5)).await;
        let mut document=serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        document["providers"][0]["credentials"][1]["test_policy"]=policy();
        document["providers"][0]["test_plans"]=json!([{"id":"text","name":"Text","scopes":[{"kind":"account","id":"exact"}],"policy":policy()}]);
        fixture.state.route_config.replace_document(serde_json::from_value(document).unwrap()).unwrap();
        let grant=Box::pin(grant_console_secret_access(&fixture.state)).await;
        within(&server.origin,Box::pin(async {
            for input in [json!({"prompt":"OK","model":MODEL,"credentialIds":["exact"]}),
                json!({"testPlan":policy(),"scope":{"kind":"account","id":"exact"},"credentialIds":["exact"]}),
                json!({"planId":"text","credentialIds":["exact"]})] {
                let response=Box::pin(send_console_json(&fixture.state,Method::POST,ENDPOINT,input,Some(&grant),None)).await;
                assert_eq!(response.status(),StatusCode::OK);
                let body=parse_json(response).await;assert_eq!(body["result"]["results"][0]["status"],"unsupported","{body}");
            }
            let body=parse_json(Box::pin(probe_credential(&fixture.state,&grant,"exact")).await).await;
            assert_eq!(body["result"]["status"],"unsupported","{body}");
            let target=fixture.state.route_config.snapshot().select_credential_probe_target("exact").unwrap();
            let mut budget=crate::provider_runtime::test_execution::RoundBudget::new(fixture.state.route_config.snapshot().revision().id());
            let result=Box::pin(crate::provider_runtime::test_execution::run(&fixture.state,&target,&serde_json::from_value(policy()).unwrap(),"account","automatic",&mut budget)).await;
            assert_eq!(result.status,crate::provider_runtime::ProviderPayloadProbeStatus::Unsupported);
            assert!(server.calls().is_empty());
        })).await;
        fixture.finish().await;server.stop().await;
    }).await;
}
