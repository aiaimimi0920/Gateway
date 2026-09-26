#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "provider_management_deletion/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "provider_credential_folder_sync_database/harness.rs"]
mod harness;
#[path = "provider_credential_folder_sync_database/schema.rs"]
mod schema;
mod support;

use harness::*;
use neuro_gateway::db;
use neuro_gateway::error::GatewayError;
use neuro_gateway::provider_credential_folder_sync::{run_folder_sync_once, FolderSyncDirection};
use serde_json::{json, Value};
use std::sync::Arc;

const PROVIDER_ACCOUNT_ROW_LIMIT: i32 = 4_096;
const PROVIDER_CREDENTIAL_METADATA_ROW_LIMIT: i32 = 100_000;
const SOURCE_PATH_RETAINED_BYTE_LIMIT: i64 = 32 * 1024 * 1024;

async fn import_error(fixture: &fixture::Fixture) -> GatewayError {
    run_folder_sync_once(&fixture.state, FolderSyncDirection::Import)
        .await
        .unwrap_err()
}

fn enable_delete_missing(fixture: &mut fixture::Fixture) {
    Arc::get_mut(&mut fixture.state)
        .expect("fixture state must be uniquely owned")
        .config
        .provider_credential_folder_sync_delete_missing = true;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn import_create_update_skip_and_export_persist_exact_state() {
    let fixture = setup(false).await;
    let first_hash = write_material(&fixture, "synthetic-first");
    let imported = run(&fixture, FolderSyncDirection::Import, (1, 0, 0, 0, 0)).await;
    assert_eq!(imported.last_import_at, imported.last_run_at);
    assert!(imported.last_export_at.is_none());
    let credentials = db::list_provider_credentials(&fixture.pg, None)
        .await
        .unwrap();
    assert_eq!(credentials.len(), 1);
    let id = &credentials[0].id;
    let created = row(&fixture, id).await;
    assert_eq!(created["provider_account_id"], fixture.account);
    assert_eq!(created["label"], "fixture");
    assert_eq!(created["status"], "active");
    assert_eq!(created["source_path"], MATERIAL_PATH);
    assert_eq!(created["source_hash"], first_hash);
    assert_eq!(created["source_kind"], "folder_sync_import");
    assert_eq!(created["sync_mode"], "folder_sync");
    assert_eq!(created["sync_state"], "imported");
    assert_eq!(created["payload_content_type"], "application/json");
    assert!(created["sync_error"].is_null());
    run(&fixture, FolderSyncDirection::Import, (0, 0, 0, 0, 1)).await;
    assert_eq!(
        row(&fixture, id).await,
        created,
        "same hash must not mutate the row"
    );
    sqlx::query("update gateway_provider_credentials set label='Retained label', status='disabled', sync_error='old error' where id=$1")
        .bind(id).execute(&fixture.pg).await.unwrap();
    let changed_hash = write_material(&fixture, "synthetic-updated");
    run(&fixture, FolderSyncDirection::Import, (0, 1, 0, 0, 0)).await;
    let updated = row(&fixture, id).await;
    assert_eq!(updated["label"], "Retained label");
    assert_eq!(updated["status"], "disabled");
    assert_eq!(updated["source_hash"], changed_hash);
    assert!(updated["sync_error"].is_null());
    assert_eq!(updated["payload_inline"]["apiKey"], "synthetic-updated");
    assert_eq!(
        updated["payload_inline"]["supportedModels"],
        json!(["qwen3-coder-plus"])
    );
    std::fs::remove_file(fixture.root.join(MATERIAL_PATH)).unwrap();
    let exported = run(&fixture, FolderSyncDirection::Export, (0, 0, 1, 0, 0)).await;
    assert_eq!(exported.last_export_at, exported.last_run_at);
    let bytes = std::fs::read(fixture.root.join(MATERIAL_PATH)).unwrap();
    let payload: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(payload, updated["payload_inline"]);
    assert_eq!(payload["serviceProviderKey"], "qwen_platform");
    assert_eq!(payload["providerSurfaceKey"], "qwen-web-chat");
    assert_eq!(payload["credentialMaterialKind"], "session_auth");
    let exported_row = row(&fixture, id).await;
    assert_eq!(exported_row["source_hash"], hash(&bytes));
    assert_eq!(exported_row["sync_state"], "exported");
    assert_eq!(exported_row["payload_inline"], updated["payload_inline"]);
    run(&fixture, FolderSyncDirection::Export, (0, 0, 0, 0, 1)).await;
    assert_eq!(
        std::fs::read(fixture.root.join(MATERIAL_PATH)).unwrap(),
        bytes
    );
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn unrelated_missing_payload_does_not_block_import() {
    let mut fixture = setup(false).await;
    let unrelated = fixture.credential("other/missing.json", "manual").await;
    remove_payload(&fixture, &unrelated, "unrelated-hash").await;
    let before = row(&fixture, &unrelated).await;
    write_material(&fixture, "synthetic-new");
    run(&fixture, FolderSyncDirection::Import, (1, 0, 0, 0, 0)).await;
    assert_eq!(row(&fixture, &unrelated).await, before);
    let count: i64 = sqlx::query_scalar("select count(*) from gateway_provider_credentials")
        .fetch_one(&fixture.pg)
        .await
        .unwrap();
    assert_eq!(count, 2);
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn same_hash_skips_without_hydrating_or_repairing_payload() {
    let mut fixture = setup(false).await;
    let source_hash = write_material(&fixture, "synthetic-skip");
    let id = fixture.credential(MATERIAL_PATH, "folder_sync").await;
    remove_payload(&fixture, &id, &source_hash).await;
    let before = row(&fixture, &id).await;
    run(&fixture, FolderSyncDirection::Import, (0, 0, 0, 0, 1)).await;
    assert_eq!(row(&fixture, &id).await, before);
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn same_hash_skips_without_hydrating_selected_account_payload() {
    let mut fixture = setup(false).await;
    let source_hash = write_material(&fixture, "synthetic-account-skip");
    let id = fixture.credential(MATERIAL_PATH, "folder_sync").await;
    sqlx::query("update gateway_provider_credentials set source_hash=$2 where id=$1")
        .bind(&id)
        .bind(&source_hash)
        .execute(&fixture.pg)
        .await
        .unwrap();
    sqlx::query(
        "update gateway_provider_accounts set payload_inline=null, payload_object_key=null where id=$1",
    )
    .bind(&fixture.account)
    .execute(&fixture.pg)
    .await
    .unwrap();
    let before = row(&fixture, &id).await;
    run(&fixture, FolderSyncDirection::Import, (0, 0, 0, 0, 1)).await;
    assert_eq!(row(&fixture, &id).await, before);
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn unselected_object_storage_account_is_not_hydrated() {
    let fixture = setup(false).await;
    sqlx::query(
        "insert into gateway_provider_accounts
         (id,label,service_provider_key,service_provider_label,adapter,protocol_family,
          protocol_profile,payload_inline,payload_object_key,storage_mode)
         values ('unselected-object-account','Unselected Object Account','other_platform',
                 'Other Platform','openai_compatible','openai','other_surface',null,
                 'missing/unselected-object-account.json','r2')",
    )
    .execute(&fixture.pg)
    .await
    .unwrap();
    write_material(&fixture, "synthetic-unselected-object");
    run(&fixture, FolderSyncDirection::Import, (1, 0, 0, 0, 0)).await;
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn selected_account_uses_scalar_classification_then_requires_full_payload() {
    let fixture = setup(false).await;
    sqlx::query(
        "update gateway_provider_accounts set payload_inline=null, payload_object_key=null where id=$1",
    )
    .bind(&fixture.account)
    .execute(&fixture.pg)
    .await
    .unwrap();
    write_material(&fixture, "synthetic-selected-missing");
    let error = import_error(&fixture).await;
    assert_eq!(error.http_status, Some(409));
    assert_eq!(error.code.as_deref(), Some("conflict"));
    assert_eq!(error.message, "Provider account payload 缺失");
    let count: i64 = sqlx::query_scalar("select count(*) from gateway_provider_credentials")
        .fetch_one(&fixture.pg)
        .await
        .unwrap();
    assert_eq!(count, 0);
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn oversized_inline_account_fails_before_credential_mutation_or_deletion() {
    let mut fixture = setup(true).await;
    let retained = fixture
        .credential("qwen-web-chat/retained.json", "folder_sync")
        .await;
    sqlx::query(
        "update gateway_provider_credentials
         set source_kind='folder_sync_import', sync_state='imported' where id=$1",
    )
    .bind(&retained)
    .execute(&fixture.pg)
    .await
    .unwrap();
    sqlx::query(
        "update gateway_provider_accounts
         set payload_inline=jsonb_build_object(
             'baseUrl', 'https://example.invalid',
             'oversized', repeat('x', $2::int))
         where id=$1",
    )
    .bind(&fixture.account)
    .bind((32 * 1024 * 1024 + 4096) as i32)
    .execute(&fixture.pg)
    .await
    .unwrap();
    write_material(&fixture, "synthetic-oversized-account");

    let error = import_error(&fixture).await;
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_account_payload_too_large")
    );
    assert_eq!(
        error.message,
        "provider credential folder account payload exceeds 33554432 byte limit"
    );
    assert!(fixture.credential_exists(&retained).await);
    assert_eq!(
        fixture
            .key_count(&fixture::Fixture::credential_keys(&retained))
            .await,
        4
    );
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn provider_account_limit_accepts_exact_boundary_and_stops_before_deletion() {
    let mut fixture = setup(false).await;
    let retained = fixture
        .credential("qwen-web-chat/retained.json", "folder_sync")
        .await;
    sqlx::query(
        "update gateway_provider_credentials set source_kind='folder_sync_import' where id=$1",
    )
    .bind(&retained)
    .execute(&fixture.pg)
    .await
    .unwrap();
    sqlx::query(
        "insert into gateway_provider_accounts (id,label,created_at)
         select 'account-limit-' || value, 'Account Limit ' || value,
                now() + value * interval '1 microsecond'
         from generate_series(1, $1) as value",
    )
    .bind(PROVIDER_ACCOUNT_ROW_LIMIT - 1)
    .execute(&fixture.pg)
    .await
    .unwrap();
    run(&fixture, FolderSyncDirection::Import, (0, 0, 0, 0, 0)).await;

    enable_delete_missing(&mut fixture);
    sqlx::query("insert into gateway_provider_accounts (id,label) values ('account-limit-overflow','Overflow')")
        .execute(&fixture.pg).await.unwrap();
    let error = import_error(&fixture).await;
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_database_limit")
    );
    assert_eq!(
        error.message,
        "provider credential folder database exceeds provider account row limit 4096"
    );
    assert!(fixture.credential_exists(&retained).await);
    assert_eq!(
        fixture
            .key_count(&fixture::Fixture::credential_keys(&retained))
            .await,
        4
    );
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn credential_metadata_limit_accepts_exact_boundary_and_stops_before_deletion() {
    let mut fixture = setup(false).await;
    let retained = fixture
        .credential("qwen-web-chat/retained.json", "folder_sync")
        .await;
    sqlx::query(
        "update gateway_provider_credentials set source_kind='folder_sync_import' where id=$1",
    )
    .bind(&retained)
    .execute(&fixture.pg)
    .await
    .unwrap();
    sqlx::query(
        "insert into gateway_provider_credentials
         (id,provider_account_id,label,source_kind,sync_mode,created_at)
         select 'credential-limit-' || value, $1, 'Credential Limit ' || value,
                'manual', 'manual', now() + value * interval '1 microsecond'
         from generate_series(1, $2) as value",
    )
    .bind(&fixture.account)
    .bind(PROVIDER_CREDENTIAL_METADATA_ROW_LIMIT - 1)
    .execute(&fixture.pg)
    .await
    .unwrap();
    run(&fixture, FolderSyncDirection::Import, (0, 0, 0, 0, 0)).await;

    enable_delete_missing(&mut fixture);
    sqlx::query(
        "insert into gateway_provider_credentials
         (id,provider_account_id,label,source_kind,sync_mode)
         values ('credential-limit-overflow',$1,'Overflow','manual','manual')",
    )
    .bind(&fixture.account)
    .execute(&fixture.pg)
    .await
    .unwrap();
    let error = import_error(&fixture).await;
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_database_limit")
    );
    assert_eq!(
        error.message,
        "provider credential folder database exceeds credential metadata row limit 100000"
    );
    assert!(fixture.credential_exists(&retained).await);
    assert_eq!(
        fixture
            .key_count(&fixture::Fixture::credential_keys(&retained))
            .await,
        4
    );
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn source_path_byte_limit_stops_before_deletion() {
    let mut fixture = setup(false).await;
    let retained = fixture
        .credential("qwen-web-chat/retained.json", "folder_sync")
        .await;
    sqlx::query(
        "update gateway_provider_credentials set source_kind='folder_sync_import' where id=$1",
    )
    .bind(&retained)
    .execute(&fixture.pg)
    .await
    .unwrap();
    sqlx::query(
        "insert into gateway_provider_credentials
         (id,provider_account_id,label,source_kind,sync_mode,source_path,created_at)
         select 'credential-path-limit-' || value, $1, 'Credential Path Limit ' || value,
                'manual', 'manual', repeat('x', 512),
                now() + value * interval '1 microsecond'
         from generate_series(1, $2) as value",
    )
    .bind(&fixture.account)
    .bind(SOURCE_PATH_RETAINED_BYTE_LIMIT / 512 + 1)
    .execute(&fixture.pg)
    .await
    .unwrap();

    enable_delete_missing(&mut fixture);
    let error = import_error(&fixture).await;
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_source_path_limit")
    );
    assert_eq!(
        error.message,
        "provider credential folder database exceeds source path byte limit 33554432"
    );
    assert!(fixture.credential_exists(&retained).await);
    assert_eq!(
        fixture
            .key_count(&fixture::Fixture::credential_keys(&retained))
            .await,
        4
    );
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn changed_file_repairs_missing_old_payload_with_existing_identity() {
    let mut fixture = setup(false).await;
    let id = fixture.credential(MATERIAL_PATH, "folder_sync").await;
    remove_payload(&fixture, &id, "old-hash").await;
    let source_hash = write_material(&fixture, "synthetic-repair");
    run(&fixture, FolderSyncDirection::Import, (0, 1, 0, 0, 0)).await;
    let repaired = row(&fixture, &id).await;
    assert_eq!(repaired["label"], "Fixture credential");
    assert_eq!(repaired["source_hash"], source_hash);
    assert_eq!(repaired["payload_inline"]["apiKey"], "synthetic-repair");
    assert_eq!(repaired["sync_state"], "imported");
    assert!(repaired["sync_error"].is_null());
    assert!(db::get_provider_credential(&fixture.pg, &id).await.is_ok());
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn duplicate_source_path_updates_last_created_row_only() {
    let mut fixture = setup(false).await;
    let older = fixture.credential(MATERIAL_PATH, "folder_sync").await;
    let newer = fixture.credential(MATERIAL_PATH, "folder_sync").await;
    sqlx::query("update gateway_provider_credentials set created_at=case when id=$1 then '2026-01-01'::timestamptz else '2026-01-02'::timestamptz end, label=case when id=$1 then 'Older' else 'Newer' end")
        .bind(&older).execute(&fixture.pg).await.unwrap();
    let original = row(&fixture, &older).await;
    let source_hash = write_material(&fixture, "synthetic-duplicate");
    run(&fixture, FolderSyncDirection::Import, (0, 1, 0, 0, 0)).await;
    assert_eq!(row(&fixture, &older).await, original);
    let selected = row(&fixture, &newer).await;
    assert_eq!(selected["label"], "Newer");
    assert_eq!(selected["source_hash"], source_hash);
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn missing_file_deletion_uses_metadata_and_preserves_ineligible_rows() {
    let mut fixture = setup(true).await;
    let eligible = fixture
        .credential("qwen-web-chat/gone.json", "folder_sync")
        .await;
    remove_payload(&fixture, &eligible, "gone-hash").await;
    sqlx::query(
        "update gateway_provider_credentials set source_kind='folder_sync_import' where id=$1",
    )
    .bind(&eligible)
    .execute(&fixture.pg)
    .await
    .unwrap();
    let pending = fixture
        .credential("qwen-web-chat/pending.json", "folder_sync")
        .await;
    let manual = fixture
        .credential("qwen-web-chat/manual.json", "manual")
        .await;
    let archived = fixture
        .credential("qwen-web-chat/archived.json", "folder_sync")
        .await;
    sqlx::query("update gateway_provider_credentials set source_kind='folder_sync_import', archived_at=now() where id=$1")
        .bind(&archived).execute(&fixture.pg).await.unwrap();
    let status_archived = fixture
        .credential("qwen-web-chat/status-archived.json", "folder_sync")
        .await;
    sqlx::query("update gateway_provider_credentials set source_kind='folder_sync_import', status='archived' where id=$1")
        .bind(&status_archived).execute(&fixture.pg).await.unwrap();
    run(&fixture, FolderSyncDirection::Import, (0, 0, 0, 1, 0)).await;
    assert!(!fixture.credential_exists(&eligible).await);
    assert_eq!(
        fixture
            .key_count(&fixture::Fixture::credential_keys(&eligible))
            .await,
        0
    );
    for id in [pending, manual, archived, status_archived] {
        assert!(fixture.credential_exists(&id).await);
        assert_eq!(
            fixture
                .key_count(&fixture::Fixture::credential_keys(&id))
                .await,
            4
        );
    }
    finish(fixture).await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn public_full_lookup_and_export_still_require_real_payload() {
    let mut fixture = setup(false).await;
    let id = fixture.credential(MATERIAL_PATH, "folder_sync").await;
    remove_payload(&fixture, &id, "missing-hash").await;
    write_material(&fixture, "synthetic-preserved");
    let original = std::fs::read(fixture.root.join(MATERIAL_PATH)).unwrap();
    assert!(db::get_provider_credential(&fixture.pg, &id).await.is_err());
    assert!(db::list_provider_credentials(&fixture.pg, None)
        .await
        .is_err());
    assert!(
        run_folder_sync_once(&fixture.state, FolderSyncDirection::Export)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(fixture.root.join(MATERIAL_PATH)).unwrap(),
        original
    );
    finish(fixture).await;
}
