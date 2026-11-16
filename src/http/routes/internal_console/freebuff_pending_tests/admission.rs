//! Real router -> live internal waiter -> original sealed SQLite admission; not public generation.
use super::*;
use crate::local_runtime::LocalRuntime;
async fn local() -> (std::path::PathBuf, LocalRuntime) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-consent-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    (root, db)
}
async fn close(root: &std::path::Path, db: LocalRuntime) {
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(root).await;
}
async fn rows(db: &LocalRuntime) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM local_freebuff_session_claims")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}
#[tokio::test]
async fn freebuff_pending_management_only_delivers_then_original_request_buys_once() {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let request = TestRequest::new(&f, &peer).await;
    let (root, db) = local().await;
    let grant = grant_console_secret_access(&f.state).await;
    let waiting = request
        .begin(&f, &peer, Duration::from_secs(10))
        .await
        .unwrap();
    let view = list(&f, &grant).await[0].clone();
    let hits = peer.hits();
    assert_eq!(rows(&db).await, 0);
    let reply = decide(&f, &grant, &view, "approve").await;
    assert_eq!(reply.status(), StatusCode::OK);
    assert_eq!(parse_json(reply).await, json!({"delivered":true}));
    assert_eq!(
        peer.hits(),
        hits,
        "operator decision cannot purchase while the caller is unpolled"
    );
    assert_eq!(rows(&db).await, 0);
    let approved = waiting.wait().await.unwrap();
    let current = request.binding();
    let requests = request
        .session
        .business_requests(&request.device, "open")
        .unwrap();
    let active = crate::protocol::freebuff::catalog::with_test_origin(
        peer.origin.clone(),
        approved.admit(&current, &db, &requests, Duration::from_secs(3)),
    )
    .await
    .unwrap();
    assert_eq!(active.active().model_key(), "open");
    assert_eq!(rows(&db).await, 1);
    assert_eq!(peer.purchases(), 1);
    assert_eq!(
        decide(&f, &grant, &view, "approve").await.status(),
        StatusCode::CONFLICT
    );
    let sealed: String = sqlx::query_scalar("SELECT sealed FROM local_freebuff_session_claims")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(!sealed.contains(active.active().claim().attempt_id()));
    assert!(!sealed.contains("synthetic-b"));
    drop(active);
    assert!(list(&f, &grant).await.as_array().unwrap().is_empty());
    peer.close().await;
    close(&root, db).await;
}
#[tokio::test]
async fn freebuff_pending_approval_drop_changed_price_or_late_grant_never_creates_an_obligation() {
    for mode in ["drop", "price", "grant", "drain"] {
        let f = fixture();
        let peer = wire::Peer::new().await;
        let request = TestRequest::new(&f, &peer).await;
        let (root, db) = local().await;
        let grant = grant_console_secret_access(&f.state).await;
        let waiting = request
            .begin(&f, &peer, Duration::from_secs(3))
            .await
            .unwrap();
        let view = list(&f, &grant).await[0].clone();
        assert_eq!(
            decide(&f, &grant, &view, "approve").await.status(),
            StatusCode::OK
        );
        let approved = waiting.wait().await.unwrap();
        match mode {
            "drop" => drop(approved),
            _ => {
                match mode {
                    "price" => peer.price(11),
                    "grant" => f
                        .state
                        .console_auth
                        .logout(
                            &crate::console::ConsoleRequestContext::loopback(),
                            support::MANAGEMENT_TOKEN,
                        )
                        .unwrap(),
                    "drain" => {
                        f.state.lifecycle.begin_drain("synthetic drain");
                    }
                    _ => unreachable!(),
                }
                let current = request.binding();
                let requests = request
                    .session
                    .business_requests(&request.device, "open")
                    .unwrap();
                let error = crate::protocol::freebuff::catalog::with_test_origin(
                    peer.origin.clone(),
                    approved.admit(&current, &db, &requests, Duration::from_secs(3)),
                )
                .await
                .err()
                .unwrap();
                if mode == "price" {
                    assert_eq!(
                        error.code.as_deref(),
                        Some("freebuff_purchase_quote_changed")
                    );
                }
            }
        }
        assert_eq!(rows(&db).await, 0, "{mode}");
        assert_eq!(peer.purchases(), 0, "{mode}");
        peer.close().await;
        close(&root, db).await;
    }
}
#[tokio::test]
async fn freebuff_pending_receipt_cannot_transfer_to_a_different_live_request_caller_or_bytes() {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let mut request = TestRequest::new(&f, &peer).await;
    let (root, db) = local().await;
    let grant = grant_console_secret_access(&f.state).await;
    let original_id = request.ctx.req_id;
    let original_session = request.ctx.session.clone();
    let original_body = request.body.clone();
    let original_candidate = request.candidate.clone();
    for mode in [
        "request",
        "caller",
        "tenant",
        "source",
        "bytes",
        "candidate",
        "model",
        "stream",
    ] {
        let waiting = request
            .begin(&f, &peer, Duration::from_secs(3))
            .await
            .unwrap();
        let view = list(&f, &grant).await[0].clone();
        assert_eq!(
            decide(&f, &grant, &view, "approve").await.status(),
            StatusCode::OK
        );
        let approved = waiting.wait().await.unwrap();
        match mode {
            "request" => request.ctx.req_id = uuid::Uuid::new_v4(),
            "caller" => {
                request.ctx.session.as_mut().unwrap().access_key_id = Some("other-key".into())
            }
            "tenant" => request.ctx.session.as_mut().unwrap().tenant_id = "other-tenant".into(),
            "source" => request.ctx.source_access_key_id = Some("other-source".into()),
            "bytes" => request.body.push(b' '),
            "candidate" => {
                request.candidate = request
                    .snapshot
                    .resolve_candidates(Some("open"))
                    .into_iter()
                    .find(|candidate| candidate.payload.credential_id.as_deref() == Some("a"))
                    .unwrap()
            }
            "stream" => request.ctx.canonical_req.stream = true,
            "model" => (),
            _ => unreachable!(),
        }
        let current = RequestBinding::capture(
            &request.ctx,
            &request.snapshot,
            &request.candidate,
            if mode == "model" { "other" } else { "open" },
            &request.body,
        )
        .unwrap();
        let requests = request
            .session
            .business_requests(&request.device, "open")
            .unwrap();
        let hits = peer.hits();
        let error = approved
            .admit(&current, &db, &requests, Duration::from_secs(3))
            .await
            .err()
            .unwrap();
        assert_eq!(
            error.code.as_deref(),
            Some("freebuff_purchase_request_binding_mismatch"),
            "{mode}"
        );
        assert_eq!(
            peer.hits(),
            hits,
            "{mode}: mismatch must precede a signed inspect"
        );
        request.ctx.req_id = original_id;
        request.ctx.session = original_session.clone();
        request.body = original_body.clone();
        request.candidate = original_candidate.clone();
        request.ctx.source_access_key_id = None;
        request.ctx.canonical_req.stream = false;
    }
    assert_eq!(rows(&db).await, 0);
    assert_eq!(peer.purchases(), 0);
    peer.close().await;
    close(&root, db).await;
}
