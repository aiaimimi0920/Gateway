//! The same spending contract runs against the two real database adapters.
use neuro_gateway::access_balance::AccessBalanceStore;
use neuro_gateway::db::{AccessKeyBalanceAdjustInput as Adjustment, UpsertAccessKeyInput};
use neuro_gateway::local_runtime::LocalRuntime;

fn key_input() -> UpsertAccessKeyInput {
    UpsertAccessKeyInput {
        owner_type: "user".into(),
        owner_id: "fixture".into(),
        resolved_project_id: "local".into(),
        resolved_tenant_id: "local".into(),
        key_kind: "normal".into(),
        public_key_prefix: "sk-gw".into(),
        display_name: "balance contract".into(),
        expires_at: None,
        metadata: None,
        bundle_ids: vec![],
    }
}

async fn spending_contract(store: AccessBalanceStore<'_>, id: &str) {
    store
        .adjust(
            id,
            Adjustment {
                balance_mode: Some(" TOKEN_PREPAID ".into()),
                total_tokens: Some(50),
                remaining_tokens: Some(50),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let decisions = futures::future::join_all((0..40).map(|_| store.reserve(id, 10))).await;
    assert_eq!(
        decisions
            .into_iter()
            .filter(|value| value.as_ref().unwrap().allowed)
            .count(),
        5
    );
    assert_eq!(
        store.get(id).await.unwrap().unwrap().remaining_tokens,
        Some(0)
    );
    store.refund(id, 10).await.unwrap();
    store.settle(id, 10, 3).await.unwrap();
    for _ in 0..3 {
        store.settle(id, 10, 10).await.unwrap();
    }
    let balance = store.get(id).await.unwrap().unwrap();
    assert_eq!(
        (balance.total_tokens, balance.remaining_tokens),
        (Some(50), Some(17))
    );
    let before = serde_json::to_value(&balance).unwrap();
    assert!(!store.reserve(id, u64::MAX).await.unwrap().allowed);
    assert!(store
        .adjust(
            id,
            Adjustment {
                token_delta: Some(i64::MAX),
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert_eq!(
        serde_json::to_value(store.get(id).await.unwrap().unwrap()).unwrap(),
        before
    );
    for result in futures::future::join_all((0..20).map(|_| {
        store.adjust(
            id,
            Adjustment {
                token_delta: Some(1),
                ..Default::default()
            },
        )
    }))
    .await
    {
        result.unwrap();
    }
    let topped_up = store.get(id).await.unwrap().unwrap();
    assert_eq!(
        (topped_up.total_tokens, topped_up.remaining_tokens),
        (Some(70), Some(37))
    );

    store
        .adjust(
            id,
            Adjustment {
                balance_mode: Some("message_prepaid".into()),
                total_messages: Some(2),
                remaining_messages: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(store.reserve(id, u64::MAX).await.unwrap().allowed);
    assert!(store.reserve(id, 0).await.unwrap().allowed);
    assert!(!store.reserve(id, 0).await.unwrap().allowed);
    store.refund(id, 1).await.unwrap();
    store.settle(id, 1, 100).await.unwrap();
    let balance = store.get(id).await.unwrap().unwrap();
    assert_eq!(
        (balance.total_messages, balance.remaining_messages),
        (Some(2), Some(1))
    );

    store
        .adjust(
            id,
            Adjustment {
                balance_mode: Some("time_pass".into()),
                unlimited_until: Some("2000-01-01T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(!store.reserve(id, 1).await.unwrap().allowed);
    assert!(store
        .adjust(
            id,
            Adjustment {
                unlimited_until: Some("invalid".into()),
                ..Default::default()
            }
        )
        .await
        .is_err());
    store
        .adjust(
            id,
            Adjustment {
                unlimited_until: Some("2999-01-01T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(store.reserve(id, u64::MAX).await.unwrap().allowed);
    store
        .adjust(
            id,
            Adjustment {
                period_starts_at: Some("2998-01-01T00:00:00Z".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(!store.reserve(id, 1).await.unwrap().allowed);
    assert!(store
        .adjust(
            id,
            Adjustment {
                period_ends_at: Some("2997-01-01T00:00:00Z".into()),
                ..Default::default()
            }
        )
        .await
        .is_err());
}

async fn remove_root(root: &std::path::Path) {
    assert!(root.starts_with(std::env::temp_dir()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("gateway-local-balance-"));
    for attempt in 0..40 {
        match std::fs::remove_dir_all(root) {
            Ok(()) => return,
            Err(_) if attempt < 39 => {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await
            }
            Err(error) => panic!("isolated fixture cleanup failed: {error}"),
        }
    }
}

#[tokio::test]
async fn sqlite_spending_contract() {
    let root = std::env::temp_dir().join(format!("gateway-local-balance-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&root).await.unwrap();
    let key = local.save_access_key(None, key_input()).await.unwrap();
    spending_contract(AccessBalanceStore::Sqlite(&local), &key.id).await;
    local.close().await;
    remove_root(&root).await;
}

#[tokio::test]
async fn sqlite_rotation_restart_and_inflight_deletion_share_one_account() {
    let root = std::env::temp_dir().join(format!("gateway-local-balance-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&root).await.unwrap();
    let key = local.save_access_key(None, key_input()).await.unwrap();
    let store = AccessBalanceStore::Sqlite(&local);
    assert_eq!(
        store.get(&key.id).await.unwrap().unwrap().balance_mode,
        "unlimited"
    );
    store
        .adjust(
            &key.id,
            Adjustment {
                balance_mode: Some("token_prepaid".into()),
                total_tokens: Some(100),
                remaining_tokens: Some(100),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(store.reserve(&key.id, 20).await.unwrap().allowed);
    let rotated = local.rotate_access_key(&key.id).await.unwrap();
    local.delete_access_key(&key.id).await.unwrap();
    store.settle(&key.id, 20, 7).await.unwrap();
    assert_eq!(
        store
            .get(&rotated.id)
            .await
            .unwrap()
            .unwrap()
            .remaining_tokens,
        Some(93)
    );
    assert!(store.reserve(&key.id, 1).await.is_err());
    local.close().await;
    let local = LocalRuntime::open(&root).await.unwrap();
    let store = AccessBalanceStore::Sqlite(&local);
    let balance = store.get(&rotated.id).await.unwrap().unwrap();
    assert_eq!(
        (balance.total_tokens, balance.remaining_tokens),
        (Some(100), Some(93))
    );
    let request = neuro_gateway::auth::adapter::AuthRequest {
        authorization: Some(format!("Bearer {}", rotated.token.unwrap())),
        api_key: None,
        path: "/v1/chat/completions".into(),
        method: "POST".into(),
    };
    assert!(local
        .authenticate_access_key(&request)
        .await
        .unwrap()
        .is_some());
    local.close().await;
    remove_root(&root).await;
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL URL in GATEWAY_TEST_BALANCE_POSTGRES_URL"]
async fn postgres_spending_contract() {
    use std::str::FromStr;
    let url =
        std::env::var("GATEWAY_TEST_BALANCE_POSTGRES_URL").expect("disposable PostgreSQL URL");
    let base = sqlx::PgPool::connect(&url).await.unwrap();
    let schema = format!("balance_contract_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&base)
        .await
        .unwrap();
    let options = sqlx::postgres::PgConnectOptions::from_str(&url)
        .unwrap()
        .options([("search_path", schema.as_str())]);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE gateway_access_keys (id TEXT PRIMARY KEY, status TEXT NOT NULL, expires_at TIMESTAMPTZ);
        CREATE TABLE gateway_access_key_balances (access_key_id TEXT PRIMARY KEY REFERENCES gateway_access_keys(id),
        balance_mode TEXT NOT NULL, status TEXT NOT NULL, unlimited_until TIMESTAMPTZ, period_starts_at TIMESTAMPTZ,
        period_ends_at TIMESTAMPTZ, total_tokens BIGINT, remaining_tokens BIGINT, total_messages BIGINT,
        remaining_messages BIGINT, updated_at TIMESTAMPTZ NOT NULL);
        INSERT INTO gateway_access_keys(id,status) VALUES ('fixture','active');")
        .execute(&pool).await.unwrap();
    spending_contract(AccessBalanceStore::Postgres(&pool), "fixture").await;
    let redis = neuro_gateway::redis::pool::disabled_pool().unwrap();
    assert!(
        neuro_gateway::db::get_access_key_balance(&pool, &redis, "fixture")
            .await
            .unwrap()
            .is_some()
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&base)
        .await
        .unwrap();
    base.close().await;
}
