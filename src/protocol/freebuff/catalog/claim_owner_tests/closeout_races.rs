//! Real transaction rollback/CAS, authorization fences and cross-runtime exclusion.
use super::closeout::count;
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn freebuff_claim_end_changed_local_preimage_rejects_before_delete() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let scope = requests.recovery_requests();
    let owner = recovery(&db, &scope).await;
    sqlx::query("UPDATE local_freebuff_session_claims SET sealed=sealed || 'changed'")
        .execute(&db.pool)
        .await
        .unwrap();
    let changed = row(&db).await;
    let peer = fixture::Peer::start(vec![]).await;
    let error = with_test_origin(peer.origin.clone(), owner.end(&scope, || Ok(()), BUDGET))
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("freebuff_session_claim_changed")
    );
    assert_eq!(row(&db).await, changed);
    assert_eq!(count(&db, true).await, 0);
    assert_eq!(peer.count(), 0);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_end_fence_revocation_before_send_after_reply_and_before_commit_is_safe() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let original = row(&db).await;
    let scope = requests.recovery_requests();
    let peer = fixture::Peer::start(vec![]).await;
    for deny_at in [0, 1, 2] {
        if deny_at != 0 {
            peer.push(fixture::Spec::json(
                200,
                json!({"status":"ended", "freebucksRefund":0}),
            ));
        }
        let calls = AtomicUsize::new(0);
        let result = with_test_origin(
            peer.origin.clone(),
            recovery(&db, &scope).await.end(
                &scope,
                || {
                    if calls.fetch_add(1, Ordering::SeqCst) == deny_at {
                        Err(crate::error::GatewayError::conflict(
                            "synthetic fence revoked",
                        ))
                    } else {
                        Ok(())
                    }
                },
                BUDGET,
            ),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(row(&db).await, original);
        assert_eq!(count(&db, true).await, 0);
    }
    assert_eq!(peer.count(), 2);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_end_guard_spans_network_and_archive_and_stale_row_never_clears() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let scope = requests.recovery_requests();
    let other = LocalRuntime::open(&root).await.unwrap();
    let original = row(&db).await;
    let mut spec = fixture::Spec::json(200, json!({"status":"ended", "freebucksRefund":0}));
    spec.body_delay = Duration::from_millis(80);
    let peer = fixture::Peer::start(vec![spec]).await;
    let reached = Arc::new(tokio::sync::Notify::new());
    let owner = recovery(&db, &scope).await;
    TEST_BODY_STARTED.scope(reached.clone(), with_test_origin(peer.origin.clone(), async {
        let future = owner.end(&scope, || Ok(()), BUDGET);
        tokio::pin!(future);
        tokio::select! { _ = reached.notified() => (), result = &mut future => panic!("end completed early: {result:?}") }
        assert_eq!(other.recover_freebuff_admission("pool", "account", &scope).await.err().unwrap().code.as_deref(), Some("freebuff_session_claim_busy"));
        // Deliberately bypass the owner in this fixture to model a competing/corrupt durable write.
        sqlx::query("UPDATE local_freebuff_session_claims SET sealed=sealed || 'changed'").execute(&other.pool).await.unwrap();
        assert!(future.await.is_err());
    })).await;
    assert_eq!(
        count(&db, true).await,
        0,
        "archive INSERT must roll back with failed exact row deletion"
    );
    let changed = row(&db).await;
    assert_eq!(changed.1, original.1);
    assert_eq!(changed.2, original.2 + "changed");
    peer.close().await;
    other.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_end_archive_and_delete_are_atomic_on_sql_failure() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let scope = requests.recovery_requests();
    let original = row(&db).await;
    sqlx::raw_sql("CREATE TRIGGER reject_fixture_delete BEFORE DELETE ON local_freebuff_session_claims BEGIN SELECT RAISE(ABORT,'fixture'); END;")
        .execute(&db.pool).await.unwrap();
    let peer = fixture::Peer::start(vec![fixture::Spec::json(
        200,
        json!({"status":"ended", "freebucksRefund":4}),
    )])
    .await;
    assert!(with_test_origin(
        peer.origin.clone(),
        recovery(&db, &scope).await.end(&scope, || Ok(()), BUDGET)
    )
    .await
    .is_err());
    assert_eq!(row(&db).await, original);
    assert_eq!(count(&db, true).await, 0);
    assert_eq!(peer.count(), 1);
    drop(recovery(&db, &scope).await);
    peer.close().await;
    cleanup(&root, db).await;
}
