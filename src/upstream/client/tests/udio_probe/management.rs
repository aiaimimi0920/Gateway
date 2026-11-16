//! Normal management auth/Grant and exact-account boundary precede the sole task submission.
use super::*;
use axum::http::{Method, StatusCode};
use support::*;
const ENDPOINT: &str = "/v1/internal/gateway/console/providers/udio-pipeline/probe";

#[tokio::test]
async fn udio_probe_management_returns_neutral_task_summary_and_no_audio_assessment() {
    Box::pin(async {
        let peer = Peer::start(complete(), 200, false).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
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
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["credentialId"], "exact");
        assert_eq!(rows[0]["status"], "unsupported", "{body}");
        assert!(
            rows[0]["message"]
                .as_str()
                .unwrap()
                .contains("任务终态已确认"),
            "{body}"
        );
        assert!(rows[0]["assessment"].is_null() && rows[0]["healthRecovery"].is_null());
        closed_audit(&fixture.audit().await, true);
        fixture::wire(&peer);
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn udio_probe_management_auth_charge_and_foreign_accounts_fail_before_submission() {
    Box::pin(async {
        let peer=Peer::start(complete(),200,false).await;
        let fixture=Box::pin(Fixture::new(&peer)).await;
        let grant=Box::pin(grant_console_secret_access(&fixture.state)).await;
        for (change,has_grant,origin,status) in [
            (json!({}),false,None,StatusCode::FORBIDDEN),
            (json!({}),true,Some("https://foreign.invalid"),StatusCode::FORBIDDEN),
            (json!({"credentialIds":["backup"]}),true,None,StatusCode::BAD_REQUEST),
            (json!({"credentialIds":["exact","unrelated"]}),true,None,StatusCode::BAD_REQUEST),
            (json!({"credentialIds":["disabled"]}),true,None,StatusCode::OK),
            (json!({"media":{"operation":"music","confirmCharge":false}}),true,None,StatusCode::BAD_REQUEST),
            (json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"model":"other"}}}),true,None,StatusCode::BAD_REQUEST),
        ] {
            let mut input=options(); input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
            let response=Box::pin(send_console_json(&fixture.state,Method::POST,ENDPOINT,input,has_grant.then_some(grant.as_str()),origin)).await;
            assert_eq!(response.status(),status,"{change}");
        }
        assert!(peer.calls().is_empty());
        fixture.finish().await; peer.stop().await;
    }).await;
}
