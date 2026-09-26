//! Folder-sync account projection; never hydrates account payload storage.

use crate::db::map_db_error;
use crate::error::GatewayError;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ProviderAccountFolderSyncMetadata {
    pub id: String,
    pub label: String,
    pub service_provider_key: String,
    pub service_provider_label: String,
    pub adapter: String,
    pub protocol_family: String,
    pub protocol_profile: String,
    pub source_kind: Option<String>,
    pub web_reverse_access_mode: Option<String>,
    pub payload_base_url: Option<String>,
}

pub(crate) async fn list_provider_account_import_metadata(
    pool: &PgPool,
    max_rows: usize,
) -> Result<Vec<ProviderAccountFolderSyncMetadata>, GatewayError> {
    let rows = sqlx::query_as::<_, ProviderAccountFolderSyncMetadata>(
        r#"
        select id, label, service_provider_key, service_provider_label,
               adapter, protocol_family, protocol_profile, source_kind,
               web_reverse_access_mode,
               coalesce(payload_inline->>'baseUrl', payload_inline->>'base_url')
                 as payload_base_url
        from gateway_provider_accounts
        order by created_at asc
        limit $1
        "#,
    )
    .bind(max_rows.saturating_add(1) as i64)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    if rows.len() > max_rows {
        return Err(GatewayError::bad_request(format!(
            "provider credential folder database exceeds provider account row limit {max_rows}"
        ))
        .with_code("provider_credential_folder_sync_database_limit"));
    }
    Ok(rows)
}
