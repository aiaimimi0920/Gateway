//! Real PostgreSQL contracts using only the guarded synthetic fixture database.
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "provider_management_deletion/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "provider_credential_folder_sync_database/schema.rs"]
mod schema;
mod support;

use neuro_gateway::db::{self, provider_credentials::update_provider_credential_from_snapshot};
use serde_json::json;

fn input(account: &str, token: &str) -> db::UpsertProviderCredentialInput {
    db::UpsertProviderCredentialInput {
        provider_account_id: account.into(),
        label: "Synthetic import".into(),
        status: Some("active".into()),
        payload: json!({"apiKey": token}),
        source_kind: Some("folder_sync_import".into()),
        source_path: Some("fixture/material.json".into()),
        source_hash: Some("synthetic-file-hash".into()),
        sync_mode: Some("folder_sync".into()),
        sync_state: Some("imported".into()),
        sync_error: None,
    }
}

async fn version(f: &fixture::Fixture, id: &str) -> String {
    sqlx::query_scalar("select xmin::text from gateway_provider_credentials where id=$1")
        .bind(id)
        .fetch_one(&f.pg)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn current_snapshot_updates_without_changing_identity() {
    let mut f = fixture::Fixture::new().await;
    let id = f.credential("fixture/material.json", "folder_sync").await;
    let snapshot = version(&f, &id).await;
    let updated = update_provider_credential_from_snapshot(
        &f.pg,
        &id,
        &snapshot,
        input(&f.account, "synthetic-file"),
    )
    .await
    .unwrap();
    assert_eq!(updated.id, id);
    assert_eq!(updated.payload["apiKey"], "synthetic-file");
    assert_ne!(version(&f, &id).await, snapshot);
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn delayed_snapshot_cannot_overwrite_a_later_refresh() {
    let mut f = fixture::Fixture::new().await;
    let id = f.credential("fixture/material.json", "folder_sync").await;
    let snapshot = version(&f, &id).await;
    let current =
        db::update_provider_credential(&f.pg, &id, input(&f.account, "synthetic-refreshed"))
            .await
            .unwrap();
    let result = update_provider_credential_from_snapshot(
        &f.pg,
        &id,
        &snapshot,
        input(&f.account, "synthetic-old-file"),
    )
    .await;
    let retained = db::get_provider_credential(&f.pg, &id)
        .await
        .unwrap()
        .unwrap();
    f.finish().await;
    assert_eq!(
        serde_json::to_value(&retained).unwrap(),
        serde_json::to_value(&current).unwrap(),
        "late file snapshot replaced refreshed material"
    );
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("provider_credential_snapshot_conflict")
    );
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn unchanged_timestamp_does_not_hide_a_new_row_version() {
    let mut f = fixture::Fixture::new().await;
    let id = f.credential("fixture/material.json", "folder_sync").await;
    let snapshot = version(&f, &id).await;
    sqlx::query(
        "update gateway_provider_credentials set label='Synthetic newer label' where id=$1",
    )
    .bind(&id)
    .execute(&f.pg)
    .await
    .unwrap();
    let result = update_provider_credential_from_snapshot(
        &f.pg,
        &id,
        &snapshot,
        input(&f.account, "synthetic-old-file"),
    )
    .await;
    let retained = db::get_provider_credential(&f.pg, &id)
        .await
        .unwrap()
        .unwrap();
    f.finish().await;
    assert_eq!(retained.label, "Synthetic newer label");
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("provider_credential_snapshot_conflict")
    );
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn stale_snapshot_is_rejected_before_object_storage_side_effects() {
    object_conflict(false).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn stale_snapshot_cannot_put_over_a_newer_object() {
    object_conflict(true).await;
}

async fn object_conflict(large: bool) {
    assert_eq!(
        std::env::var("AI_GATEWAY_OBJECT_STORAGE_DRIVER").as_deref(),
        Ok("local")
    );
    let root = std::env::var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR").expect("isolated object root");
    assert!(std::path::Path::new(&root).starts_with(std::env::temp_dir()));
    let mut f = fixture::Fixture::new().await;
    let id = f.credential("fixture/material.json", "folder_sync").await;
    let snapshot = version(&f, &id).await;
    let key = format!("synthetic/{id}.json");
    let current = json!({"apiKey": "synthetic-newer-object"});
    let storage = neuro_gateway::object_storage::gateway_object_storage().unwrap();
    storage.put_json(&key, &current).await.unwrap();
    sqlx::query("update gateway_provider_credentials set payload_inline=null, payload_object_key=$2, storage_mode='r2' where id=$1")
        .bind(&id).bind(&key).execute(&f.pg).await.unwrap();
    let mut incoming = input(&f.account, "synthetic-old-file");
    if large {
        incoming.payload["padding"] = json!("x".repeat(5000));
    }
    let result = update_provider_credential_from_snapshot(&f.pg, &id, &snapshot, incoming).await;
    let retained = storage.read_json(&key).await;
    f.finish().await;
    assert_eq!(
        retained.unwrap(),
        current,
        "stale update changed or deleted the current object"
    );
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("provider_credential_snapshot_conflict")
    );
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn waiting_snapshot_rechecks_after_concurrent_commit() {
    let mut f = fixture::Fixture::new().await;
    let id = f.credential("fixture/material.json", "folder_sync").await;
    let snapshot = version(&f, &id).await;
    let mut writer = f.pg.begin().await.unwrap();
    let writer_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *writer)
        .await
        .unwrap();
    sqlx::query("update gateway_provider_credentials set payload_inline=$2 where id=$1")
        .bind(&id)
        .bind(sqlx::types::Json(json!({"apiKey":"synthetic-concurrent"})))
        .execute(&mut *writer)
        .await
        .unwrap();
    let pool = f.pg.clone();
    let id_copy = id.clone();
    let incoming = input(&f.account, "synthetic-old-file");
    let mut pending = tokio::spawn(async move {
        update_provider_credential_from_snapshot(&pool, &id_copy, &snapshot, incoming).await
    });
    let blocked =
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)))"
            ).bind(writer_pid).fetch_one(&f.pg).await.unwrap();
                if waiting {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;
    // Release the fixture writer even when the expected blocking observation failed.
    writer.commit().await.unwrap();
    let result = match tokio::time::timeout(std::time::Duration::from_secs(5), &mut pending).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => {
            f.finish().await;
            panic!("snapshot task failed: {error}");
        }
        Err(error) => {
            // A timed-out JoinHandle must not detach a writer from this fixture.
            pending.abort();
            let _ = pending.await;
            f.finish().await;
            panic!("snapshot task did not complete: {error}");
        }
    };
    let retained = db::get_provider_credential(&f.pg, &id)
        .await
        .unwrap()
        .unwrap();
    f.finish().await;
    blocked.expect("guarded update must encounter the concurrent row lock");
    assert_eq!(retained.payload["apiKey"], "synthetic-concurrent");
    assert_eq!(
        result.unwrap_err().code.as_deref(),
        Some("provider_credential_snapshot_conflict")
    );
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn ordinary_update_retains_not_found_semantics() {
    let f = fixture::Fixture::new().await;
    let error =
        db::update_provider_credential(&f.pg, "synthetic-absent", input(&f.account, "synthetic"))
            .await
            .unwrap_err();
    f.finish().await;
    assert_eq!(error.http_status, Some(404));
    assert_eq!(error.code, None);
    assert_eq!(error.message, "Provider credential 不存在");
}
