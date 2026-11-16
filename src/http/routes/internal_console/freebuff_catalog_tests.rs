//! Production management router tests; synthetic accounts only, no live credentials.
use super::*;
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, send_console_json, ConsoleStateFixture};
use support::{parse_json, MANAGEMENT_TOKEN};
#[path = "freebuff_catalog_fixture.rs"]
mod wire;
const ENDPOINT: &str = "/v1/internal/gateway/console/freebuff-model-catalog";
#[path = "freebuff_device_tests.rs"]
mod device;
fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{
        "id":"pool", "preset":"freebuff", "base_url":"https://freebuff.com", "explicit_credentials_only":true,
        "credentials":[{"id":"a","api_key":"synthetic-a"},{"id":"b","api_key":"synthetic-b","enabled":false}]
    }]})).unwrap(), true)
}
fn body(f: &ConsoleStateFixture) -> Value {
    json!({"providerId":"pool","credentialId":"b","expectedRevision":f.state.route_config.snapshot().revision().id(),
        "businessOrigin":"https://codebuff.com","client":"cli"})
}
#[tokio::test]
async fn freebuff_catalog_management_requires_grant_before_network() {
    let f = fixture();
    let response = send_console_json(&f.state, Method::POST, ENDPOINT, body(&f), None, None).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn freebuff_catalog_management_rejects_stale_revision_before_network() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let mut input = body(&f);
    input["expectedRevision"] = json!("stale");
    let response =
        send_console_json(&f.state, Method::POST, ENDPOINT, input, Some(&grant), None).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}
