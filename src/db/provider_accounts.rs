mod import_metadata;
mod input;
mod lookup;
mod mutations;
mod payload;
#[cfg(test)]
mod qwen_tests;
#[cfg(test)]
mod tests;
use crate::routing::candidate::ProviderExecutionMode;
pub(crate) use import_metadata::{
    list_provider_account_import_metadata, ProviderAccountFolderSyncMetadata,
};
pub(crate) use lookup::get_provider_account_for_folder_sync;
pub use lookup::{
    get_provider_account, list_provider_accounts, recover_expired_cooling_provider_accounts,
};
pub use mutations::{create_provider_account, delete_provider_account, update_provider_account};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::FromRow;
use std::collections::HashMap;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderAccountView {
    pub id: String,
    pub label: String,
    pub service_provider_key: String,
    pub service_provider_label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub protocol_profile: String,
    pub status: String,
    pub source_kind: Option<String>,
    pub aggregator_api_mode: Option<String>,
    pub web_reverse_access_mode: Option<String>,
    pub source_notes: Option<String>,
    pub execution_mode: ProviderExecutionMode,
    pub endpoint_execution_modes: Option<HashMap<String, ProviderExecutionMode>>,
    pub payload: Value,
    pub storage_mode: String,
    pub cooldown_until: Option<String>,
    pub last_error: Option<String>,
    pub failure_count: i32,
    pub last_health_check_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertProviderAccountInput {
    pub label: String,
    #[serde(default)]
    pub service_provider_key: Option<String>,
    #[serde(default)]
    pub service_provider_label: Option<String>,
    pub adapter: String,
    pub protocol_family: String,
    #[serde(default)]
    pub protocol_profile: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub aggregator_api_mode: Option<String>,
    #[serde(default)]
    pub web_reverse_access_mode: Option<String>,
    #[serde(default)]
    pub source_notes: Option<String>,
    #[serde(default)]
    pub execution_mode: Option<String>,
    #[serde(default)]
    pub endpoint_execution_modes: Option<HashMap<String, String>>,
    pub payload: Value,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProviderAccountRow {
    id: String,
    label: String,
    service_provider_key: String,
    service_provider_label: String,
    adapter: String,
    protocol_family: String,
    protocol_profile: String,
    status: String,
    source_kind: Option<String>,
    aggregator_api_mode: Option<String>,
    web_reverse_access_mode: Option<String>,
    source_notes: Option<String>,
    execution_mode: String,
    endpoint_execution_modes: Option<Json<Value>>,
    payload_inline: Option<Json<Value>>,
    payload_object_key: Option<String>,
    storage_mode: String,
    cooldown_until: Option<OffsetDateTime>,
    last_error: Option<String>,
    failure_count: i32,
    last_health_check_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}
