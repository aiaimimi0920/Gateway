//! Provider credential management routes with domain-owned implementations.

mod credential_payload;
mod display_metadata;
mod folder_sync;
mod listing;
mod mutations;
mod quota;
mod route_config;
mod views;

#[cfg(test)]
mod metadata_contract_tests;

#[cfg(test)]
mod tests;

pub use listing::{
    get_provider_credential, list_provider_credentials, list_provider_credentials_for_account,
    ProviderCredentialListQuery,
};

pub use mutations::{
    create_provider_credential, create_provider_credential_for_account, delete_provider_credential,
    update_provider_credential, ProviderCredentialCreateBody, ProviderCredentialUpdateBody,
};

pub use folder_sync::{
    export_provider_credentials_to_folder, get_provider_credential_folder_sync_status,
    import_provider_credentials_from_folder, update_provider_credential_folder_sync_status,
    ProviderCredentialFolderSyncStatusUpdateBody,
};

pub use quota::{get_provider_credential_quota, refresh_provider_credential_quota};

use crate::error::GatewayError;
use crate::state::AppState;

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}
