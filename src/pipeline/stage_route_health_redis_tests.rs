use deadpool_redis::{Config, Runtime};
use redis::AsyncCommands;
use serde_json::json;

use super::{breaker_key, quota_key, read_breakers_open, read_cached_quota_snapshots};
use crate::redis::keys;

fn mget_calls(info: &str) -> u64 {
    info.lines()
        .find_map(|line| line.strip_prefix("cmdstat_mget:"))
        .and_then(|stats| {
            stats
                .split(',')
                .find_map(|field| field.strip_prefix("calls="))
        })
        .and_then(|calls| calls.parse().ok())
        .unwrap_or(0)
}

fn quota_snapshot(account_id: &str, credential_id: Option<&str>, status: &str) -> String {
    json!({
        "providerAccountId": account_id,
        "providerCredentialId": credential_id,
        "providerType": "contract",
        "source": "route_health_batch_test",
        "status": status,
        "ready": true,
        "checkedAt": "2026-09-02T00:00:00Z",
        "nextCheckAt": "2999-09-02T00:00:00Z",
        "nextResetAt": null,
        "planType": null,
        "representativeClaim": null,
        "windows": [],
        "error": null,
        "rawData": {}
    })
    .to_string()
}

#[test]
fn credential_scoped_keys_take_precedence_over_account_keys() {
    assert_eq!(
        quota_key("account-a", Some("credential-a")),
        keys::provider_credential_quota_snapshot_key("credential-a")
    );
    assert_eq!(
        breaker_key("account-a", Some("credential-a")),
        keys::provider_credential_breaker_open_key("credential-a")
    );
}

#[tokio::test]
async fn empty_batches_do_not_open_a_redis_connection() {
    let pool = Config::from_url("redis://127.0.0.1:1")
        .create_pool(Some(Runtime::Tokio1))
        .expect("create lazy Redis pool");
    let empty: &[(&str, Option<&str>)] = &[];

    assert!(read_cached_quota_snapshots(&pool, empty)
        .await
        .expect("empty quota batch")
        .is_empty());
    assert!(read_breakers_open(&pool, empty).await.is_empty());
}

#[tokio::test]
#[ignore = "requires GATEWAY_ROUTE_HEALTH_TEST_REDIS_URL pointing to disposable Redis"]
async fn batches_quota_and_breaker_reads_into_two_mget_commands() {
    let redis_url = std::env::var("GATEWAY_ROUTE_HEALTH_TEST_REDIS_URL")
        .expect("set GATEWAY_ROUTE_HEALTH_TEST_REDIS_URL for this ignored test");
    let pool = Config::from_url(redis_url)
        .create_pool(Some(Runtime::Tokio1))
        .expect("create Redis test pool");
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let account_a = format!("batch-account-a-{suffix}");
    let account_b = format!("batch-account-b-{suffix}");
    let account_missing = format!("batch-account-missing-{suffix}");
    let account_malformed = format!("batch-account-malformed-{suffix}");
    let credential_b = format!("batch-credential-b-{suffix}");
    let quota_keys = [
        keys::provider_quota_snapshot_key(&account_a),
        keys::provider_quota_snapshot_key(&account_b),
        keys::provider_credential_quota_snapshot_key(&credential_b),
        keys::provider_quota_snapshot_key(&account_malformed),
    ];
    let breaker_keys = [
        keys::provider_breaker_open_key(&account_a),
        keys::provider_breaker_open_key(&account_b),
    ];

    let mut conn = pool.get().await.expect("get Redis connection");
    let _: () = redis::pipe()
        .set(
            &quota_keys[0],
            quota_snapshot(&account_a, None, "available"),
        )
        .set(
            &quota_keys[1],
            quota_snapshot(&account_b, None, "exhausted"),
        )
        .set(
            &quota_keys[2],
            quota_snapshot(&account_b, Some(&credential_b), "warning"),
        )
        .set(&quota_keys[3], "{malformed")
        .set(&breaker_keys[0], "1")
        .set(&breaker_keys[1], "1")
        .query_async(&mut conn)
        .await
        .expect("seed route-health cache keys");
    let before = redis::cmd("INFO")
        .arg("commandstats")
        .query_async::<String>(&mut conn)
        .await
        .map(|info| mget_calls(&info))
        .expect("read command stats before batch");

    let quota = read_cached_quota_snapshots(
        &pool,
        &[
            (&account_a, None),
            (&account_b, Some(credential_b.as_str())),
            (&account_missing, None),
            (&account_malformed, None),
        ],
    )
    .await
    .expect("read quota batch");
    let breakers = read_breakers_open(
        &pool,
        &[
            (&account_a, None),
            (&account_b, Some(credential_b.as_str())),
            (&account_missing, None),
        ],
    )
    .await;
    let after = redis::cmd("INFO")
        .arg("commandstats")
        .query_async::<String>(&mut conn)
        .await
        .map(|info| mget_calls(&info))
        .expect("read command stats after batch");
    let cleanup_keys = quota_keys
        .iter()
        .chain(breaker_keys.iter())
        .collect::<Vec<_>>();
    let _: usize = conn
        .del(cleanup_keys)
        .await
        .expect("clean route-health keys");

    assert_eq!(after - before, 2);
    assert_eq!(
        quota[0].as_ref().map(|item| item.status.as_str()),
        Some("available")
    );
    assert_eq!(
        quota[1].as_ref().map(|item| item.status.as_str()),
        Some("warning")
    );
    assert!(quota[2].is_none());
    assert!(quota[3].is_none());
    assert_eq!(breakers, vec![true, false, false]);
}
