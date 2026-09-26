mod import_metadata;
mod input;
mod lookup;
mod mutations;
mod payload;
mod state;
#[cfg(test)]
mod tests;
pub(crate) use import_metadata::{
    list_provider_credential_import_metadata, ProviderCredentialImportMetadata,
};
pub use lookup::{
    get_provider_credential, get_provider_credential_by_source_path,
    list_active_provider_credential_refs_for_accounts,
    list_active_provider_credentials_for_accounts, list_active_provider_credentials_for_adapter,
    list_expired_cooling_provider_credential_ids, list_provider_credentials,
};
pub use mutations::{
    create_provider_credential, delete_provider_credential, update_provider_credential,
};
pub use payload::merge_provider_account_and_credential_payloads;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::FromRow;
pub use state::{
    mark_provider_credential_probe_failure, mark_provider_credential_probe_success,
    note_provider_credential_runtime_failure, note_provider_credential_runtime_success,
    update_provider_credential_sync_metadata, update_provider_credential_sync_state,
};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderCredentialView {
    pub id: String,
    pub provider_account_id: String,
    pub label: String,
    pub status: String,
    pub payload: Value,
    pub storage_mode: String,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub source_hash: Option<String>,
    pub sync_mode: String,
    pub sync_state: String,
    pub sync_error: Option<String>,
    pub cooldown_until: Option<String>,
    pub last_error: Option<String>,
    pub failure_count: i32,
    pub last_health_check_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GatewayProviderCredentialRef {
    pub id: String,
    pub provider_account_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertProviderCredentialInput {
    pub provider_account_id: String,
    pub label: String,
    #[serde(default)]
    pub status: Option<String>,
    pub payload: Value,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_hash: Option<String>,
    #[serde(default)]
    pub sync_mode: Option<String>,
    #[serde(default)]
    pub sync_state: Option<String>,
    #[serde(default)]
    pub sync_error: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProviderCredentialRow {
    id: String,
    provider_account_id: String,
    label: String,
    status: String,
    payload_inline: Option<Json<Value>>,
    payload_object_key: Option<String>,
    storage_mode: String,
    source_kind: String,
    source_path: Option<String>,
    source_hash: Option<String>,
    sync_mode: String,
    sync_state: String,
    sync_error: Option<String>,
    cooldown_until: Option<OffsetDateTime>,
    last_error: Option<String>,
    failure_count: i32,
    last_health_check_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    archived_at: Option<OffsetDateTime>,
}
