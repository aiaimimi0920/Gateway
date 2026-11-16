//! New wire uses the current catalog and the exact finalized body, never the legacy executor.
use super::{fixture::*, wire_tests::*, *};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{UnparsedPublicKey, ED25519};
use serde_json::json;
const BUDGET: Duration = Duration::from_secs(3);

pub(super) fn verify(hit: &Hit, public: &[u8], fetch: &str) {
    let timestamp = hit.headers["x-freebuff-device-ts"].to_str().unwrap();
    let hash = hex::encode(ring::digest::digest(&ring::digest::SHA256, &hit.body));
    let payload = format!(
        "freebuff-device-v1\n{}\n{}\n{timestamp}\n{hash}\n{fetch}",
        hit.method,
        hit.uri.path()
    );
    let sig = URL_SAFE_NO_PAD
        .decode(hit.headers["x-freebuff-device-sig"].as_bytes())
        .unwrap();
    UnparsedPublicKey::new(&ED25519, public)
        .verify(payload.as_bytes(), &sig)
        .unwrap();
}

pub(super) async fn ready(
    peer: &Peer,
    client: ClientSurface,
) -> (CatalogSession, DeviceRegistration, Vec<u8>) {
    let mut catalog = session(client);
    let (key, public) = key();
    with_test_origin(peer.origin.clone(), async {
        catalog.refresh(None, BUDGET).await.unwrap();
        let device = catalog.register_device(key, BUDGET).await.unwrap();
        (catalog, device, public)
    })
    .await
}

#[tokio::test]
async fn desktop_inspection_is_signed_but_cli_purchase_identity_is_not_fabricated() {
    let peer = Peer::start(vec![
        Spec::catalog(),
        Spec::json(200, json!({"keyId":"device"})),
        Spec::json(200, json!({"status":"none"})),
    ])
    .await;
    with_test_origin(peer.origin.clone(), async {
        let (catalog, device, public) = ready(&peer, ClientSurface::Desktop).await;
        let scope = catalog.business_requests(&device, "open").unwrap();
        assert!(matches!(
            scope.prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks),
            Err(BusinessError::Catalog(CatalogError::Unsupported))
        ));
        scope.inspect(BUDGET).await.unwrap();
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[2].headers["x-freebuff-client"], "desktop");
        assert!(hits[2].headers.get("x-freebuff-instance-id").is_none());
        assert!(hits[2].headers.get("x-freebuff-multi-session").is_none());
        verify(&hits[2], &public, "synthetic-fetch");
    })
    .await;
    peer.close().await;
}

#[tokio::test]
async fn final_completion_bytes_and_session_operations_are_signed_without_legacy_paths() {
    let peer = Peer::start(vec![
        Spec::catalog(),
        Spec::json(200, json!({"keyId":"device"})),
        Spec::json(200, json!({"status":"none"})),
        Spec::json(200, json!({"status":"active"})),
        Spec::json(200, json!({"status":"ended"})),
        Spec::json(200, json!({"choices":[]})),
    ])
    .await;
    let mut catalog = session(ClientSurface::Cli);
    let (key, public) = key();
    with_test_origin(peer.origin.clone(), async {
        catalog.refresh(None, BUDGET).await.unwrap();
        let device = catalog.register_device(key, BUDGET).await.unwrap();
        let scope = catalog.business_requests(&device,"open").unwrap();
        let status = scope.inspect(BUDGET).await.unwrap();
        assert_eq!(status.status,200);
        let admission = scope.prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks).unwrap();
        let claim = admission.claim().clone();
        assert_eq!(peer.count(),3,"preparation is not a purchase");
        admission.send(BUDGET).await.unwrap();
        scope.end(&claim,BUDGET).await.unwrap();
        let body = json!({"model":"fbm1.synthetic-open","stream":true,"messages":[{"role":"user","content":"中文 \\\""}],"codebuff_metadata":{"run_id":"synthetic-run"}});
        let expected = serde_json::to_vec(&body).unwrap();
        scope.prepare_completion(&body, 32768).unwrap().send(BUDGET).await.unwrap();
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(),6);
        assert_eq!(hits[2].uri.path(),"/api/v1/freebuff/session");
        assert_eq!(hits[2].method,"GET");
        assert!(hits[2].headers.get("x-freebuff-wallet-spend-limit").is_none());
        assert_eq!(hits[3].uri.path(),"/api/v1/freebuff/session/admission");
        assert_eq!(hits[3].headers["x-freebuff-wallet-spend-limit"],"session");
        assert_eq!(hits[3].headers["x-freebuff-model"],"fbm1.synthetic-open");
        assert_eq!(hits[4].uri.path(),"/api/v1/freebuff/session/attempt");
        assert_eq!(hits[4].method,"DELETE");
        assert_eq!(hits[3].headers["x-freebuff-instance-id"],hits[4].headers["x-freebuff-instance-id"]);
        assert_eq!(hits[5].uri.path(),"/api/v1/chat/completions");
        assert_eq!(hits[5].body,expected);
        for hit in &hits[2..] {
            verify(hit,&public,"synthetic-fetch");
            assert_eq!(hit.headers["authorization"],"Bearer synthetic-token");
            assert!(hit.headers.get("cookie").is_none());
            assert!(hit.headers.get("x-freebuff-takeover-instance-id").is_none());
        }
        for hit in &hits[2..5] { assert!(hit.body.is_empty()); }
    }).await;
    peer.close().await;
}
