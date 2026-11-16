//! Real management auth/Grant, single-account admission and neutral result contract.
use super::*;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
use axum::http::{Method, StatusCode};
use support::*;
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/luma-pipeline/probe";

#[tokio::test]
async fn luma_web_probe_management_preserves_neutral_success_and_nonpenalized_failure() {
    Box::pin(async {
        for failed in [false, true] {
            let image = if failed {
                Reply::bytes("text/html", b"private remote error".to_vec())
            } else {
                Reply::image(image::ImageFormat::Png)
            };
            let peer = Peer::start(vec![Reply::executor(ASSET), image]).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
            let response = within(
                &peer.origin,
                Box::pin(send_console_json(
                    &fixture.state,
                    Method::POST,
                    ENDPOINT,
                    options(),
                    Some(&grant),
                    None,
                )),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = parse_json(response).await;
            let rows = body["result"]["results"].as_array().unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["credentialId"], "exact");
            assert_eq!(
                rows[0]["status"],
                if failed { "failed" } else { "unsupported" },
                "{body}"
            );
            assert_eq!(
                rows[0]["healthRecovery"],
                if failed {
                    json!("failure_not_penalized")
                } else {
                    Value::Null
                }
            );
            assert!(rows[0]["assessment"].is_null());
            closed_audit(&fixture.audit().await, !failed);
            peer::wire(&peer, true);
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn luma_web_probe_management_rejects_unconfirmed_foreign_and_batch_requests() {
    Box::pin(async {
        let peer=Peer::start(vec![]).await;let fixture=Box::pin(Fixture::new(&peer)).await;
        let grant=Box::pin(grant_console_secret_access(&fixture.state)).await;
        for (change,has_grant,origin,status) in [
            (json!({}),false,None,StatusCode::FORBIDDEN),
            (json!({}),true,Some("https://foreign.invalid"),StatusCode::FORBIDDEN),
            (json!({"credentialIds":["backup"]}),true,None,StatusCode::BAD_REQUEST),
            (json!({"credentialIds":["exact","unrelated"]}),true,None,StatusCode::BAD_REQUEST),
            (json!({"credentialIds":["disabled"]}),true,None,StatusCode::OK),
            (json!({"media":{"operation":"image","confirmCharge":false}}),true,None,StatusCode::BAD_REQUEST),
            (json!({"media":{"operation":"image","confirmCharge":true,"parameters":{"model":"other"}}}),true,None,StatusCode::BAD_REQUEST),
            (json!({"planId":"saved-plan"}),true,None,StatusCode::BAD_REQUEST),
            (json!({"scope":{"kind":"pool"}}),true,None,StatusCode::BAD_REQUEST),
        ] {
            let mut input=options();input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
            let response=Box::pin(send_console_json(&fixture.state,Method::POST,ENDPOINT,input,has_grant.then_some(grant.as_str()),origin)).await;
            assert_eq!(response.status(),status,"{change}");
        }
        assert!(peer.calls().is_empty());fixture.finish().await;peer.stop().await;
    }).await;
}
