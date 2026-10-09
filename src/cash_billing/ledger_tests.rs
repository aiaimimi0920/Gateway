use crate::{
    access_balance::{quota::KeyQuotaInput, AccessBalanceStore},
    cash_billing::{CashOutcome, CashQuote, CashReceipt},
    db::UpsertAccessKeyInput,
    local_runtime::LocalRuntime,
};

fn input() -> UpsertAccessKeyInput {
    UpsertAccessKeyInput {
        owner_type: "user".into(),
        owner_id: "fixture".into(),
        resolved_project_id: "local".into(),
        resolved_tenant_id: "local".into(),
        key_kind: "normal".into(),
        public_key_prefix: "sk-gw".into(),
        display_name: "cash".into(),
        expires_at: None,
        bundle_ids: vec![],
        metadata: None,
    }
}

fn quota(limit: i64) -> KeyQuotaInput {
    KeyQuotaInput {
        mode: "cash_prepaid".into(),
        limit: Some(limit),
        currency: Some("USD".into()),
    }
}

fn receipt(key: &str, reserved: i64) -> CashReceipt {
    CashReceipt {
        request_id: uuid::Uuid::new_v4().to_string(),
        access_key_id: key.into(),
        created_at: "2026-10-08T00:00:00Z".into(),
        status: "reserved".into(),
        reserved_micros: reserved,
        amount_micros: None,
        currency: "USD".into(),
        quotes: vec![CashQuote {
            provider_account_id: "p".into(),
            credential_id: "c".into(),
            model: "m".into(),
            group_id: Some("g".into()),
            price_source: "fixture".into(),
            prompt_micros_per_1k_tokens: 1000,
            completion_micros_per_1k_tokens: 2000,
            group_multiplier_ppm: 1_500_000,
            account_multiplier_ppm: 800_000,
            cache_tokens_separate: false,
        }],
        settled_quote: None,
        usage: None,
        reason: None,
    }
}

fn success() -> CashOutcome {
    CashOutcome::Success {
        provider: "p".into(),
        credential: "c".into(),
        model: "m".into(),
        usage: Some(crate::protocol::canonical::TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            ..Default::default()
        }),
    }
}

async fn setup() -> (std::path::PathBuf, LocalRuntime, String) {
    let root = std::env::temp_dir().join(format!("gateway-local-cash-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&root).await.unwrap();
    let key = local
        .save_access_key_with_quota(None, input(), Some(&quota(2000)))
        .await
        .unwrap();
    (root, local, key.id)
}

async fn cleanup(root: &std::path::Path, local: LocalRuntime) {
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(root).await;
}

#[tokio::test]
async fn cash_reservation_is_atomic_under_concurrent_requests_and_finalization_is_idempotent() {
    let (root, local, key) = setup().await;
    let reservations = (0..8).map(|_| receipt(&key, 1000)).collect::<Vec<_>>();
    let results = futures::future::join_all(
        reservations
            .iter()
            .map(|receipt| AccessBalanceStore::Sqlite(&local).reserve_cash(receipt.clone())),
    )
    .await;
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 2);
    let store = AccessBalanceStore::Sqlite(&local);
    let balance = store.cash_balance(&key).await.unwrap().unwrap();
    assert_eq!(balance.reserved_micros, 2000);
    assert_eq!(balance.spent_micros, 0);
    for (receipt, result) in reservations.iter().zip(results) {
        if result.is_ok() {
            store
                .finish_cash(&receipt.request_id, success())
                .await
                .unwrap();
            store
                .finish_cash(&receipt.request_id, success())
                .await
                .unwrap();
        }
    }
    let balance = store.cash_balance(&key).await.unwrap().unwrap();
    assert_eq!(
        (
            balance.spent_micros,
            balance.reserved_micros,
            balance.pending_requests
        ),
        (480, 0, 0)
    );
    assert!(store
        .cash_ledger(&key)
        .await
        .unwrap()
        .iter()
        .all(|row| row.amount_micros == Some(240)));
    cleanup(&root, local).await;
}

#[tokio::test]
async fn cash_unknown_usage_rotation_restart_and_limit_edits_preserve_one_account() {
    let (root, local, key) = setup().await;
    let pending = receipt(&key, 1000);
    let store = AccessBalanceStore::Sqlite(&local);
    store.reserve_cash(pending.clone()).await.unwrap();
    store
        .finish_cash(&pending.request_id, CashOutcome::Unknown)
        .await
        .unwrap();
    let next = local.rotate_access_key(&key).await.unwrap();
    let cash = store.cash_balance(&next.id).await.unwrap().unwrap();
    assert_eq!((cash.spent_micros, cash.reserved_micros), (0, 1000));
    local.close().await;
    let local = LocalRuntime::open(&root).await.unwrap();
    let store = AccessBalanceStore::Sqlite(&local);
    assert_eq!(
        store.cash_ledger(&next.id).await.unwrap()[0].status,
        "unresolved"
    );
    store
        .finish_cash(&pending.request_id, success())
        .await
        .unwrap();
    local
        .save_access_key_with_quota(Some(&next.id), input(), Some(&quota(200)))
        .await
        .unwrap();
    let cash = store.get(&next.id).await.unwrap().unwrap().cash.unwrap();
    assert_eq!((cash.spent_micros, cash.remaining_micros()), (240, -40));
    let no_limit = KeyQuotaInput {
        mode: "unlimited".into(),
        limit: None,
        currency: None,
    };
    local
        .save_access_key_with_quota(Some(&next.id), input(), Some(&no_limit))
        .await
        .unwrap();
    local
        .save_access_key_with_quota(Some(&next.id), input(), Some(&quota(2000)))
        .await
        .unwrap();
    assert_eq!(
        store
            .cash_balance(&next.id)
            .await
            .unwrap()
            .unwrap()
            .spent_micros,
        240
    );
    cleanup(&root, local).await;
}

#[tokio::test]
async fn cash_release_before_dispatch_is_not_a_charge_and_invalid_quota_rolls_back_metadata() {
    let (root, local, key) = setup().await;
    let pending = receipt(&key, 1000);
    let store = AccessBalanceStore::Sqlite(&local);
    store.reserve_cash(pending.clone()).await.unwrap();
    store
        .finish_cash(&pending.request_id, CashOutcome::NotSent)
        .await
        .unwrap();
    store
        .finish_cash(&pending.request_id, CashOutcome::Unknown)
        .await
        .unwrap();
    let cash = store.cash_balance(&key).await.unwrap().unwrap();
    assert_eq!(
        (
            cash.spent_micros,
            cash.reserved_micros,
            cash.pending_requests
        ),
        (0, 0, 0)
    );
    let mut changed = input();
    changed.display_name = "must-not-save".into();
    assert!(local
        .save_access_key_with_quota(Some(&key), changed, Some(&quota(-1)))
        .await
        .is_err());
    assert_eq!(
        local.access_catalog().await.unwrap().access_keys[0].display_name,
        "cash"
    );
    cleanup(&root, local).await;
}
