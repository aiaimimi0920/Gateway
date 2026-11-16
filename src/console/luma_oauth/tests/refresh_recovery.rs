//! Restart recovery uses received evidence only; unknown sends and replaced authorizations stay fenced.
use super::{fixture::Peer, refresh_fixture::Local, support};
use crate::console::{
    refresh_grants::{GrantPhase, GrantReceipt, GrantRequest, GrantReservation},
    RouteConfigRedisRevision,
};
use crate::protocol::luma_oauth::{test_support::ENDPOINT, ReceivedRefresh};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

fn request(f: &Local) -> (RouteConfigRedisRevision, GrantRequest) {
    let s = f.state.route_config.snapshot();
    let r = RouteConfigRedisRevision::new(s.revision().clone(), s.document().clone()).unwrap();
    let q = GrantRequest::from_revision(&r, "luma-mcp", "saved-mcp").unwrap();
    (r, q)
}
fn receipt(q: &GrantRequest) -> GrantReceipt {
    GrantReceipt::LumaOauth { tokens: ReceivedRefresh::capture(
        br#"{"access_token":"recovered-access","refresh_token":"recovered-refresh","token_type":"Bearer","expires_in":3600}"#,
        time::OffsetDateTime::UNIX_EPOCH,
        q.material.luma_oauth.as_ref().unwrap(), &q.material.refresh,
    ).unwrap() }
}

