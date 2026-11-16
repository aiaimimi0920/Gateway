//! Real SQLite plus signed loopback requests. No normal route buys an idle session in this sub-batch.
use super::*;
use crate::local_runtime::{
    freebuff_sessions::{AdmissionPreparation, PendingAdmission, SessionRecovery},
    LocalRuntime,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
mod closeout;
mod closeout_history;
mod closeout_races;
mod closeout_safety;
mod completion;
mod completion_safety;
mod execution;
mod multistep;
mod multistep_cancellation;
mod multistep_safety;
mod observed_controls;
mod observed_failures;
mod observed_stream;
mod process;
mod purchase;
mod purchase_races;
#[path = "root_builder/wire_tests.rs"]
mod root_builder;
#[path = "root_context/wire_tests.rs"]
mod root_context;
#[path = "root_builder/history_wire_tests.rs"]
mod root_history;
#[path = "root_summary/wire_tests.rs"]
mod root_summary;
#[path = "root_summary/cancellation_wire_tests.rs"]
mod root_summary_cancellation;
#[path = "root_summary/failure_wire_tests.rs"]
mod root_summary_failures;
mod run_journal;
mod run_process;
mod safety;
mod step_cancellation;
mod step_failures;
mod step_owner;
const BUDGET: Duration = Duration::from_secs(3);
async fn setup() -> (PathBuf, LocalRuntime) {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-claim-{}",
        uuid::Uuid::new_v4()
    ));
    let db = LocalRuntime::open(&root).await.unwrap();
    (root, db)
}
async fn cleanup(root: &Path, db: LocalRuntime) {
    db.close().await;
    crate::local_runtime::test_support::remove_test_root(root).await;
}
fn synthetic(
    token: &str,
    client: ClientSurface,
    origin: BusinessOrigin,
) -> (CatalogSession, DeviceRegistration) {
    let mut session = CatalogSession::new(origin, token, client).unwrap();
    session.catalog =
        Some(ModelCatalog::parse(&serde_json::to_vec(&tests::body()).unwrap()).unwrap());
    let (private, public) = tests::pair();
    let key = Arc::new(DeviceKey::from_pkcs8(&private, &public).unwrap());
    let device = DeviceRegistration::new(&session.binding, key, "synthetic-ready".into());
    (session, device)
}
fn account() -> (CatalogSession, DeviceRegistration) {
    synthetic(
        "synthetic-account",
        ClientSurface::Cli,
        BusinessOrigin::Codebuff,
    )
}
async fn pending<'a>(
    db: &LocalRuntime,
    requests: &'a BusinessRequests<'a>,
) -> PendingAdmission<'a> {
    match db
        .prepare_freebuff_admission(
            "pool",
            "account",
            requests,
            SessionChargeConsent::MaySpendFreebucks,
        )
        .await
        .unwrap()
    {
        AdmissionPreparation::Send(pending) => pending,
        _ => panic!("only the first preparation can expose a POST"),
    }
}
async fn recovery(db: &LocalRuntime, requests: &RecoveryRequests<'_>) -> SessionRecovery {
    db.recover_freebuff_admission("pool", "account", requests)
        .await
        .unwrap()
        .unwrap()
}
async fn row(db: &LocalRuntime) -> (String, String, String) {
    sqlx::query_as("SELECT binding,attempt_id,sealed FROM local_freebuff_session_claims")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}
fn active(claim: &SessionClaim, model: &str) -> Value {
    let now = OffsetDateTime::now_utc();
    json!({"status":"active", "instanceId":claim.instance_id(), "model":model, "accessTier":"full",
        "admittedAt":(now-time::Duration::seconds(1)).format(&time::format_description::well_known::Rfc3339).unwrap(),
        "expiresAt":(now+time::Duration::hours(1)).format(&time::format_description::well_known::Rfc3339).unwrap(), "remainingMs":3600000})
}

