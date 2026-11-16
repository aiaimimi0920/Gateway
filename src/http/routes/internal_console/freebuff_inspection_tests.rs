//! Normal management route; only synthetic stored accounts and loopback upstreams.
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
#[path = "freebuff_catalog_fixture.rs"]
mod wire;
use std::sync::Arc;
const ENDPOINT: &str = "/v1/internal/gateway/console/freebuff-session-inspection";
fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{
        "id":"pool", "preset":"freebuff", "base_url":"https://freebuff.com", "explicit_credentials_only":true,
        "credentials":[{"id":"a","api_key":"synthetic-a"},{"id":"b","api_key":"synthetic-b","enabled":false,
            "supported_models":["open"],"extra_body":{"freebuffBusiness":{"origin":"https://codebuff.com","client":"cli"}}}]
    }]})).unwrap(), true)
}
fn body(f: &ConsoleStateFixture) -> Value {
    json!({"providerId":"pool","credentialId":"b","expectedRevision":f.state.route_config.snapshot().revision().id(),"modelKey":"open"})
}
#[tokio::test]
async fn freebuff_inspection_requires_grant_before_network() {
    let f = fixture();
    let response = send_console_json(&f.state, Method::POST, ENDPOINT, body(&f), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn freebuff_inspection_rejects_stale_revision_before_network() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let mut input = body(&f);
    input["expectedRevision"] = json!("stale");
    let response =
        send_console_json(&f.state, Method::POST, ENDPOINT, input, Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
#[tokio::test]
async fn freebuff_inspection_requires_local_registered_device_without_bootstrap_or_purchase() {
    let f = fixture();
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
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "freebuff_device_storage_unsupported"
    );
}
async fn local_fixture() -> ConsoleStateFixture {
    let root = std::env::temp_dir().join(format!(
        "gateway-freebuff-inspection-{}",
        uuid::Uuid::new_v4()
    ));
    let local = crate::local_runtime::LocalRuntime::open(&root)
        .await
        .unwrap();
    let mut f = fixture();
    Arc::get_mut(&mut f.state).unwrap().local_runtime = Some(local);
    f
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
async fn register(f: &ConsoleStateFixture, peer: &wire::Peer, grant: &str) {
    let mut input = body(f);
    input.as_object_mut().unwrap().remove("modelKey");
    input["businessOrigin"] = json!("https://codebuff.com");
    input["client"] = json!("cli");
    let response = call(
        f,
        peer,
        "/v1/internal/gateway/console/freebuff-device-registration",
        input,
        grant,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    peer.hits.lock().unwrap().clear();
}
#[tokio::test]
async fn freebuff_inspection_exact_account_signed_gets_and_typed_snapshot_never_claim_or_purchase()
{
    let f = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::session(json!({"status":"active","accessTier":"limited","instanceId":"foreign-instance",
        "model":"other","admittedAt":"2026-11-06T00:00:00Z","expiresAt":"2026-11-06T01:00:00Z","remainingMs":1000,
        "freebucks":{"balance":7,"daily":{"remaining":5},"wallet":{"balance":2},"prices":{"open":0}},
        "upstreamSecret":"never-publish"}),200,None).await;
    register(&f, &peer, &grant).await;
    let before = serde_json::to_value(f.state.route_config.snapshot().document()).unwrap();
    let response = call(&f, &peer, ENDPOINT, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let value = parse_json(response).await;
    assert_eq!(value["credentialId"], "b");
    assert_eq!(value["modelKey"], "open");
    assert_eq!(value["businessOrigin"], "https://codebuff.com");
    assert_eq!(value["session"]["modelKey"], "other");
    assert_eq!(value["meter"]["sessionPrice"], 0.0);
    for forbidden in [
        "synthetic-b",
        "foreign-instance",
        "never-publish",
        "fetchId",
        "fbm1.",
        "device-fixture",
        "instanceId",
    ] {
        assert!(!value.to_string().contains(forbidden), "{forbidden}");
    }
    let hits = peer.hits.lock().unwrap();
    assert_eq!(hits.len(), 2);
    for (index, path) in ["/api/v1/freebuff/models", "/api/v1/freebuff/session"]
        .into_iter()
        .enumerate()
    {
        let (method, uri, headers, body) = &hits[index];
        assert_eq!(method, "GET");
        assert_eq!(uri, path);
        assert_eq!(headers["authorization"], "Bearer synthetic-b");
        assert_eq!(headers["x-freebuff-device-key"], "device-fixture");
        assert!(
            !headers.contains_key("x-freebuff-instance-id")
                && !headers.contains_key("x-freebuff-desktop-attempt-id")
        );
        assert!(body.is_empty());
    }
    drop(hits);
    assert_eq!(
        serde_json::to_value(f.state.route_config.snapshot().document()).unwrap(),
        before
    );
    f.state.local_runtime.as_ref().unwrap().close().await;
    peer.close().await;
}
#[tokio::test]
async fn freebuff_inspection_unregistered_does_not_create_key_or_send_unsigned_request() {
    let f = local_fixture().await;
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::session(json!({"status":"none"}), 200, None).await;
    let response = call(&f, &peer, ENDPOINT, body(&f), &grant).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        parse_json(response).await["error"]["code"],
        "freebuff_device_registration_required"
    );
    assert!(peer.hits.lock().unwrap().is_empty());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM local_freebuff_device_keys")
        .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    f.state.local_runtime.as_ref().unwrap().close().await;
    peer.close().await;
}
#[tokio::test]
async fn freebuff_inspection_rejects_unstored_models_targets_origins_and_supplied_secrets() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    for (field, value, status) in [
        ("modelKey", "foreign", 400),
        ("credentialId", "a", 400),
        ("providerId", "foreign", 400),
        ("businessOrigin", "https://www.codebuff.com", 422),
        ("apiKey", "caller-secret", 422),
    ] {
        let mut input = body(&f);
        input[field] = json!(value);
        assert_eq!(
            send_console_json(&f.state, Method::POST, ENDPOINT, input, Some(&grant), None)
                .await
                .status()
                .as_u16(),
            status
        );
    }
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
}
#[tokio::test]
async fn freebuff_inspection_post_fetch_grant_and_revision_fences_block_stale_display() {
    for revoke in [true, false] {
        let f = local_fixture().await;
        let grant = grant_console_secret_access(&f.state).await;
        let state = f.state.clone();
        let peer = wire::Peer::session(
            json!({"status":"none"}),
            200,
            Some(Arc::new(move || {
                if revoke {
                    let request =
                        super::console_request_context(&axum::http::HeaderMap::new(), None);
                    state
                        .console_auth
                        .logout(&request, support::MANAGEMENT_TOKEN)
                        .unwrap();
                    return;
                }
                let mut document = state.route_config.snapshot().document().clone();
                document.providers[0].credentials[0].account_name = Some("changed".into());
                state.route_config.replace_document(document).unwrap();
            })),
        )
        .await;
        register(&f, &peer, &grant).await;
        let response = call(&f, &peer, ENDPOINT, body(&f), &grant).await;
        assert_eq!(
            response.status(),
            if revoke {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::CONFLICT
            }
        );
        assert_eq!(peer.hits.lock().unwrap().len(), 2);
        f.state.local_runtime.as_ref().unwrap().close().await;
        peer.close().await;
    }
}
#[tokio::test]
async fn freebuff_inspection_non_success_or_malformed_reply_is_safe_and_not_retried() {
    for (body, status) in [
        (
            json!({"status":"none","secret":"synthetic-b diagnostic"}),
            401,
        ),
        (
            json!({"status":"active","secret":"synthetic-b diagnostic"}),
            200,
        ),
    ] {
        let f = local_fixture().await;
        let grant = grant_console_secret_access(&f.state).await;
        let peer = wire::Peer::session(body, status, None).await;
        register(&f, &peer, &grant).await;
        let response = call(
            &f,
            &peer,
            ENDPOINT,
            super::freebuff_inspection_tests::body(&f),
            &grant,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let value = parse_json(response).await.to_string();
        assert!(!value.contains("synthetic-b") && !value.contains("diagnostic"));
        assert_eq!(peer.hits.lock().unwrap().len(), 2);
        f.state.local_runtime.as_ref().unwrap().close().await;
        peer.close().await;
    }
}
