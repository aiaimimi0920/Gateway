#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "provider_management_deletion/fixture.rs"]
mod fixture;
#[path = "provider_management_deletion/schema.rs"]
mod schema;
mod support;

use fixture::{request, Fixture};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn credential_success_preserves_response_and_cleans_all_keys() {
    let mut fixture = Fixture::new().await;
    let id = fixture.credential("key.json", "folder_sync").await;
    std::fs::write(fixture.root.join("key.json"), b"fixture").unwrap();
    let result = request(Arc::clone(&fixture.state), false, &id, true).await;
    let exists = fixture.credential_exists(&id).await;
    let account_exists = fixture.account_exists().await;
    let keys = fixture.key_count(&Fixture::credential_keys(&id)).await;
    let file_exists = fixture.root.join("key.json").exists();
    let expected = serde_json::json!({"success":true,"providerCredentialId":id,
        "providerAccountId":fixture.account,"message":"服务商凭证已删除"});
    fixture.finish().await;
    assert_eq!(result, (200, expected));
    assert!(!exists && !file_exists && account_exists);
    assert_eq!(keys, 0);
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn account_success_cleans_each_credential_before_account() {
    let mut fixture = Fixture::new().await;
    let first = fixture.credential("first.json", "folder_sync").await;
    let second = fixture.credential("missing.json", "folder_sync").await;
    std::fs::write(fixture.root.join("first.json"), b"fixture").unwrap();
    let result = request(Arc::clone(&fixture.state), true, &fixture.account, true).await;
    let exists = fixture.account_exists().await;
    let rows = fixture.credential_exists(&first).await || fixture.credential_exists(&second).await;
    let mut keys = fixture.account_keys();
    keys.extend(Fixture::credential_keys(&first));
    keys.extend(Fixture::credential_keys(&second));
    let keys = fixture.key_count(&keys).await;
    let file_exists = fixture.root.join("first.json").exists();
    let expected = serde_json::json!({"deleted":true,"providerAccountId":fixture.account,
        "label":"Fixture account","deletedCredentialCount":2});
    fixture.finish().await;
    assert_eq!(result, (200, expected));
    assert!(!exists && !rows && !file_exists);
    assert_eq!(keys, 0);
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn invalid_source_stops_before_database_and_redis_deletion() {
    for account in [false, true] {
        let mut fixture = Fixture::new().await;
        let id = fixture.credential("../outside.json", "folder_sync").await;
        let result = request(
            Arc::clone(&fixture.state),
            account,
            if account { &fixture.account } else { &id },
            true,
        )
        .await;
        let rows = fixture.credential_exists(&id).await && fixture.account_exists().await;
        let keys = fixture.key_count(&Fixture::credential_keys(&id)).await;
        fixture.finish().await;
        assert_eq!(result.0, 400);
        assert!(rows);
        assert_eq!(keys, 4);
    }
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn disabled_and_manual_credentials_keep_files() {
    for enabled in [false, true] {
        let mut fixture = Fixture::new().await;
        fixture
            .state
            .provider_credential_folder_sync
            .set_enabled(enabled);
        let id = fixture
            .credential("key.json", if enabled { "manual" } else { "folder_sync" })
            .await;
        std::fs::write(fixture.root.join("key.json"), b"fixture").unwrap();
        let result = request(Arc::clone(&fixture.state), false, &id, true).await;
        let exists = fixture.root.join("key.json").exists();
        let row = fixture.credential_exists(&id).await;
        fixture.finish().await;
        assert_eq!(result.0, 200);
        assert!(exists && !row);
    }
}

async fn cancelled_delete(account: bool) {
    let mut fixture = Fixture::new().await;
    let id = fixture.credential("held.json", "folder_sync").await;
    let file = fixture.root.join("held.json");
    std::fs::write(&file, b"fixture").unwrap();
    let mut transaction = fixture.pg.begin().await.unwrap();
    sqlx::query("select id from gateway_provider_credentials where id=$1 for update")
        .bind(&id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    let state = Arc::clone(&fixture.state);
    let target = if account {
        fixture.account.clone()
    } else {
        id.clone()
    };
    let caller = tokio::spawn(async move { request(state, account, &target, true).await });
    let removed = tokio::time::timeout(Duration::from_secs(3), async {
        while file.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();
    // The row lock establishes a real file -> DB boundary before caller abort.
    let row_before_release = fixture.credential_exists(&id).await;
    caller.abort();
    let cancelled = caller.await.unwrap_err().is_cancelled();
    let busy = request(Arc::clone(&fixture.state), !account, "missing", true).await;
    let unauthorized = request(Arc::clone(&fixture.state), !account, "missing", false).await;
    transaction.rollback().await.unwrap();
    let finished = tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.key_count(&Fixture::credential_keys(&id)).await != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .is_ok();
    fixture.wait_idle().await;
    let exists = fixture.credential_exists(&id).await;
    let account_exists = fixture.account_exists().await;
    let account_keys = fixture.key_count(&fixture.account_keys()).await;
    fixture.finish().await;
    assert!(removed && row_before_release && cancelled);
    assert!(
        finished && !exists,
        "caller cancellation abandoned DB/Redis cleanup"
    );
    assert_eq!(account_exists, !account);
    assert_eq!(account_keys, if account { 0 } else { 5 });
    assert_eq!(busy.0, 409);
    assert!(matches!(unauthorized.0, 401 | 403));
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn cancelled_credential_request_completes_admitted_cleanup() {
    cancelled_delete(false).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn cancelled_account_request_completes_admitted_cleanup() {
    cancelled_delete(true).await;
}
