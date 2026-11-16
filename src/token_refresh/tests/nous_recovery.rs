//! Raw successors survive storage outages and restart; ambiguous sends never consume the old RT twice.
use super::{nous_fixture::*, nous_scheduler_fixture::*, *};
use crate::console::refresh_grants::*;
use serde_json::json;

#[tokio::test]
async fn nous_receipt_write_failure_retains_raw_material_and_retries_disk_only() {
    let f = fixture().await;
    let original = request(&f);
    let mock = Mock::new(vec![(200, reply())], 150).await;
    force_due(&f);
    let mut pending = Pending::new();
    let (_, guard) = tokio::join!(refresh(&f, &mock, &mut pending), async {
        mock.wait_called().await;
        f.writer_lock()
    });
    assert_eq!(pending.len(), 1);
    assert!(load(&f, &original).receipt().is_none());
    assert_eq!(f.saved().refresh_token.as_deref(), Some("nous-old-refresh"));
    refresh(&f, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    drop(guard);
    refresh(&f, &mock, &mut pending).await;
    assert!(pending.is_empty());
    assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn nous_restart_interprets_an_exact_durable_receipt_without_http() {
    let mut f = fixture().await;
    let original = request(&f);
    let snapshot = f.runtime.route_config().snapshot();
    let revision = crate::console::RouteConfigRedisRevision::new(
        snapshot.revision().clone(),
        snapshot.document().clone(),
    )
    .unwrap();
    let GrantReservation::Reserved(sending) = f
        .runtime
        .reserve_refresh_grant(&revision, &original)
        .await
        .unwrap()
    else {
        panic!("reserve")
    };
    f.runtime
        .store_refresh_grant_receipt(&sending, &GrantReceipt::NousPortal { tokens: receipt() })
        .await
        .unwrap();
    f.runtime = RouteConfigRuntime::new_local(&f.config).unwrap();
    f.runtime.recover_startup().await.unwrap();
    force_due(&f);
    let mock = Mock::new(vec![], 0).await;
    refresh(&f, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 0);
    assert_eq!(f.saved().refresh_token.as_deref(), Some("nous-new-refresh"));
    assert_eq!(load(&f, &original).phase(), GrantPhase::Published);
    mock.stop().await;
}

#[tokio::test]
async fn nous_invalid_successor_is_durable_but_never_publishable_or_resent() {
    for mutation in ["access", "scope", "target"] {
        let mut f = fixture().await;
        let original = request(&f);
        let mut body: serde_json::Value = serde_json::from_str(&reply()).unwrap();
        match mutation {
            "access" => body["access_token"] = json!("not-a-jwt"),
            "scope" => {
                // A response scope cannot remove a scope embedded in the JWT; use a JWT with no scope.
                use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
                body["access_token"] = json!(format!(
                    "{}.{}.{}",
                    URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
                    URL_SAFE_NO_PAD.encode(br#"{"sub":"synthetic-subject","exp":4102444900}"#),
                    URL_SAFE_NO_PAD.encode(b"synthetic-signature")
                ));
            }
            _ => body["inference_base_url"] = json!("https://example.com/v1"),
        }
        let mock = Mock::new(vec![(200, body.to_string())], 0).await;
        force_due(&f);
        refresh(&f, &mock, &mut Pending::new()).await;
        let record = load(&f, &original);
        assert_eq!(record.phase(), GrantPhase::Sending, "{mutation}");
        assert!(record.receipt().is_some() && record.result().is_none());
        assert_eq!(f.saved().refresh_token.as_deref(), Some("nous-old-refresh"));
        let snapshot = f.runtime.route_config().snapshot();
        let owner = snapshot
            .oauth_refresh_target("oauth", "account")
            .unwrap()
            .1
            .refresh_config
            .as_ref()
            .unwrap();
        assert_eq!(
            owner.dispatch_readiness().unwrap_err().code.as_deref(),
            Some("oauth_reauthorization_required")
        );
        f.runtime = RouteConfigRuntime::new_local(&f.config).unwrap();
        f.runtime.recover_startup().await.unwrap();
        force_due(&f);
        refresh(&f, &mock, &mut Pending::new()).await;
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn nous_unknown_send_result_remains_permanently_fenced_after_restart() {
    let mut f = fixture().await;
    let original = request(&f);
    let mock = Mock::new(vec![(503, "temporarily unavailable".into())], 0).await;
    force_due(&f);
    refresh(&f, &mock, &mut Pending::new()).await;
    assert!(load(&f, &original).receipt().is_none());
    f.runtime = RouteConfigRuntime::new_local(&f.config).unwrap();
    f.runtime.recover_startup().await.unwrap();
    force_due(&f);
    refresh(&f, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn nous_two_independent_local_runtimes_reserve_only_one_sender() {
    let a = fixture().await;
    let b = LocalFixture {
        config: a.config.clone(),
        runtime: RouteConfigRuntime::new_local(&a.config).unwrap(),
        root: a.root.clone(),
    };
    b.runtime.recover_startup().await.unwrap();
    force_due(&a);
    force_due(&b);
    let mock = Mock::new(vec![(200, reply())], 100).await;
    let (mut x, mut y) = (Pending::new(), Pending::new());
    tokio::join!(refresh(&a, &mock, &mut x), refresh(&b, &mock, &mut y));
    assert_eq!(mock.count(), 1);
    assert!(x.is_empty() && y.is_empty());
    let reopened = RouteConfigRuntime::new_local(&a.config).unwrap();
    reopened.recover_startup().await.unwrap();
    assert_eq!(
        reopened.route_config().snapshot().document().providers[0].credentials[0]
            .refresh_token
            .as_deref(),
        Some("nous-new-refresh")
    );
    mock.stop().await;
}

#[tokio::test]
async fn nous_historical_receipt_keeps_absolute_expiry_then_refreshes_only_its_successor() {
    let f = fixture().await;
    let original = request(&f);
    let snapshot = f.runtime.route_config().snapshot();
    let revision = crate::console::RouteConfigRedisRevision::new(
        snapshot.revision().clone(),
        snapshot.document().clone(),
    )
    .unwrap();
    let GrantReservation::Reserved(sending) = f
        .runtime
        .reserve_refresh_grant(&revision, &original)
        .await
        .unwrap()
    else {
        panic!("reserve")
    };
    let historical: crate::protocol::nous_portal::ReceivedTokens = serde_json::from_value(json!({
        "received_at":1,"response":{"access_token":access("synthetic-subject",61),
        "refresh_token":"nous-historical-refresh"}}))
    .unwrap();
    f.runtime
        .store_refresh_grant_receipt(&sending, &GrantReceipt::NousPortal { tokens: historical })
        .await
        .unwrap();
    force_due(&f);
    let mock = Mock::new(vec![(200, reply())], 0).await;
    refresh(&f, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 0);
    assert_eq!(
        f.saved().expires_at.as_deref(),
        Some("1970-01-01T00:01:01Z")
    );
    assert_eq!(f.saved().token_expires_in_secs, Some(60));
    let snapshot = f.runtime.route_config().snapshot();
    assert!(snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .refresh_config
        .as_ref()
        .unwrap()
        .dispatch_readiness()
        .is_err());
    refresh(&f, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    assert_eq!(
        mock.requests()[0].0["x-nous-refresh-token"],
        "nous-historical-refresh"
    );
    assert_eq!(f.saved().refresh_token.as_deref(), Some("nous-new-refresh"));
    mock.stop().await;
}
