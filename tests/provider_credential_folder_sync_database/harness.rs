use super::fixture::Fixture;
use neuro_gateway::provider_credential_folder_sync::{
    run_folder_sync_once, FolderSyncDirection, ProviderCredentialFolderSyncStatusView,
};
use neuro_gateway::redis::keys;
use redis::AsyncCommands;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const MATERIAL_PATH: &str = "qwen-web-chat/fixture.json";

pub async fn setup(delete_missing: bool) -> Fixture {
    let mut fixture = Fixture::new().await;
    let config = &mut Arc::get_mut(&mut fixture.state).unwrap().config;
    config.provider_credential_folder_sync_import_enabled = true;
    config.provider_credential_folder_sync_export_enabled = true;
    config.provider_credential_folder_sync_delete_missing = delete_missing;
    sqlx::query(
        "update gateway_provider_accounts set label='Qwen Web Fixture',
         service_provider_key='qwen_platform', service_provider_label='Qwen Platform',
         adapter='qwen_web_compatible', protocol_family='qwen_web',
         protocol_profile='qwen_web_chat',
         payload_inline='{\"baseUrl\":\"https://example.invalid\",\"defaultModel\":\"qwen3-coder-plus\"}'
         where id=$1",
    )
    .bind(&fixture.account)
    .execute(&fixture.pg)
    .await
    .unwrap();
    clear_status(&fixture).await;
    fixture
}

async fn clear_status(fixture: &Fixture) {
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let _: usize = connection
        .del(keys::provider_credential_folder_sync_status_key())
        .await
        .unwrap();
}

pub async fn finish(fixture: Fixture) {
    clear_status(&fixture).await;
    fixture.finish().await;
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn write_material(fixture: &Fixture, token: &str) -> String {
    let path = fixture.root.join(MATERIAL_PATH);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "apiKey": token, "selectedModel": "qwen3-coder-plus"
    }))
    .unwrap();
    std::fs::write(path, &bytes).unwrap();
    hash(&bytes)
}

pub async fn row(fixture: &Fixture, id: &str) -> Value {
    sqlx::query_scalar::<_, sqlx::types::Json<Value>>(
        "select to_jsonb(c) from gateway_provider_credentials c where id=$1",
    )
    .bind(id)
    .fetch_one(&fixture.pg)
    .await
    .unwrap()
    .0
}

pub async fn run(
    fixture: &Fixture,
    direction: FolderSyncDirection,
    expected: (usize, usize, usize, usize, usize),
) -> ProviderCredentialFolderSyncStatusView {
    let status = run_folder_sync_once(&fixture.state, direction)
        .await
        .unwrap();
    assert_eq!(
        (
            status.imported_count,
            status.updated_count,
            status.exported_count,
            status.deleted_count,
            status.skipped_count
        ),
        expected
    );
    assert!(status.last_error.is_none());
    assert!(status.last_run_at.is_some());
    let mut connection = fixture.state.redis_pool.get().await.unwrap();
    let stored: String = connection
        .get(keys::provider_credential_folder_sync_status_key())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&stored).unwrap(),
        serde_json::to_value(&status).unwrap()
    );
    status
}

pub async fn remove_payload(fixture: &Fixture, id: &str, source_hash: &str) {
    sqlx::query(
        "update gateway_provider_credentials set payload_inline=null,
         source_hash=$2, sync_error='retained fixture error' where id=$1",
    )
    .bind(id)
    .bind(source_hash)
    .execute(&fixture.pg)
    .await
    .unwrap();
}
