//! Failed/unknown end and changed authority never discard an obligation or synthesize refund.
use super::closeout::count;
use super::*;

#[tokio::test]
async fn freebuff_claim_end_rejects_unconfirmed_status_and_body_without_clearing_original() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let original = row(&db).await;
    let scope = requests.recovery_requests();
    let peer = fixture::Peer::start(vec![]).await;
    for (status, body) in [
        (404, json!({"status":"none"})),
        (403, json!({"status":"ended", "freebucksRefund":0})),
        (500, json!({"status":"ended", "freebucksRefund":0})),
        (200, json!({"status":"none"})),
        (200, json!({"status":"active"})),
        (
            200,
            json!({"status":"ended", "instanceId":"foreign", "freebucksRefund":0}),
        ),
        (200, json!({"status":"ended", "freebucksRefund":-1})),
        (200, json!({"status":"ended", "freebucksRefund":null})),
        (
            200,
            json!({"status":"ended", "freebucksRefund":0, "freebucksRefundSources":null}),
        ),
    ] {
        peer.push(fixture::Spec::json(status, body));
        assert!(with_test_origin(
            peer.origin.clone(),
            recovery(&db, &scope).await.end(&scope, || Ok(()), BUDGET)
        )
        .await
        .is_err());
        assert_eq!(row(&db).await, original);
        assert_eq!(count(&db, true).await, 0);
    }
    assert_eq!(
        peer.count(),
        9,
        "no hidden end retries or replacement admission"
    );
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_end_foreign_authority_and_zero_budget_fail_before_network() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let original = row(&db).await;
    let scope = requests.recovery_requests();
    let peer = fixture::Peer::start(vec![]).await;
    for (token, client, origin) in [
        ("foreign", ClientSurface::Cli, BusinessOrigin::Codebuff),
        (
            "synthetic-account",
            ClientSurface::Desktop,
            BusinessOrigin::Codebuff,
        ),
        (
            "synthetic-account",
            ClientSurface::Cli,
            BusinessOrigin::CodebuffWww,
        ),
    ] {
        let (other, key) = synthetic(token, client, origin);
        let wrong = other.recovery_requests(&key).unwrap();
        assert!(with_test_origin(
            peer.origin.clone(),
            recovery(&db, &scope).await.end(&wrong, || Ok(()), BUDGET)
        )
        .await
        .is_err());
    }
    assert!(with_test_origin(
        peer.origin.clone(),
        recovery(&db, &scope)
            .await
            .end(&scope, || Ok(()), Duration::ZERO)
    )
    .await
    .is_err());
    assert_eq!(peer.count(), 0);
    assert_eq!(row(&db).await, original);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_claim_end_redirect_oversize_bad_json_timeout_and_drop_retain_original() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    drop(pending(&db, &requests).await);
    let original = row(&db).await;
    let scope = requests.recovery_requests();
    let redirect = fixture::Peer::start(vec![]).await;
    let mut spec = fixture::Spec::json(307, json!({}));
    spec.headers.insert(
        "location",
        format!("{}/leak", redirect.origin).parse().unwrap(),
    );
    let peer = fixture::Peer::start(vec![spec]).await;
    let mut bad = fixture::Spec::json(200, json!({}));
    bad.body = b"{bad".to_vec();
    let mut large = fixture::Spec::json(200, json!({}));
    large.body = vec![b' '; 1_048_577];
    for (index, next) in [None, Some(bad), Some(large)].into_iter().enumerate() {
        if let Some(next) = next {
            peer.push(next);
        }
        assert!(with_test_origin(
            peer.origin.clone(),
            recovery(&db, &scope).await.end(&scope, || Ok(()), BUDGET)
        )
        .await
        .is_err());
        assert_eq!(peer.count(), index + 1);
        assert_eq!(row(&db).await, original);
    }
    for cancel in [false, true] {
        let mut slow = fixture::Spec::json(200, json!({"status":"ended", "freebucksRefund":0}));
        slow.body_delay = Duration::from_secs(2);
        peer.push(slow);
        let reached = Arc::new(tokio::sync::Notify::new());
        let owner = recovery(&db, &scope).await;
        TEST_BODY_STARTED.scope(reached.clone(), with_test_origin(peer.origin.clone(), async {
            let future = owner.end(&scope, || Ok(()), if cancel {BUDGET} else {Duration::from_millis(40)});
            tokio::pin!(future);
            if cancel {
                tokio::select! { _ = reached.notified() => (), result = &mut future => panic!("end completed early: {result:?}") }
            } else { assert!(future.await.is_err()); }
        })).await;
        assert_eq!(row(&db).await, original);
    }
    assert_eq!(count(&db, true).await, 0);
    assert_eq!(peer.count(), 5);
    assert_eq!(redirect.count(), 0);
    peer.close().await;
    redirect.close().await;
    cleanup(&root, db).await;
}
