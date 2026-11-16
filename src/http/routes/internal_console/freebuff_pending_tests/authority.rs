use super::*;
use crate::console::ConsoleRequestContext;
#[tokio::test]
async fn freebuff_pending_management_rejects_missing_grant_origin_stale_quote_and_injected_authority(
) {
    let f = fixture();
    let peer = wire::Peer::new().await;
    let request = TestRequest::new(&f, &peer).await;
    let waiting = request
        .begin(&f, &peer, Duration::from_secs(3))
        .await
        .unwrap();
    assert_eq!(
        send_console_json(&f.state, Method::POST, PATH, body(&f), None, None)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let grant = grant_console_secret_access(&f.state).await;
    assert_eq!(
        send_console_json(
            &f.state,
            Method::POST,
            PATH,
            body(&f),
            Some(&grant),
            Some("https://foreign.invalid")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let mut input = body(&f);
    input["expectedRevision"] = json!("stale");
    assert_eq!(
        send_console_json(&f.state, Method::POST, PATH, input, Some(&grant), None)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let view = list(&f, &grant).await[0].clone();
    for field in [
        "apiKey",
        "confirmed",
        "callerId",
        "requestId",
        "businessOrigin",
    ] {
        let mut input = body(&f);
        input[field] = json!("forged");
        let reply =
            send_console_json(&f.state, Method::POST, PATH, input, Some(&grant), None).await;
        assert_eq!(reply.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(reply.headers()["cache-control"], "no-store");
    }
    let mut wrong = view.clone();
    wrong["quoteId"] = json!(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        decide(&f, &grant, &wrong, "approve").await.status(),
        StatusCode::CONFLICT
    );
    let mut foreign = body(&f);
    foreign["credentialId"] = json!("a");
    foreign["pendingId"] = view["pendingId"].clone();
    foreign["quoteId"] = view["quoteId"].clone();
    foreign["decision"] = json!("approve");
    assert_eq!(
        send_console_json(
            &f.state,
            Method::POST,
            &format!("{PATH}/decision"),
            foreign,
            Some(&grant),
            None
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(list(&f, &grant).await[0], view);
    drop(waiting);
    assert_eq!(peer.purchases(), 0);
    peer.close().await;
}
#[tokio::test]
async fn freebuff_pending_delivered_confirmation_reauthenticates_management_key_before_consumption()
{
    let f = fixture();
    let peer = wire::Peer::new().await;
    let request = TestRequest::new(&f, &peer).await;
    let auth = &f.state.console_auth;
    let context = ConsoleRequestContext::loopback();
    auth.add_management_key(
        &context,
        support::MANAGEMENT_TOKEN,
        "retained",
        "second-management-test-token",
    )
    .unwrap();
    let grant = grant_console_secret_access(&f.state).await;
    let waiting = request
        .begin(&f, &peer, Duration::from_secs(10))
        .await
        .unwrap();
    let view = list(&f, &grant).await[0].clone();
    assert_eq!(
        decide(&f, &grant, &view, "approve").await.status(),
        StatusCode::OK
    );
    auth.revoke_management_key(&context, "second-management-test-token", "initial")
        .unwrap();
    assert_eq!(
        waiting.wait().await.err().unwrap().code.as_deref(),
        Some("console_management_token_invalid")
    );
    assert_eq!(peer.purchases(), 0);
    peer.close().await;
}
#[tokio::test]
async fn freebuff_pending_delivered_confirmation_does_not_survive_grant_or_revision_change() {
    for revision in [false, true] {
        let f = fixture();
        let peer = wire::Peer::new().await;
        let request = TestRequest::new(&f, &peer).await;
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
        if revision {
            let mut document = f.state.route_config.snapshot().document().clone();
            document.providers[0].label = Some("changed".into());
            f.state.route_config.replace_document(document).unwrap();
        } else {
            f.state
                .console_auth
                .logout(
                    &ConsoleRequestContext::loopback(),
                    support::MANAGEMENT_TOKEN,
                )
                .unwrap();
        }
        assert!(waiting.wait().await.is_err());
        assert_eq!(peer.purchases(), 0);
        peer.close().await;
    }
}
