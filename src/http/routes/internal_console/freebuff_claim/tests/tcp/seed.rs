//! Existing-claim test setup through normal registration and the private sealed prepare boundary.
use super::*;
pub(super) async fn original(host: &Host, peer: &wire::Peer) -> String {
    let grant = host.grant().await;
    let mut input = host.input();
    input["businessOrigin"] = json!("https://codebuff.com");
    input["client"] = json!("cli");
    let (status, body) = host
        .request(
            "/v1/internal/gateway/console/freebuff-device-registration",
            input,
            Some(&grant),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    let local = host.state.local_runtime.as_ref().unwrap();
    let mut session =
        CatalogSession::new(BusinessOrigin::Codebuff, TOKEN, ClientSurface::Cli).unwrap();
    let device = local
        .freebuff_registration("pool", "b", &session)
        .await
        .unwrap()
        .unwrap();
    crate::protocol::freebuff::catalog::with_test_origin(
        peer.origin.clone(),
        session.refresh(Some(&device), Duration::from_secs(3)),
    )
    .await
    .unwrap();
    let requests = session.business_requests(&device, "open").unwrap();
    // Test-only journal preparation: no admission POST, idle purchase route or new consent bypass.
    let crate::local_runtime::freebuff_sessions::AdmissionPreparation::Send(pending) = local
        .prepare_freebuff_admission(
            "pool",
            "b",
            &requests,
            crate::protocol::freebuff::catalog::SessionChargeConsent::MaySpendFreebucks,
        )
        .await
        .unwrap()
    else {
        panic!("one original fixture claim required")
    };
    let id = pending.claim().attempt_id().to_owned();
    drop(pending);
    peer.hits.lock().unwrap().clear();
    id
}
