use super::{
    fixture::*,
    tests::{body, pair},
    *,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{UnparsedPublicKey, ED25519};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
const BUDGET: Duration = Duration::from_secs(3);

pub(super) fn session(client: ClientSurface) -> CatalogSession {
    CatalogSession::new(BusinessOrigin::CodebuffWww, "synthetic-token", client).unwrap()
}
pub(super) fn key() -> (Arc<DeviceKey>, Vec<u8>) {
    let (private, public) = pair();
    (
        Arc::new(DeviceKey::from_pkcs8(&private, &public).unwrap()),
        public,
    )
}
pub(super) async fn fetch(
    peer: &Peer,
    session: &mut CatalogSession,
    device: Option<&DeviceRegistration>,
    budget: Duration,
) -> Result<(), CatalogError> {
    TEST_ORIGIN
        .scope(peer.origin.clone(), session.refresh(device, budget))
        .await
        .map(|_| ())
}

#[tokio::test]
async fn bootstrap_register_and_signed_get_are_exact_for_cli_and_desktop() {
    for client in [ClientSurface::Cli, ClientSurface::Desktop] {
        let peer = Peer::start(vec![
            Spec::catalog(),
            Spec::json(200, json!({"keyId":"synthetic-key-id"})),
            Spec::catalog(),
        ])
        .await;
        let mut session = session(client);
        let (key, public) = key();
        assert_eq!(
            session.register_device(key.clone(), BUDGET).await.err(),
            Some(CatalogError::CatalogRequired)
        );
        assert_eq!(peer.count(), 0, "no registration before catalog");
        fetch(&peer, &mut session, None, BUDGET).await.unwrap();
        let registered = TEST_ORIGIN
            .scope(peer.origin.clone(), session.register_device(key, BUDGET))
            .await
            .unwrap();
        fetch(&peer, &mut session, Some(&registered), BUDGET)
            .await
            .unwrap();
        {
            let hits = peer.hits.lock().unwrap();
            assert_eq!(hits.len(), 3, "no admission or automatic retry");
            for hit in &*hits {
                assert_eq!(hit.headers["authorization"], "Bearer synthetic-token");
                assert!(hit.headers.get("cookie").is_none());
                assert!(hit.headers.get("x-freebuff-catalog-fetch").is_none());
                assert!(hit.uri.query().is_none());
            }
            assert_eq!(hits[0].method, "GET");
            assert_eq!(hits[0].uri.path(), "/api/v1/freebuff/models");
            assert_eq!(hits[0].headers["x-freebuff-catalog-protocol"], "1");
            assert_eq!(hits[0].headers["x-freebuff-client"], client.as_str());
            assert!(hits[0].headers.get("x-freebuff-device-sig").is_none());
            assert!(hits[0].body.is_empty());
            assert_eq!(hits[1].method, "POST");
            assert_eq!(hits[1].uri.path(), "/api/v1/freebuff/device-keys");
            let body: Value = serde_json::from_slice(&hits[1].body).unwrap();
            assert_eq!(
                body,
                json!({"client":client.as_str(),"publicKey":URL_SAFE_NO_PAD.encode(&public)})
            );
            assert!(hits[1].headers.get("x-freebuff-device-sig").is_none());
            let signed = &hits[2];
            assert_eq!(signed.method, "GET");
            assert_eq!(signed.uri.path(), "/api/v1/freebuff/models");
            assert_eq!(signed.headers["x-freebuff-device-key"], "synthetic-key-id");
            let timestamp: u64 = signed.headers["x-freebuff-device-ts"]
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            let payload = format!("freebuff-device-v1\nGET\n/api/v1/freebuff/models\n{timestamp}\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n");
            let signature = URL_SAFE_NO_PAD
                .decode(signed.headers["x-freebuff-device-sig"].as_bytes())
                .unwrap();
            UnparsedPublicKey::new(&ED25519, &public)
                .verify(payload.as_bytes(), &signature)
                .unwrap();
        }
        peer.close().await;
    }
}
#[tokio::test]
async fn foreign_registration_is_rejected_before_io_and_existing_registration_can_sign_first_get() {
    let peer = Peer::start(vec![
        Spec::catalog(),
        Spec::json(200, json!({"keyId":"synthetic-key"})),
        Spec::catalog(),
    ])
    .await;
    let mut first = session(ClientSurface::Cli);
    fetch(&peer, &mut first, None, BUDGET).await.unwrap();
    let registration = TEST_ORIGIN
        .scope(peer.origin.clone(), first.register_device(key().0, BUDGET))
        .await
        .unwrap();
    for (origin, token, client) in [
        (
            BusinessOrigin::Codebuff,
            "synthetic-token",
            ClientSurface::Cli,
        ),
        (
            BusinessOrigin::CodebuffWww,
            "changed-token",
            ClientSurface::Cli,
        ),
        (
            BusinessOrigin::CodebuffWww,
            "synthetic-token",
            ClientSurface::Desktop,
        ),
    ] {
        let mut foreign = CatalogSession::new(origin, token, client).unwrap();
        assert_eq!(
            fetch(&peer, &mut foreign, Some(&registration), BUDGET).await,
            Err(CatalogError::ScopeMismatch)
        );
        assert_eq!(peer.count(), 2);
    }
    let mut restarted_owner = session(ClientSurface::Cli);
    fetch(&peer, &mut restarted_owner, Some(&registration), BUDGET)
        .await
        .unwrap();
    assert!(peer.hits.lock().unwrap()[2]
        .headers
        .get("x-freebuff-device-sig")
        .is_some());
    peer.close().await;
}
#[tokio::test]
async fn transient_refresh_retains_snapshot_permanent_or_malformed_answers_clear_it() {
    for (status, error) in [
        (408, CatalogError::Retryable),
        (429, CatalogError::Retryable),
        (500, CatalogError::Retryable),
        (401, CatalogError::Unauthorized),
        (403, CatalogError::Unauthorized),
        (404, CatalogError::Unsupported),
        (405, CatalogError::Unsupported),
        (400, CatalogError::Rejected),
        (200, CatalogError::InvalidResponse),
    ] {
        let peer = Peer::start(vec![
            Spec::catalog(),
            Spec::json(status, json!({"error":"synthetic failure"})),
        ])
        .await;
        let mut session = session(ClientSurface::Cli);
        fetch(&peer, &mut session, None, BUDGET).await.unwrap();
        assert_eq!(fetch(&peer, &mut session, None, BUDGET).await, Err(error));
        assert_eq!(
            session.catalog().is_some(),
            error == CatalogError::Retryable
        );
        assert_eq!(peer.count(), 2);
        peer.close().await;
    }
}
#[tokio::test]
async fn successful_refresh_replaces_handle_and_fetch_together_without_persistent_or_default_agent()
{
    let mut next = body();
    next["fetchId"] = json!("next-fetch");
    next["rows"][0]["handle"] = json!("fbm1.next");
    let peer = Peer::start(vec![Spec::catalog(), Spec::json(200, next)]).await;
    let mut session = session(ClientSurface::Cli);
    fetch(&peer, &mut session, None, BUDGET).await.unwrap();
    fetch(&peer, &mut session, None, BUDGET).await.unwrap();
    let pick = session.catalog().unwrap().select("open").unwrap();
    assert_eq!(
        (pick.key, pick.handle, pick.fetch_id),
        ("open", "fbm1.next", Some("next-fetch"))
    );
    assert_eq!(peer.count(), 2);
    peer.close().await;
}
#[tokio::test]
async fn registration_errors_are_explicit_and_never_auto_retry_or_fall_back_unsigned() {
    for (status, response, expected) in [
        (200, json!({}), CatalogError::InvalidResponse),
        (
            200,
            json!({"keyId":"bad\r\nkey"}),
            CatalogError::InvalidResponse,
        ),
        (
            401,
            json!({"error":"sensitive-token-in-error-prose"}),
            CatalogError::Unauthorized,
        ),
        (404, json!({}), CatalogError::Unsupported),
        (503, json!({}), CatalogError::Retryable),
        (
            403,
            json!({"error":"unknown_device_key"}),
            CatalogError::DeviceUnknown,
        ),
    ] {
        let peer = Peer::start(vec![Spec::catalog(), Spec::json(status, response)]).await;
        let mut session = session(ClientSurface::Cli);
        fetch(&peer, &mut session, None, BUDGET).await.unwrap();
        let error = TEST_ORIGIN
            .scope(
                peer.origin.clone(),
                session.register_device(key().0, BUDGET),
            )
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error} {session:?}").contains("sensitive-token"));
        assert_eq!(peer.count(), 2);
        peer.close().await;
    }
}