#[tokio::test]
async fn freebuff_claim_is_sealed_before_send_and_rebuild_only_recovers_original_identity() {
    let (root, db) = setup().await;
    let peer = fixture::Peer::start(vec![]).await;
    let (mut session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    let claim = prepared.claim().clone();
    let persisted = row(&db).await;
    assert_eq!(persisted.1, claim.attempt_id());
    assert!(!persisted.2.contains(claim.attempt_id()));
    assert!(!persisted.2.contains("synthetic-account"));
    assert_eq!(peer.count(), 0, "preparation is local, not purchase");
    let other = LocalRuntime::open(&root).await.unwrap();
    assert_eq!(
        other
            .prepare_freebuff_admission(
                "pool",
                "account",
                &requests,
                SessionChargeConsent::MaySpendFreebucks
            )
            .await
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("freebuff_session_claim_busy")
    );
    drop(prepared);
    drop(requests);
    db.close().await;
    // A catalog no longer offering the old model must not force a purchase or selection of a new one.
    let mut catalog = tests::body();
    catalog["rows"][0]["access"] = json!("locked");
    session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    assert!(session.business_requests(&device, "open").is_err());
    let requests = session.recovery_requests(&device).unwrap();
    let recovered = recovery(&other, &requests).await;
    assert_eq!(recovered.claim().attempt_id(), claim.attempt_id());
    assert_eq!(recovered.model_key(), "open");
    peer.push(fixture::Spec::json(200, active(&claim, "open")));
    let admitted = with_test_origin(peer.origin.clone(), recovered.heartbeat(&requests, BUDGET))
        .await
        .unwrap();
    assert_eq!(admitted.active().claim().instance_id(), claim.instance_id());
    assert_eq!(admitted.active().model_key(), "open");
    assert!(admitted.active().is_live());
    assert!(admitted.active().expires_at() > OffsetDateTime::now_utc());
    assert_eq!(
        row(&other).await,
        persisted,
        "a live observation is never a serialized execution capability"
    );
    {
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].method, "GET");
        assert_eq!(hits[0].uri.path(), "/api/v1/freebuff/session");
        assert_eq!(
            hits[0].headers["x-freebuff-instance-id"],
            claim.instance_id()
        );
        assert!(hits[0].headers.get("x-freebuff-model").is_none());
        assert!(hits[0]
            .headers
            .get("x-freebuff-wallet-spend-limit")
            .is_none());
        use base64::Engine;
        let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(device.key.public_key())
            .unwrap();
        business_tests::verify(&hits[0], &public, "synthetic-fetch");
    }
    drop(admitted);
    peer.close().await;
    cleanup(&root, other).await;
}

#[tokio::test]
async fn freebuff_claim_one_post_then_existing_preparation_never_repeats_admission() {
    let (root, db) = setup().await;
    let peer = fixture::Peer::start(vec![]).await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let prepared = pending(&db, &requests).await;
    let claim = prepared.claim().clone();
    peer.push(fixture::Spec::json(200, active(&claim, "open")));
    let admitted = with_test_origin(peer.origin.clone(), prepared.send(BUDGET))
        .await
        .unwrap();
    assert!(admitted.active().is_live());
    drop(admitted);
    for _ in 0..2 {
        let AdmissionPreparation::Recover(recovered) = db
            .prepare_freebuff_admission(
                "pool",
                "account",
                &requests,
                SessionChargeConsent::MaySpendFreebucks,
            )
            .await
            .unwrap()
        else {
            panic!("an existing record cannot POST")
        };
        assert_eq!(recovered.claim().attempt_id(), claim.attempt_id());
        drop(recovered);
    }
    {
        let hits = peer.hits.lock().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].method, "POST");
        assert_eq!(hits[0].uri.path(), "/api/v1/freebuff/session/admission");
        assert_eq!(
            hits[0].headers["x-freebuff-desktop-attempt-id"],
            claim.attempt_id()
        );
        assert_eq!(hits[0].headers["x-freebuff-model"], "fbm1.synthetic-open");
        assert_eq!(hits[0].headers["x-freebuff-wallet-spend-limit"], "session");
    }
    peer.close().await;
    cleanup(&root, db).await;
}
