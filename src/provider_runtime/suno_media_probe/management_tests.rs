//! Grant/Origin and saved/default/automatic text plans remain ahead of all paid music work.
use super::*;
use axum::http::{Method, StatusCode};
use support::*;
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/suno-pipeline/probe";

#[tokio::test]
async fn suno_probe_normal_management_preserves_exact_account_alias_and_closed_music_audit() {
    Box::pin(async {
        let peer = Server::start(completed()).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
        crate::upstream::public_assets::within(
            &peer.origin,
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
                let rows = body["result"]["results"].as_array().unwrap();
                assert_eq!(rows.len(), 1, "{body}");
                assert_eq!(rows[0]["credentialId"], "exact");
                assert_eq!(rows[0]["status"], "passed", "{body}");
                assert!(rows[0]["assessment"].is_null());
                let row = fixture.audit().await;
                assert_eq!(row.status, "completed");
                closed_audit(&row, true);
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                fixture::wire(&peer, 2);
            }),
        )
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn suno_probe_management_rejects_foreign_grant_origin_accounts_cost_and_parameters_without_network(
) {
    Box::pin(async {
        let peer=Server::start(vec![]).await;let fixture=Box::pin(Fixture::new(&peer)).await;
        let grant=Box::pin(grant_console_secret_access(&fixture.state)).await;
        for (change,has_grant,origin,status) in [
            (json!({}),false,None,StatusCode::FORBIDDEN),
            (json!({}),true,Some("https://foreign.invalid"),StatusCode::FORBIDDEN),
            (json!({"credentialIds":["backup"]}),true,None,StatusCode::BAD_REQUEST),
            (json!({"credentialIds":["exact","unrelated"]}),true,None,StatusCode::BAD_REQUEST),
            (json!({"credentialIds":["disabled"]}),true,None,StatusCode::OK),
            (json!({"model":"undeclared"}),true,None,StatusCode::OK),
            (json!({"media":{"operation":"music","confirmCharge":false}}),true,None,StatusCode::BAD_REQUEST),
            (json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"model":"other"}}}),true,None,StatusCode::BAD_REQUEST),
            (json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"waitAudio":false}}}),true,None,StatusCode::OK),
            (json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"duration":30}}}),true,None,StatusCode::OK),
        ] {
            let mut input=options();input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
            let response=Box::pin(send_console_json(&fixture.state,Method::POST,ENDPOINT,input,has_grant.then_some(grant.as_str()),origin)).await;
            assert_eq!(response.status(),status,"{change}");
            let body=parse_json(response).await;
            if status==StatusCode::OK {assert_eq!(body["result"]["results"][0]["status"],"unsupported","{body}");}
        }
        assert!(peer.calls().is_empty());assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        fixture.finish().await;peer.stop().await;
    }).await;
}

fn policy() -> Value {
    json!({"automaticEnabled":true,"intervalMinutes":15,"modelSelection":"selected","models":[MODEL],
        "cases":[{"id":"ok","name":"OK","prompt":"OK","expectedAnswer":"OK","difficulty":1,"enabled":true}]})
}
#[tokio::test]
async fn suno_probe_saved_temporary_named_default_and_automatic_text_plans_never_generate() {
    Box::pin(async {
        let peer=Server::start(vec![]).await;let fixture=Box::pin(Fixture::new(&peer)).await;
        let mut document=serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        document["providers"][0]["credentials"][1]["test_policy"]=policy();
        document["providers"][0]["test_plans"]=json!([{"id":"text","name":"Text","scopes":[{"kind":"account","id":"exact"}],"policy":policy()}]);
        fixture.state.route_config.replace_document(serde_json::from_value(document).unwrap()).unwrap();
        let grant=Box::pin(grant_console_secret_access(&fixture.state)).await;
        for input in [json!({"prompt":"OK","model":MODEL,"credentialIds":["exact"]}),
            json!({"testPlan":policy(),"scope":{"kind":"account","id":"exact"},"credentialIds":["exact"]}),
            json!({"planId":"text","credentialIds":["exact"]})] {
            let response=Box::pin(send_console_json(&fixture.state,Method::POST,ENDPOINT,input,Some(&grant),None)).await;
            assert_eq!(response.status(),StatusCode::OK);let body=parse_json(response).await;
            assert_eq!(body["result"]["results"][0]["status"],"unsupported","{body}");
        }
        let body=parse_json(Box::pin(probe_credential(&fixture.state,&grant,"exact")).await).await;
        assert_eq!(body["result"]["status"],"unsupported","{body}");
        let target=fixture.state.route_config.snapshot().select_credential_probe_target("exact").unwrap();
        let mut budget=crate::provider_runtime::test_execution::RoundBudget::new(fixture.state.route_config.snapshot().revision().id());
        let result=Box::pin(crate::provider_runtime::test_execution::run(&fixture.state,&target,&serde_json::from_value(policy()).unwrap(),"account","automatic",&mut budget)).await;
        assert_eq!(result.status,ProviderPayloadProbeStatus::Unsupported);
        assert!(peer.calls().is_empty());assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        fixture.finish().await;peer.stop().await;
    }).await;
}
