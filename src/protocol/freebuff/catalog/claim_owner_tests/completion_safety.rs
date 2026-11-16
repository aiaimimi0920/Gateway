//! No success on transport/semantic truncation, no redirected or replayed FINISH, no hidden cleanup.
use super::completion::{activate, answer, send};
use super::*;

#[tokio::test]
async fn freebuff_catalog_completion_finish_redirect_timeout_and_http_failure_do_not_replay() {
    for mode in [0, 1, 2, 3] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        let target = fixture::Peer::start(vec![]).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        peer.push(fixture::Spec::json(200, answer()));
        let mut finish = fixture::Spec::json(
            if mode == 0 {
                307
            } else if mode == 1 {
                500
            } else {
                200
            },
            json!({}),
        );
        if mode == 0 {
            finish.headers.insert(
                "location",
                format!("{}/leak", target.origin).parse().unwrap(),
            );
        }
        if mode == 2 {
            finish.body_delay = Duration::from_millis(150);
        }
        peer.push(finish);
        let decoded = send(&mut owner, &requests, &peer)
            .await
            .decode(BUDGET)
            .await;
        assert!(with_test_origin(
            peer.origin.clone(),
            decoded.finish_single_step(if mode == 2 {
                Duration::from_millis(40)
            } else {
                if mode == 3 {
                    Duration::ZERO
                } else {
                    BUDGET
                }
            })
        )
        .await
        .is_err());
        assert_eq!(peer.count(), if mode == 3 { 3 } else { 4 });
        assert_eq!(target.count(), 0);
        assert_eq!(row(&db).await, original);
        drop(owner);
        peer.close().await;
        target.close().await;
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_catalog_completion_terminal_and_done_do_not_succeed_before_real_eof() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let mut spec = fixture::Spec::json(200, json!(null));
    spec.headers
        .insert("content-type", "text/event-stream".parse().unwrap());
    spec.body = format!(
        "data: {}\n\ndata: [DONE]\n\n",
        json!({"choices":[{"index":0,
        "delta":{"content":"answer"},"finish_reason":"stop"}],"usage":answer()["usage"]})
    )
    .into_bytes();
    spec.eof_delay = Duration::from_millis(200);
    peer.push(spec);
    let decoded = send(&mut owner, &requests, &peer)
        .await
        .decode(Duration::from_millis(60))
        .await;
    assert_eq!(decoded.completion().err(), Some(BusinessError::Timeout));
    assert_eq!(
        decoded.accounting().sdk_credits().unwrap(),
        40,
        "terminal bytes were observed, but no actual EOF"
    );
    drop(decoded);
    assert_eq!(peer.count(), 3, "no automatic FINISH after timeout or Drop");
    assert_eq!(row(&db).await, original);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_completion_status_type_timeout_and_cancel_never_clear_original_or_finish()
{
    for mode in [0, 1, 2, 3] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        let mut spec = fixture::Spec::json(if mode == 0 { 403 } else { 200 }, answer());
        if mode == 1 {
            spec.headers
                .insert("content-type", "text/html".parse().unwrap());
        }
        if mode >= 2 {
            spec.body_delay = Duration::from_millis(150);
        }
        peer.push(spec);
        let response = send(&mut owner, &requests, &peer).await;
        if mode == 3 {
            let future = response.decode(BUDGET);
            tokio::pin!(future);
            tokio::select! {_=tokio::time::sleep(Duration::from_millis(30))=>(),_= &mut future=>panic!("slow decode completed early")}
        } else {
            let decoded = response
                .decode(if mode == 2 {
                    Duration::from_millis(40)
                } else {
                    BUDGET
                })
                .await;
            assert!(decoded.completion().is_err());
            drop(decoded);
        }
        assert_eq!(peer.count(), 3);
        assert_eq!(row(&db).await, original);
        drop(owner);
        drop(recovery(&db, &requests.recovery_requests()).await);
        peer.close().await;
        cleanup(&root, db).await;
    }
}
