//! Browser admission failures and uncertain results never create a direct or second paid task.
use super::*;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;

#[tokio::test]
async fn suno_probe_browser_missing_binding_pending_and_remote_errors_fail_without_audio_or_replay()
{
    Box::pin(async {
        for reply in [
            Reply::status(404, "private old executor"),
            Reply::status(503, "private executor failure"),
            Reply::json(json!({"ok":true,"result":{"clips":[clip("one","complete")],"completed":true}})),
            Reply::json(json!({"ok":true,"result":{"clips":[clip("one","complete")],"completed":true,"wireModel":"chirp-auk"}})),
            Reply::json(json!({"ok":true,"result":{"clips":[clip("one","submitted")],"completed":false,"wireModel":"chirp-auk-turbo"}})),
        ] {
            let peer = Server::start(vec![reply]).await;
            let fixture = browser_fixture(&peer, 0).await;
            let report = crate::upstream::public_assets::within(&peer.origin, run(&fixture, options())).await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Failed, "{}", report.message);
            assert!(report.answer.is_none() && !report.message.contains("private"));
            let row = fixture.audit().await;
            assert_eq!(row.status, "failed"); audit(&row, false); wire(&peer, 0);
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            fixture.finish().await; peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn suno_probe_browser_all_mp3_must_validate_before_success_or_health_recovery() {
    Box::pin(async {
        for failure in [1, 2] {
            let mut replies = completed();
            replies[failure] = Reply::status(200, "private signed HTML, not audio");
            let peer = Server::start(replies).await;
            let fixture = browser_fixture(&peer, 1).await;
            let report =
                crate::upstream::public_assets::within(&peer.origin, run(&fixture, options()))
                    .await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
            let row = fixture.audit().await;
            assert_eq!(row.status, "failed");
            audit(&row, false);
            wire(&peer, failure);
            let states = fixture
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_model_states(Default::default())
                .await
                .unwrap();
            assert!(states.iter().all(|row| row.last_success_at.is_none()));
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn suno_probe_browser_drop_during_executor_or_each_audio_cancels_only_local_owner() {
    Box::pin(async {
        for stage in 0..3 {
            let mut replies = completed(); replies[stage].hold = true;
            let peer = Server::start(replies).await;
            let fixture = browser_fixture(&peer, 0).await;
            let mut call = Box::pin(crate::upstream::public_assets::within(&peer.origin, run(&fixture, options())));
            tokio::select! { _=peer.wait_calls(stage+1)=>{}, result=call.as_mut()=>panic!("held probe returned early: {result:?}") }
            drop(call); peer.wait_closed().await;
            let row = fixture.audit().await; assert_eq!(row.status, "cancelled"); audit(&row, false);
            wire(&peer, stage);
            let states = fixture.state.local_runtime.as_ref().unwrap().list_model_states(Default::default()).await.unwrap();
            assert!(states.iter().all(|row| row.failure_count == 0));
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            fixture.finish().await; peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn suno_probe_browser_management_still_rejects_missing_grant_origin_and_unconfirmed_cost() {
    use axum::http::{Method, StatusCode};
    Box::pin(async {
        let peer = Server::start(vec![]).await;
        let fixture = browser_fixture(&peer, 2).await;
        let grant = Box::pin(support::grant_console_secret_access(&fixture.state)).await;
        for (has_grant, origin, change, status) in [
            (false, None, json!({}), StatusCode::FORBIDDEN),
            (true, Some("https://foreign.invalid"), json!({}), StatusCode::FORBIDDEN),
            (true, None, json!({"credentialIds":["backup"]}), StatusCode::BAD_REQUEST),
            (true, None, json!({"media":{"operation":"music","confirmCharge":false}}), StatusCode::BAD_REQUEST),
            (true, None, json!({"model":"undeclared"}), StatusCode::OK),
            (true, None, json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"waitCompletion":false}}}), StatusCode::OK),
        ] {
            let mut input = options(); input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
            let response = Box::pin(support::send_console_json(&fixture.state, Method::POST,
                "/v1/internal/gateway/console/providers/suno-pipeline/probe", input, has_grant.then_some(grant.as_str()), origin)).await;
            assert_eq!(response.status(), status);
            let body = support::parse_json(response).await;
            if status == StatusCode::OK { assert_eq!(body["result"]["results"][0]["status"], "unsupported", "{body}"); }
        }
        assert!(peer.calls().is_empty());
        assert!(fixture.state.local_runtime.as_ref().unwrap().list_audits(&Default::default()).await.unwrap().is_empty());
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        fixture.finish().await; peer.stop().await;
    }).await;
}
