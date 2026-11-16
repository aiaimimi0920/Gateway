//! State reconciliation is local and must remain available when upstream discovery is unavailable.
use super::*;
const STATUS: &str = "/v1/internal/gateway/console/freebuff-device-registration/status";

#[tokio::test]
async fn freebuff_device_local_status_requires_grant_and_supported_owner() {
    let f = fixture();
    let response = send_console_json(&f.state, Method::POST, STATUS, body(&f), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&f.state).await;
    let response =
        send_console_json(&f.state, Method::POST, STATUS, body(&f), Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "freebuff_device_storage_unsupported"
    );
}

#[tokio::test]
async fn freebuff_device_local_status_and_unknown_reconcile_do_not_contact_upstream() {
    let (root, f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::device(503, None).await;
    let response = call(&f, &peer, STATUS, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(parse_json(response).await["registration"], "absent");
    let local = f.state.local_runtime.as_ref().unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_device_keys")
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert_eq!(
        count, 0,
        "state inspection must not create an installation key"
    );
    assert!(peer.hits.lock().unwrap().is_empty());
    assert_eq!(
        call(&f, &peer, REGISTER, body(&f), &grant).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let response = call(&f, &peer, STATUS, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    let unknown = parse_json(response).await;
    assert_eq!(unknown["registration"], "unconfirmed");
    assert!(uuid::Uuid::parse_str(unknown["unconfirmedAttemptId"].as_str().unwrap()).is_ok());
    let offline = wire::Peer::start(json!({}), 503).await;
    let response = call(&f, &offline, REGISTER, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(parse_json(response).await, unknown);
    assert!(
        offline.hits.lock().unwrap().is_empty(),
        "ordinary pending reconciliation must precede GET"
    );
    assert_eq!(peer.hits.lock().unwrap().len(), 2);
    offline.close().await;
    peer.close().await;
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn freebuff_device_local_status_rejects_foreign_revision_origin_and_product_without_keygen() {
    let (root, f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::start(json!({}), 503).await;
    for (field, value, status) in [
        ("expectedRevision", "stale", StatusCode::CONFLICT),
        ("providerId", "foreign", StatusCode::BAD_REQUEST),
        ("credentialId", "foreign", StatusCode::BAD_REQUEST),
        (
            "businessOrigin",
            "http://127.0.0.1:1",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("client", "web", StatusCode::UNPROCESSABLE_ENTITY),
        ("apiKey", "caller", StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let mut input = body(&f);
        input[field] = json!(value);
        assert_eq!(
            call(&f, &peer, STATUS, input, &grant).await.status(),
            status,
            "{field}"
        );
    }
    let response = send_console_json(
        &f.state,
        Method::POST,
        STATUS,
        body(&f),
        Some(&grant),
        Some("https://foreign.invalid"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let local = f.state.local_runtime.as_ref().unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_device_keys")
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(peer.hits.lock().unwrap().is_empty());
    peer.close().await;
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn freebuff_device_management_explicit_replacement_preserves_receipt_until_confirmed() {
    let (root, f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let good = wire::Peer::device(200, None).await;
    let response = call(&f, &good, REGISTER, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    let stored = parse_json(response).await;
    assert!(uuid::Uuid::parse_str(stored["recordId"].as_str().unwrap()).is_ok());
    let local = f.state.local_runtime.as_ref().unwrap();
    let original: (String, String, String) =
        sqlx::query_as("SELECT public_key,attempt_id,sealed FROM local_freebuff_registrations")
            .fetch_one(&local.pool)
            .await
            .unwrap();
    let offline = wire::Peer::start(json!({}), 503).await;
    let response = call(&f, &offline, STATUS, body(&f), &grant).await;
    assert_eq!(parse_json(response).await, stored);
    assert!(offline.hits.lock().unwrap().is_empty());
    let mut replace = body(&f);
    replace["replaceStoredRecordId"] = stored["recordId"].clone();
    assert_eq!(
        call(&f, &offline, REGISTER, replace.clone(), &grant)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        parse_json(call(&f, &offline, STATUS, body(&f), &grant).await).await,
        stored,
        "failed GET must not consume confirmation"
    );
    let rejected = wire::Peer::device(503, None).await;
    assert_eq!(
        call(&f, &rejected, REGISTER, replace.clone(), &grant)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let unchanged: (String, String, String) =
        sqlx::query_as("SELECT public_key,attempt_id,sealed FROM local_freebuff_registrations")
            .fetch_one(&local.pool)
            .await
            .unwrap();
    assert_eq!(
        unchanged, original,
        "unknown replacement retains exact previous sealed receipt"
    );
    let unknown = parse_json(call(&f, &offline, STATUS, body(&f), &grant).await).await;
    assert_eq!(unknown["registration"], "unconfirmed");
    assert_ne!(unknown["unconfirmedAttemptId"], stored["recordId"]);
    let before = rejected.hits.lock().unwrap().len();
    assert_eq!(
        call(&f, &rejected, REGISTER, replace, &grant)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        rejected.hits.lock().unwrap().len(),
        before,
        "stale replacement confirmation rejected before GET"
    );
    assert_eq!(
        parse_json(call(&f, &offline, REGISTER, body(&f), &grant).await).await,
        unknown
    );
    let response = call(&f, &good, ENDPOINT, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "freebuff_device_registration_unconfirmed"
    );
    let mut retry = body(&f);
    retry["retryUnconfirmedAttemptId"] = unknown["unconfirmedAttemptId"].clone();
    let response = call(&f, &good, REGISTER, retry.clone(), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    let next = parse_json(response).await;
    assert_eq!(next["registration"], "stored");
    assert_ne!(next["recordId"], stored["recordId"]);
    let count = good.hits.lock().unwrap().len();
    assert_eq!(
        call(&f, &good, REGISTER, retry, &grant).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(good.hits.lock().unwrap().len(), count);
    assert_eq!(
        call(&f, &good, ENDPOINT, body(&f), &grant).await.status(),
        StatusCode::OK
    );
    good.close().await;
    offline.close().await;
    rejected.close().await;
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn freebuff_device_management_requires_one_precise_confirmation_before_network() {
    let (root, f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::start(json!({}), 503).await;
    for fields in [
        json!({"retryUnconfirmedAttemptId":"bad"}),
        json!({"replaceStoredRecordId":"bad"}),
        json!({"retryUnconfirmedAttemptId":uuid::Uuid::new_v4().to_string(), "replaceStoredRecordId":uuid::Uuid::new_v4().to_string()}),
    ] {
        let mut input = body(&f);
        input
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        assert_eq!(
            call(&f, &peer, REGISTER, input, &grant).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    let mut input = body(&f);
    input["replaceStoredRecordId"] = json!(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        call(&f, &peer, REGISTER, input, &grant).await.status(),
        StatusCode::CONFLICT
    );
    assert!(peer.hits.lock().unwrap().is_empty());
    peer.close().await;
    f.state.local_runtime.as_ref().unwrap().close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
