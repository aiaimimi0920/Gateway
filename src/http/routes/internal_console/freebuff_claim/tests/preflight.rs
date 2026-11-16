use super::*;
#[tokio::test]
async fn freebuff_claim_management_requires_normal_grant_revision_and_local_owner() {
    let (root, f) = fixture().await;
    assert_eq!(
        send_console_json(&f.state, Method::POST, ENDPOINT, body(&f), None, None)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let grant = grant_console_secret_access(&f.state).await;
    let mut stale = body(&f);
    stale["expectedRevision"] = json!("stale");
    assert_eq!(
        send_console_json(&f.state, Method::POST, ENDPOINT, stale, Some(&grant), None)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        send_console_json(
            &f.state,
            Method::POST,
            ENDPOINT,
            body(&f),
            Some(&grant),
            Some("https://foreign.invalid")
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(counts(&f).await, (0, 0));
    cleanup(&root, f).await;
    let f = state_fixture();
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
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "freebuff_session_claim_storage_unsupported"
    );
}
#[tokio::test]
async fn freebuff_claim_management_local_read_never_registers_fetches_or_adopts_foreign_sessions() {
    let (root, f) = fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer =
        wire::Peer::session(json!({"status":"active","instanceId":"foreign"}), 200, None).await;
    let response = call(&f, &peer, ENDPOINT, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let value = parse_json(response).await;
    assert!(value["claim"].is_null());
    assert_eq!(value["history"], json!([]));
    assert!(peer.hits.lock().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM local_freebuff_device_keys")
            .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
            .await
            .unwrap(),
        0
    );
    let response = call(
        &f,
        &peer,
        &format!("{ENDPOINT}/end"),
        end_body(&f, &uuid::Uuid::new_v4().to_string()),
        &grant,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(peer.hits.lock().unwrap().is_empty());
    peer.close().await;
    cleanup(&root, f).await;
}
#[tokio::test]
async fn freebuff_claim_management_rejects_supplied_authority_secrets_and_bad_attempts() {
    let (root, f) = fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    for (key, value) in [
        ("businessOrigin", json!("https://foreign.invalid")),
        ("apiKey", json!("caller-secret")),
        ("modelKey", json!("foreign")),
        ("instanceId", json!("cli:foreign")),
    ] {
        let mut input = body(&f);
        input[key] = value;
        let response =
            send_console_json(&f.state, Method::POST, ENDPOINT, input, Some(&grant), None).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    for id in ["foreign", "00000000-0000-1000-8000-000000000000"] {
        let response = send_console_json(
            &f.state,
            Method::POST,
            &format!("{ENDPOINT}/end"),
            end_body(&f, id),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    cleanup(&root, f).await;
}
#[tokio::test]
async fn freebuff_claim_management_authority_change_does_not_turn_original_claim_into_absence() {
    let (root, f) = fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::session(json!({"status":"ended","freebucksRefund":0}), 200, None).await;
    let (id, _) = seed(&f, &peer, &grant).await;
    let mut document = f.state.route_config.snapshot().document().clone();
    document.providers[0].credentials[1].api_key = Some("different-authority".into());
    f.state.route_config.replace_document(document).unwrap();
    for (path, input) in [
        (ENDPOINT.to_owned(), body(&f)),
        (format!("{ENDPOINT}/end"), end_body(&f, &id)),
    ] {
        let response = call(&f, &peer, &path, input, &grant).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            parse_json(response).await["error"]["code"],
            "freebuff_session_claim_unavailable"
        );
    }
    assert_eq!(counts(&f).await, (1, 0));
    assert!(peer.hits.lock().unwrap().is_empty());
    peer.close().await;
    cleanup(&root, f).await;
}
#[tokio::test]
async fn freebuff_claim_management_read_does_not_take_execution_guard_and_end_cannot_steal_it() {
    let (root, f) = fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::session(json!({"status":"ended","freebucksRefund":0}), 200, None).await;
    let (id, _) = seed(&f, &peer, &grant).await;
    let local = f.state.local_runtime.as_ref().unwrap();
    let mut session =
        CatalogSession::new(BusinessOrigin::Codebuff, "synthetic-b", ClientSurface::Cli).unwrap();
    let device = local
        .freebuff_registration("pool", "b", &session)
        .await
        .unwrap()
        .unwrap();
    crate::protocol::freebuff::catalog::with_test_origin(
        peer.origin.clone(),
        session.refresh(Some(&device), std::time::Duration::from_secs(3)),
    )
    .await
    .unwrap();
    let requests = session.recovery_requests(&device).unwrap();
    let owner = local
        .recover_freebuff_admission("pool", "b", &requests)
        .await
        .unwrap()
        .unwrap();
    peer.hits.lock().unwrap().clear();
    assert_eq!(
        parse_json(call(&f, &peer, ENDPOINT, body(&f), &grant).await).await["claim"]["attemptId"],
        id
    );
    assert!(peer.hits.lock().unwrap().is_empty());
    let response = call(
        &f,
        &peer,
        &format!("{ENDPOINT}/end"),
        end_body(&f, &id),
        &grant,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(counts(&f).await, (1, 0));
    assert!(peer.hits.lock().unwrap().iter().all(|hit| hit.0 == "GET"));
    drop(owner);
    peer.close().await;
    cleanup(&root, f).await;
}
