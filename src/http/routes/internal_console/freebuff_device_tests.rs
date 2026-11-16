//! Exact management boundaries for explicit local device registration.
use super::*;
use crate::local_runtime::LocalRuntime;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
const REGISTER: &str = "/v1/internal/gateway/console/freebuff-device-registration";
#[path = "freebuff_device_state_tests.rs"]
mod state_tests;

#[tokio::test]
async fn freebuff_device_management_requires_grant_before_any_device_work() {
    let f = fixture();
    let response = send_console_json(&f.state, Method::POST, REGISTER, body(&f), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn freebuff_device_management_rejects_server_without_local_secret_owner() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let response = send_console_json(
        &f.state,
        Method::POST,
        REGISTER,
        body(&f),
        Some(&grant),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "freebuff_device_storage_unsupported"
    );
}

async fn local_fixture() -> (std::path::PathBuf, ConsoleStateFixture) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-management-{}",
        uuid::Uuid::new_v4()
    ));
    let local = LocalRuntime::open(&root).await.unwrap();
    let mut f = fixture();
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local);
    (root, f)
}
async fn call(
    f: &ConsoleStateFixture,
    peer: &wire::Peer,
    endpoint: &str,
    input: Value,
    grant: &str,
) -> axum::response::Response {
    crate::protocol::freebuff::catalog::with_test_origin(
        peer.origin.clone(),
        send_console_json(&f.state, Method::POST, endpoint, input, Some(grant), None),
    )
    .await
}
#[tokio::test]
async fn freebuff_device_management_persists_and_signs_exact_catalog_after_rebuild() {
    let (root, mut f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::device(200, None).await;
    assert_eq!(
        call(&f, &peer, ENDPOINT, body(&f), &grant).await.status(),
        StatusCode::OK
    );
    let local = f.state.local_runtime.as_ref().unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_device_keys")
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "read-only bootstrap must not generate a device");
    let response = call(&f, &peer, REGISTER, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let value = parse_json(response).await;
    assert_eq!(value["registration"], "stored");
    assert!(!value.to_string().contains("device-fixture"));
    let (public, sealed): (String, String) =
        sqlx::query_as("SELECT public_key,sealed FROM local_freebuff_device_keys")
            .fetch_one(&local.pool)
            .await
            .unwrap();
    assert!(!sealed.contains(&public));
    local.close().await;
    // A rebuilt runtime has a new management state/session, not an in-place Arc mutation after requests.
    f = fixture();
    Arc::get_mut(&mut f.state).unwrap().local_runtime =
        Some(LocalRuntime::open(&root).await.unwrap());
    let grant = grant_console_secret_access(&f.state).await;
    assert_eq!(
        call(&f, &peer, ENDPOINT, body(&f), &grant).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        call(&f, &peer, REGISTER, body(&f), &grant).await.status(),
        StatusCode::OK
    );
    let hits = peer.hits.lock().unwrap();
    assert_eq!(
        hits.len(),
        4,
        "ready registration is restored without another upstream request"
    );
    assert_eq!(hits[2].0, "POST");
    assert_eq!(hits[2].1, "/api/v1/freebuff/device-keys");
    let registration: Value = serde_json::from_slice(&hits[2].3).unwrap();
    assert_eq!(registration, json!({"publicKey":public,"client":"cli"}));
    assert_eq!(hits[2].2["authorization"], "Bearer synthetic-b");
    let headers = &hits[3].2;
    assert_eq!(headers["x-freebuff-device-key"], "device-fixture");
    let payload = format!(
        "freebuff-device-v1\nGET\n/api/v1/freebuff/models\n{}\n{}\n",
        headers["x-freebuff-device-ts"].to_str().unwrap(),
        hex::encode(ring::digest::digest(&ring::digest::SHA256, b""))
    );
    ring::signature::UnparsedPublicKey::new(
        &ring::signature::ED25519,
        URL_SAFE_NO_PAD.decode(&public).unwrap(),
    )
    .verify(
        payload.as_bytes(),
        &URL_SAFE_NO_PAD
            .decode(headers["x-freebuff-device-sig"].to_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(!headers.contains_key("x-freebuff-catalog-fetch"));
    drop(hits);
    peer.close().await;
    f.state.local_runtime.as_ref().unwrap().close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_management_unknown_post_requires_single_use_exact_attempt_ack() {
    let (root, f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::device(503, None).await;
    assert_eq!(
        call(&f, &peer, REGISTER, body(&f), &grant).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let response = call(&f, &peer, REGISTER, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    let value = parse_json(response).await;
    assert_eq!(value["registration"], "unconfirmed");
    let mut retry = body(&f);
    retry["retryUnconfirmedAttemptId"] = value["unconfirmedAttemptId"].clone();
    assert_eq!(
        call(&f, &peer, REGISTER, retry.clone(), &grant)
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        call(&f, &peer, REGISTER, retry, &grant).await.status(),
        StatusCode::CONFLICT
    );
    let hits = peer.hits.lock().unwrap();
    let posts = hits.iter().filter(|h| h.0 == "POST").collect::<Vec<_>>();
    assert_eq!(
        posts.len(),
        2,
        "no automatic or repeated stale acknowledgment replay"
    );
    assert_eq!(
        posts[0].3, posts[1].3,
        "explicit retry preserves the installation key"
    );
    drop(hits);
    peer.close().await;
    f.state.local_runtime.as_ref().unwrap().close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
#[tokio::test]
async fn freebuff_device_management_post_revision_change_keeps_unconfirmed_instead_of_ready() {
    let (root, f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let state = f.state.clone();
    let peer = wire::Peer::device(
        200,
        Some(Arc::new(move || {
            let mut document = state.route_config.snapshot().document().clone();
            document.providers[0].credentials[0].account_name = Some("changed".into());
            state.route_config.replace_document(document).unwrap();
        })),
    )
    .await;
    assert_eq!(
        call(&f, &peer, REGISTER, body(&f), &grant).await.status(),
        StatusCode::CONFLICT
    );
    let local = f.state.local_runtime.as_ref().unwrap();
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM local_freebuff_registrations WHERE sealed IS NULL",
    )
    .fetch_one(&local.pool)
    .await
    .unwrap();
    assert_eq!(pending, 1);
    peer.close().await;
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn freebuff_device_management_post_commit_fence_failure_preserves_known_receipt() {
    let (root, f) = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let state = f.state.clone();
    let peer = wire::Peer::device(200, None).await;
    let hook: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let mut document = state.route_config.snapshot().document().clone();
        document.providers[0].credentials[0].account_name = Some("after commit".into());
        state.route_config.replace_document(document).unwrap();
    });
    let response = crate::http::routes::internal_console::freebuff_device::AFTER_SAVE
        .scope(hook, call(&f, &peer, REGISTER, body(&f), &grant))
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let ready: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM local_freebuff_registrations WHERE sealed IS NOT NULL",
    )
    .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
    .await
    .unwrap();
    assert_eq!(
        ready, 1,
        "a response error cannot erase a verified durable registration"
    );
    assert_eq!(
        call(&f, &peer, REGISTER, body(&f), &grant).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        peer.hits.lock().unwrap().len(),
        2,
        "reconciliation must not repeat POST or bootstrap"
    );
    peer.close().await;
    f.state.local_runtime.as_ref().unwrap().close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
