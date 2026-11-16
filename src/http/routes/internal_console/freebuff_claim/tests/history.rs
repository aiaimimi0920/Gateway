use super::*;
#[tokio::test]
async fn freebuff_claim_management_history_is_bounded_paged_and_old_end_never_touches_new_claim() {
    let (root, f) = fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::session(json!({"status":"ended","freebucksRefund":0}), 200, None).await;
    let mut ids = Vec::new();
    for _ in 0..21 {
        let (id, _) = seed(&f, &peer, &grant).await;
        let response = call(
            &f,
            &peer,
            &format!("{ENDPOINT}/end"),
            end_body(&f, &id),
            &grant,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        ids.push(id);
        peer.hits.lock().unwrap().clear();
    }
    let (active, _) = seed(&f, &peer, &grant).await;
    let first = parse_json(call(&f, &peer, ENDPOINT, body(&f), &grant).await).await;
    assert_eq!(first["history"].as_array().unwrap().len(), 20);
    assert_eq!(first["history"][0]["attemptId"], ids[20]);
    assert_eq!(first["claim"]["attemptId"], active);
    let mut next = body(&f);
    next["cursor"] = first["nextCursor"].clone();
    let second = parse_json(call(&f, &peer, ENDPOINT, next, &grant).await).await;
    assert_eq!(second["history"].as_array().unwrap().len(), 1);
    assert_eq!(second["history"][0]["attemptId"], ids[0]);
    assert!(second["nextCursor"].is_null());
    let old = parse_json(
        call(
            &f,
            &peer,
            &format!("{ENDPOINT}/end"),
            end_body(&f, &ids[0]),
            &grant,
        )
        .await,
    )
    .await;
    assert_eq!(old["fromHistory"], true);
    assert_eq!(counts(&f).await, (1, 21));
    assert!(peer.hits.lock().unwrap().is_empty());
    let mut foreign = body(&f);
    foreign["cursor"] = json!(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        call(&f, &peer, ENDPOINT, foreign, &grant).await.status(),
        StatusCode::BAD_REQUEST
    );
    peer.close().await;
    cleanup(&root, f).await;
}
#[tokio::test]
async fn freebuff_claim_management_offline_journal_survives_reopen_and_sealed_tampering_is_not_absence(
) {
    let (root, f) = fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::session(json!({"status":"ended","freebucksRefund":0}), 200, None).await;
    let (id, _) = seed(&f, &peer, &grant).await;
    peer.close().await;
    let document = f.state.route_config.snapshot().document().clone();
    f.state.local_runtime.as_ref().unwrap().close().await;
    drop(f);
    let local = LocalRuntime::open(&root).await.unwrap();
    let mut f = ConsoleStateFixture::new(document, true);
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local);
    let grant = grant_console_secret_access(&f.state).await;
    let response = send_console_json(
        &f.state,
        Method::POST,
        ENDPOINT,
        body(&f),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(parse_json(response).await["claim"]["attemptId"], id);
    sqlx::query("UPDATE local_freebuff_session_claims SET sealed='corrupt'")
        .execute(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    let response = send_console_json(
        &f.state,
        Method::POST,
        ENDPOINT,
        body(&f),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(counts(&f).await, (1, 0));
    cleanup(&root, f).await;
}
