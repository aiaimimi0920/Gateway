//! Opt-in Lua contracts run only against a newly created, loopback-only disposable Redis.
use neuro_gateway::console::{
    RouteConfigRedisActivationOutcome as Activation, RouteConfigRedisSettlement as Settlement,
    TransactionRecord,
};
use redis::AsyncCommands;
#[path = "console_transaction_fence_redis_contract/support.rs"]
mod support;
use support::*;

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn missing_gate_is_fenced_before_late_prepared_and_has_no_expiry() {
    let f = Fixture::new();
    assert_eq!(
        f.store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .unwrap(),
        Settlement::Revoked
    );
    let first = f.gate(&f.prepared).await.unwrap();
    assert_eq!(
        serde_json::from_str::<TransactionRecord>(&first).unwrap(),
        f.prepared.revocation_tombstone().unwrap()
    );
    assert!(f
        .store
        .store_prepared_transaction(&f.pool, &f.prepared)
        .await
        .is_err());
    assert_eq!(
        f.store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .unwrap(),
        Settlement::Revoked
    );
    assert_eq!(f.gate(&f.prepared).await.unwrap(), first);
    let ttl: i64 = f
        .pool
        .get()
        .await
        .unwrap()
        .ttl(f.store.keys().transaction_key(f.prepared.tx_id()))
        .await
        .unwrap();
    assert_eq!(ttl, -1);
    assert!(f
        .store
        .load_active_revision(&f.pool)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn revocation_rejects_late_new_and_legacy_activation_without_writes() {
    let f = Fixture::new();
    f.stage().await;
    assert_eq!(
        f.store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .unwrap(),
        Settlement::Revoked
    );
    assert_eq!(
        f.activate(&f.prepared, &f.activated).await,
        Activation::PreparedTransactionMismatch
    );
    assert_eq!(f.legacy_activate().await, ["prepared_transaction_mismatch"]);
    assert!(f
        .store
        .load_active_revision(&f.pool)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn normal_old_active_cas_is_fenced_for_both_new_and_legacy_scripts() {
    let f = Fixture::new();
    f.install_old().await;
    f.stage().await;
    let old = f
        .store
        .load_active_revision(&f.pool)
        .await
        .unwrap()
        .unwrap()
        .to_json()
        .unwrap();
    assert_eq!(
        f.store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .unwrap(),
        Settlement::Revoked
    );
    assert_eq!(
        f.store
            .activate_revision(
                &f.pool,
                Some(f.old_revision.metadata().id()),
                &f.revision,
                &f.prepared,
                &f.activated
            )
            .await
            .unwrap(),
        Activation::PreparedTransactionMismatch
    );
    assert_eq!(
        f.legacy_activate_expected(Some(f.old_revision.metadata().id()))
            .await,
        ["prepared_transaction_mismatch"]
    );
    assert_eq!(
        f.store
            .load_active_revision(&f.pool)
            .await
            .unwrap()
            .unwrap()
            .to_json()
            .unwrap(),
        old
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn activation_first_settles_only_its_own_exact_transaction() {
    let f = Fixture::new();
    f.stage().await;
    assert_eq!(
        f.activate(&f.prepared, &f.activated).await,
        Activation::Activated
    );
    assert_eq!(
        f.activate(&f.prepared, &f.activated).await,
        Activation::AlreadyActive
    );
    assert_eq!(
        f.store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .unwrap(),
        Settlement::Activated
    );
    let (other, other_activated) = records("tx-other", &f.revision);
    f.store
        .store_prepared_transaction(&f.pool, &other)
        .await
        .unwrap();
    assert!(matches!(
        f.activate(&other, &other_activated).await,
        Activation::RevisionConflict { .. }
    ));
    assert_eq!(
        f.store.settle_transaction(&f.pool, &other).await.unwrap(),
        Settlement::Revoked
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn successor_with_same_revision_never_overrides_old_tombstone_or_gets_rolled_back() {
    let f = Fixture::new();
    f.stage().await;
    f.store
        .settle_transaction(&f.pool, &f.prepared)
        .await
        .unwrap();
    let tombstone = f.gate(&f.prepared).await;
    let (next, next_activated) = records("tx-successor", &f.revision);
    f.store
        .store_prepared_transaction(&f.pool, &next)
        .await
        .unwrap();
    assert_eq!(
        f.activate(&next, &next_activated).await,
        Activation::Activated
    );
    assert_eq!(
        f.store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .unwrap(),
        Settlement::Revoked
    );
    assert_eq!(
        f.activate(&f.prepared, &f.activated).await,
        Activation::PreparedTransactionMismatch
    );
    assert_eq!(f.legacy_activate().await, ["already_active"]);
    assert_eq!(f.gate(&f.prepared).await, tombstone);
    assert_eq!(
        f.store.settle_transaction(&f.pool, &next).await.unwrap(),
        Settlement::Activated
    );
    assert_eq!(
        f.store
            .load_active_revision(&f.pool)
            .await
            .unwrap()
            .unwrap()
            .to_json()
            .unwrap(),
        f.revision.to_json().unwrap()
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn reordered_legacy_json_is_validated_before_exact_wire_cas() {
    let f = Fixture::new();
    f.stage().await;
    f.write_gate(&serde_json::to_string_pretty(&f.prepared).unwrap())
        .await;
    assert_eq!(
        f.store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .unwrap(),
        Settlement::Revoked
    );
    assert!(f
        .store
        .store_prepared_transaction(&f.pool, &f.prepared)
        .await
        .is_err());
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn already_active_requires_the_exact_immutable_revision_payload() {
    let f = Fixture::new();
    f.stage().await;
    assert_eq!(
        f.activate(&f.prepared, &f.activated).await,
        Activation::Activated
    );
    let _: () = f
        .pool
        .get()
        .await
        .unwrap()
        .set(
            f.store.keys().revision_key(f.revision.metadata().id()),
            "corrupt",
        )
        .await
        .unwrap();
    assert_eq!(
        f.activate(&f.prepared, &f.activated).await,
        Activation::RevisionPayloadMismatch
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn invalid_remote_identity_phase_and_json_are_never_overwritten() {
    let f = Fixture::new();
    f.stage().await;
    for raw in [
        "{broken".to_owned(),
        {
            let mut v = serde_json::to_value(&f.prepared).unwrap();
            v["txId"] = "tx-foreign".into();
            v.to_string()
        },
        {
            let mut v = serde_json::to_value(&f.prepared).unwrap();
            v["phase"] = "unknown".into();
            v.to_string()
        },
    ] {
        f.write_gate(&raw).await;
        assert!(f
            .store
            .settle_transaction(&f.pool, &f.prepared)
            .await
            .is_err());
        assert_eq!(f.gate(&f.prepared).await.unwrap(), raw);
        assert!(f
            .store
            .load_active_revision(&f.pool)
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
#[ignore = "requires explicit disposable Redis URL"]
async fn fence_namespace_does_not_create_or_mutate_another_namespace() {
    let f = Fixture::new();
    let other = Fixture::new();
    other.stage().await;
    assert_eq!(
        other.activate(&other.prepared, &other.activated).await,
        Activation::Activated
    );
    let gate = other.gate(&other.prepared).await;
    let active = other
        .store
        .load_active_revision(&other.pool)
        .await
        .unwrap()
        .unwrap()
        .to_json()
        .unwrap();
    f.store
        .settle_transaction(&f.pool, &f.prepared)
        .await
        .unwrap();
    assert_eq!(other.gate(&other.prepared).await, gate);
    assert_eq!(
        other
            .store
            .load_active_revision(&other.pool)
            .await
            .unwrap()
            .unwrap()
            .to_json()
            .unwrap(),
        active
    );
}
