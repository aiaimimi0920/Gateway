#![cfg(feature = "line-accio-web-reverse-api")]
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "provider_management_deletion/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "accio_legacy_db_reader_contract/fixture.rs"]
mod material;
#[path = "provider_credential_folder_sync_database/schema.rs"]
mod schema;
mod support;

use material::*;
use neuro_gateway::{
    db,
    provider_credential_folder_sync::{run_folder_sync_once, FolderSyncDirection},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn accio_same_hash_normal_readers_project_without_reimport_or_any_storage_write() {
    let (f, id) = setup().await;
    let before = row(&f, &id).await;
    let bytes = std::fs::read(f.root.join(PATH)).unwrap();
    for _ in 0..2 {
        let status = run_folder_sync_once(&f.state, FolderSyncDirection::Import)
            .await
            .unwrap();
        assert_eq!(
            (
                status.imported_count,
                status.updated_count,
                status.skipped_count
            ),
            (0, 0, 1)
        );
        assert_eq!(row(&f, &id).await, before);
        let view = read(&f, &id).await;
        projected(&view.payload);
        assert_eq!(view.failure_count, 7);
        assert_eq!(view.sync_error.as_deref(), Some("retained diagnostic"));
        assert_eq!(view.payload["rawSource"], legacy()["rawSource"]);
        for views in [
            db::list_provider_credentials(&f.pg, Some(&f.account))
                .await
                .unwrap(),
            db::list_active_provider_credentials_for_accounts(
                &f.pg,
                std::slice::from_ref(&f.account),
            )
            .await
            .unwrap(),
            db::list_active_provider_credentials_for_adapter(&f.pg, "accio_compatible", 10)
                .await
                .unwrap(),
        ] {
            assert_eq!(views.len(), 1);
            assert_eq!(views[0].id, id);
            assert_eq!(views[0].payload, view.payload);
        }
        assert_eq!(
            db::get_provider_credential_by_source_path(&f.pg, PATH)
                .await
                .unwrap()
                .unwrap()
                .payload,
            view.payload
        );
    }
    assert_eq!(std::fs::read(f.root.join(PATH)).unwrap(), bytes);
    let response = neuro_gateway::http::router::build_router(f.state.clone())
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/internal/gateway/provider-credentials")
                .header("x-management-token", "provider-deletion-contract-token")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let public = String::from_utf8(
        axum::body::to_bytes(response.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(!public.contains("synthetic opaque sg") && !public.contains("synthetic-refresh"));
    let public: Value = serde_json::from_str(&public).unwrap();
    assert_eq!(
        public["credentials"][0]["credential"]["accioSgcookie"],
        "syn*** sg"
    );
    assert_eq!(row(&f, &id).await, before);
    // A retained DB credential remains readable when its former source is no longer present.
    std::fs::remove_file(f.root.join(PATH)).unwrap();
    projected(&read(&f, &id).await.payload);
    assert_eq!(row(&f, &id).await, before);
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn accio_old_read_projection_cannot_overwrite_a_later_rotation_via_snapshot_cas() {
    let (f, id) = setup().await;
    let before = row(&f, &id).await;
    let old = read(&f, &id).await;
    projected(&old.payload);
    let mut rotated = legacy();
    rotated["apiKey"] = json!("synthetic-current-access");
    rotated["refreshToken"] = json!("synthetic-current-refresh");
    rotated["accioSgcookie"] = Value::Null;
    sqlx::query("update gateway_provider_credentials set payload_inline=$2 where id=$1")
        .bind(&id)
        .bind(sqlx::types::Json(rotated))
        .execute(&f.pg)
        .await
        .unwrap();
    let current = row(&f, &id).await;
    let rejected = db::provider_credentials::update_provider_credential_from_snapshot(
        &f.pg,
        &id,
        before["snapshot"].as_str().unwrap(),
        input(&f.account, old.payload),
    )
    .await
    .unwrap_err();
    assert_eq!(
        rejected.code.as_deref(),
        Some("provider_credential_snapshot_conflict")
    );
    let view = read(&f, &id).await;
    assert_eq!(view.payload["apiKey"], "synthetic-current-access");
    assert_eq!(view.payload["refreshToken"], "synthetic-current-refresh");
    assert!(view.payload["accioSgcookie"].is_null());
    assert!(view.payload.get("refreshClientId").is_none());
    assert_eq!(row(&f, &id).await, current);
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn accio_object_payload_is_projected_without_put_delete_or_storage_mode_changes() {
    assert_eq!(
        std::env::var("AI_GATEWAY_OBJECT_STORAGE_DRIVER").as_deref(),
        Ok("local")
    );
    let root =
        std::path::PathBuf::from(std::env::var("AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR").unwrap());
    assert!(root.starts_with(std::env::temp_dir()));
    let (f, id) = setup().await;
    let key = format!("accio-legacy/{id}.json");
    let storage = neuro_gateway::object_storage::gateway_object_storage().unwrap();
    let payload = legacy();
    storage.put_json(&key, &payload).await.unwrap();
    let object_path = root.join(&key);
    let object_bytes = std::fs::read(&object_path).unwrap();
    let modified = std::fs::metadata(&object_path).unwrap().modified().unwrap();
    sqlx::query("update gateway_provider_credentials set payload_inline=null,payload_object_key=$2,storage_mode='r2' where id=$1")
        .bind(&id).bind(&key).execute(&f.pg).await.unwrap();
    let before = row(&f, &id).await;
    for _ in 0..2 {
        let view = read(&f, &id).await;
        projected(&view.payload);
        assert_eq!(view.storage_mode, "r2");
        assert_eq!(storage.read_json(&key).await.unwrap(), payload);
        assert_eq!(std::fs::read(&object_path).unwrap(), object_bytes);
        assert_eq!(
            std::fs::metadata(&object_path).unwrap().modified().unwrap(),
            modified
        );
        assert_eq!(row(&f, &id).await, before);
        let account = db::get_provider_account(&f.pg, &f.account)
            .await
            .unwrap()
            .unwrap();
        let merged =
            db::merge_provider_account_and_credential_payloads(&account.payload, &view.payload);
        let mut merged = merged;
        merged["adapter"] = json!("accio_compatible");
        let target = serde_json::from_value(merged).unwrap();
        let request=neuro_gateway::protocol::openai::normalize_chat_completions(json!({"model":"exact-model","messages":[{"role":"user","content":"synthetic legacy probe"}]})).unwrap();
        let plan = neuro_gateway::upstream::accio::build_request_plan(
            &target,
            &request,
            "exact-model",
            true,
        )
        .unwrap();
        let body = plan.body.unwrap();
        assert_eq!(body["token"], "synthetic-access");
        assert_eq!(body["utdid"], "synthetic-device");
        assert_eq!(body["model"], "exact-model");
        assert!(
            !body.to_string().contains("synthetic opaque sg")
                && !body.to_string().contains("synthetic-refresh")
        );
    }
    f.finish().await;
    storage.delete_object(&key).await.unwrap();
}