#[tokio::test]
async fn freebuff_catalog_management_reads_exact_disabled_account_without_publishing_wire_material()
{
    for (origin, client) in [
        ("https://codebuff.com", "cli"),
        ("https://www.codebuff.com", "desktop"),
    ] {
        let f = fixture();
        let grant = grant_console_secret_access(&f.state).await;
        let before = serde_json::to_value(f.state.route_config.snapshot().document()).unwrap();
        let revision = f.state.route_config.snapshot().revision().id().to_owned();
        let peer = wire::Peer::start(wire::catalog(), 200).await;
        let mut input = body(&f);
        input["businessOrigin"] = json!(origin);
        input["client"] = json!(client);
        let response = crate::protocol::freebuff::catalog::with_test_origin(
            peer.origin.clone(),
            send_console_json(&f.state, Method::POST, ENDPOINT, input, Some(&grant), None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let value = parse_json(response).await;
        assert_eq!(value["providerId"], "pool");
        assert_eq!(value["credentialId"], "b");
        assert_eq!(value["businessOrigin"], origin);
        assert_eq!(value["client"], client);
        assert_eq!(value["revision"], revision);
        assert_eq!(value["models"].as_array().unwrap().len(), 2);
        assert_eq!(value["models"][1]["access"], "locked");
        for forbidden in ["fbm1.", "fetchId", "synthetic-b", "plansUrl", "fingerprint"] {
            assert!(!value.to_string().contains(forbidden), "{forbidden}");
        }
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "GET");
        assert_eq!(hits[0].1, "/api/v1/freebuff/models");
        assert_eq!(hits[0].2["authorization"], "Bearer synthetic-b");
        assert_eq!(hits[0].2["x-freebuff-client"], client);
        for header in [
            "x-freebuff-device-key",
            "x-freebuff-device-ts",
            "x-freebuff-device-sig",
        ] {
            assert!(!hits[0].2.contains_key(header));
        }
        drop(hits);
        assert_eq!(
            serde_json::to_value(f.state.route_config.snapshot().document()).unwrap(),
            before
        );
        assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
        peer.close().await;
    }
}
#[tokio::test]
async fn freebuff_catalog_management_rejects_foreign_targets_and_extra_material_without_fetch() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    for (field, value, status) in [
        ("providerId", "foreign", StatusCode::BAD_REQUEST),
        ("credentialId", "foreign", StatusCode::BAD_REQUEST),
        (
            "businessOrigin",
            "https://freebuff.com",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "businessOrigin",
            "http://127.0.0.1:1",
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("client", "web", StatusCode::UNPROCESSABLE_ENTITY),
        ("apiKey", "caller-key", StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let mut input = body(&f);
        input[field] = json!(value);
        let response =
            send_console_json(&f.state, Method::POST, ENDPOINT, input, Some(&grant), None).await;
        assert_eq!(response.status(), status, "{field}");
    }
    let response = send_console_json(
        &f.state,
        Method::POST,
        ENDPOINT,
        body(&f),
        Some(&grant),
        Some("https://foreign.invalid"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn freebuff_catalog_management_rejects_other_product_and_login_audience() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    for preset in ["freebuff", "xai-api"] {
        let mut document = f.state.route_config.snapshot().document().clone();
        document.providers[0].preset = Some(preset.into());
        document.providers[0].base_url = "https://other.invalid".into();
        f.state.route_config.replace_document(document).unwrap();
        let response = send_console_json(
            &f.state,
            Method::POST,
            ENDPOINT,
            body(&f),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
async fn headers(f: &ConsoleStateFixture) -> HeaderMap {
    let grant = grant_console_secret_access(&f.state).await;
    HeaderMap::from_iter([
        (
            "x-management-token".parse().unwrap(),
            MANAGEMENT_TOKEN.parse().unwrap(),
        ),
        ("x-secret-grant".parse().unwrap(), grant.parse().unwrap()),
    ])
}
#[tokio::test]
async fn freebuff_catalog_management_rechecks_grant_and_revision_after_successful_fetch() {
    for revoke in [true, false] {
        let f = fixture();
        let headers = headers(&f).await;
        let request = console_request_context(&headers, None);
        let state = f.state.clone();
        let response = read_with_fetch(
            f.state.clone(),
            None,
            None,
            headers,
            serde_json::from_value(body(&f)).unwrap(),
            |_, _, token| async move {
                assert_eq!(token, "synthetic-b");
                if revoke {
                    state
                        .console_auth
                        .logout(&request, MANAGEMENT_TOKEN)
                        .unwrap();
                } else {
                    let mut doc = state.route_config.snapshot().document().clone();
                    doc.providers[0].credentials[0].account_name = Some("changed".into());
                    state.route_config.replace_document(doc).unwrap();
                }
                Ok(vec![])
            },
        )
        .await
        .unwrap_err();
        assert_eq!(response.http_status, Some(if revoke { 403 } else { 409 }));
    }
}
#[tokio::test]
async fn freebuff_catalog_management_upstream_failure_never_exposes_token_or_body() {
    let f = fixture();
    let grant = grant_console_secret_access(&f.state).await;
    let peer = wire::Peer::start(
        json!({"error":"secret synthetic-b upstream-diagnostic"}),
        401,
    )
    .await;
    let response = crate::protocol::freebuff::catalog::with_test_origin(
        peer.origin.clone(),
        send_console_json(
            &f.state,
            Method::POST,
            ENDPOINT,
            body(&f),
            Some(&grant),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let value = parse_json(response).await.to_string();
    assert!(!value.contains("synthetic-b") && !value.contains("upstream-diagnostic"));
    peer.close().await;
}
#[tokio::test]
async fn freebuff_catalog_management_cancellation_releases_discovery_slot() {
    let f = fixture();
    let headers = headers(&f).await;
    let slots = &crate::provider_discovery::background::SLOTS;
    let before = slots.available_permits();
    let held = slots.try_acquire_many(before as u32).unwrap();
    let result = read_with_fetch(
        f.state.clone(),
        None,
        None,
        headers.clone(),
        serde_json::from_value(body(&f)).unwrap(),
        |_, _, _| async { panic!("busy must not fetch") },
    )
    .await
    .unwrap_err();
    assert_eq!(result.http_status, Some(409));
    drop(held);
    let future = read_with_fetch(
        f.state.clone(),
        None,
        None,
        headers,
        serde_json::from_value(body(&f)).unwrap(),
        |_, _, _| async { std::future::pending().await },
    );
    assert!(tokio::time::timeout(Duration::from_millis(10), future)
        .await
        .is_err());
    assert_eq!(slots.available_permits(), before);
}