#[tokio::test]
async fn luma_explicit_refresh_restart_recovers_received_or_result_without_resending_unknown_sending(
) {
    for stage in 0..3 {
        let f = Local::new();
        let runtime = f.state.route_config_runtime.as_ref().unwrap();
        let (revision, q) = request(&f);
        let GrantReservation::Reserved(mut record) =
            runtime.reserve_refresh_grant(&revision, &q).await.unwrap()
        else {
            panic!("reservation")
        };
        if stage > 0 {
            let received = receipt(&q);
            record = runtime
                .store_refresh_grant_receipt(&record, &received)
                .await
                .unwrap();
            if stage == 2 {
                let GrantReceipt::LumaOauth { tokens } = received else {
                    unreachable!()
                };
                let result = q.result_with_luma(&tokens).unwrap();
                runtime
                    .store_refresh_grant_result(&record, &result)
                    .await
                    .unwrap();
            }
        }
        let state = f.reopen();
        let peer = Peer::new(Duration::ZERO).await;
        ENDPOINT.scope(peer.endpoint.clone(), async {
            let grant = support::grant_console_secret_access(&state).await;
            let root = "/v1/internal/gateway/console/luma-refresh-sessions";
            let response = support::send_console_json(&state, Method::POST, root, json!({"providerId":"luma-mcp","credentialId":"saved-mcp","expectedRevision":revision.metadata().id()}), Some(&grant), None).await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = support::parse_json(response).await;
            let path = format!("{root}/{}", body["session"]["id"].as_str().unwrap());
            let response = support::send_console_json(&state, Method::POST, &format!("{path}/import"), json!({"expectedRevision":revision.metadata().id()}), Some(&grant), None).await;
            assert_eq!(response.status(), StatusCode::OK);
            let expected = if stage == 0 { "save_unconfirmed" } else { "succeeded" };
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    let response = support::send_console_json(&state, Method::GET, &path, Value::Null, None, None).await;
                    let body = support::parse_json(response).await;
                    if body["session"]["status"] == expected { break; }
                    assert_eq!(body["session"]["status"], "importing", "{body}");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.unwrap();
            assert_eq!(peer.count(), 0);
            if stage > 0 {
                let s = state.route_config.snapshot();
                let c = &s.document().providers[0].credentials[0];
                assert_eq!(c.refresh_token.as_deref(), Some("recovered-refresh"));
                assert!(c.luma_oauth.as_ref().unwrap().restore_material(c.api_key.as_deref().unwrap()).is_err());
            }
            state.lifecycle.luma_oauth.join().await;
        }).await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_explicit_refresh_committed_publication_repair_is_local_and_time_independent() {
    let f = Local::new();
    let runtime = f.state.route_config_runtime.as_ref().unwrap();
    let (revision, q) = request(&f);
    let GrantReservation::Reserved(record) =
        runtime.reserve_refresh_grant(&revision, &q).await.unwrap()
    else {
        panic!("reservation")
    };
    let received = receipt(&q);
    let GrantReceipt::LumaOauth { tokens } = &received else {
        unreachable!()
    };
    let result = q.result_with_luma(tokens).unwrap();
    assert!(runtime
        .store_refresh_grant_result(&record, &result)
        .await
        .is_err());
    let record = runtime
        .store_refresh_grant_receipt(&record, &received)
        .await
        .unwrap();
    let mut forged = result.clone();
    forged.material.access = "forged".into();
    assert!(runtime
        .store_refresh_grant_result(&record, &forged)
        .await
        .is_err());
    let record = runtime
        .store_refresh_grant_result(&record, &result)
        .await
        .unwrap();
    runtime
        .commit_refresh_ticket(&super::super::refresh_publication::Publication(record))
        .await
        .unwrap();
    // Simulate losing the acknowledgement after configuration commit, before mark Published.
    assert_eq!(
        runtime
            .find_refresh_grant(&q)
            .await
            .unwrap()
            .unwrap()
            .phase(),
        GrantPhase::Result
    );
    let reopened = f.reopen();
    let runtime = reopened.route_config_runtime.as_ref().unwrap();
    runtime.recover_refresh_authority().await.unwrap();
    runtime.repair_refresh_cache().await.unwrap();
    runtime.repair_refresh_cache().await.unwrap();
    assert_eq!(
        runtime
            .find_refresh_grant(&q)
            .await
            .unwrap()
            .unwrap()
            .phase(),
        GrantPhase::Published
    );
}

#[tokio::test]
async fn luma_explicit_refresh_projection_rejects_policy_changes_and_replaced_authority() {
    let f = Local::new();
    let runtime = f.state.route_config_runtime.as_ref().unwrap();
    let (revision, q) = request(&f);
    let GrantReservation::Reserved(record) =
        runtime.reserve_refresh_grant(&revision, &q).await.unwrap()
    else {
        panic!("reservation")
    };
    let received = receipt(&q);
    let GrantReceipt::LumaOauth { tokens } = &received else {
        unreachable!()
    };
    let result = q.result_with_luma(tokens).unwrap();
    let record = runtime
        .store_refresh_grant_receipt(&record, &received)
        .await
        .unwrap();
    let record = runtime
        .store_refresh_grant_result(&record, &result)
        .await
        .unwrap();
    use crate::console::runtime::refresh_authority::RefreshPublicationTicket;
    let ticket = super::super::refresh_publication::Publication(record);
    let next = ticket
        .project(&f.state.route_config.snapshot())
        .unwrap()
        .unwrap();
    assert!(crate::token_refresh::is_refresh_projection(
        revision.document(),
        &next,
        "luma-mcp",
        "saved-mcp"
    ));
    for key in ["enabled", "account_name", "refresh_endpoint"] {
        let mut raw = serde_json::to_value(&next).unwrap();
        raw["providers"][0]["credentials"][0][key] = if key == "enabled" {
            json!(true)
        } else {
            json!("changed")
        };
        let changed = serde_json::from_value(raw).unwrap();
        assert!(!crate::token_refresh::is_refresh_projection(
            revision.document(),
            &changed,
            "luma-mcp",
            "saved-mcp"
        ));
    }
    let mut replaced = revision.document().clone();
    replaced.providers[0].credentials[0].api_key = Some("new-authorization".into());
    runtime
        .commit_document(revision.metadata().id(), replaced, None)
        .await
        .unwrap();
    assert!(runtime.commit_refresh_ticket(&ticket).await.is_err());
    assert_eq!(
        f.state.route_config.snapshot().document().providers[0].credentials[0]
            .api_key
            .as_deref(),
        Some("new-authorization")
    );
    assert_eq!(
        runtime
            .find_refresh_grant(&q)
            .await
            .unwrap()
            .unwrap()
            .phase(),
        GrantPhase::Result
    );
}
