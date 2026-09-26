//! Import bookkeeping never needs existing credential payloads or object storage.
use crate::db::map_db_error;
use crate::error::GatewayError;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;

#[derive(Debug, FromRow)]
struct ProviderCredentialImportBounds {
    row_count: i64,
    source_path_bytes: i64,
}

#[derive(Debug, FromRow)]
pub(crate) struct ProviderCredentialImportMetadata {
    pub id: String,
    pub label: String,
    pub status: String,
    pub source_kind: String,
    pub source_path: Option<String>,
    pub source_hash: Option<String>,
    pub sync_mode: String,
    pub sync_state: String,
    pub archived_at: Option<OffsetDateTime>,
}

pub(crate) async fn list_provider_credential_import_metadata(
    pool: &PgPool,
    max_rows: usize,
    max_source_path_bytes: usize,
) -> Result<Vec<ProviderCredentialImportMetadata>, GatewayError> {
    // Retain the complete deletion snapshot and existing duplicate-path ordering.
    let limit = max_rows.saturating_add(1) as i64;
    let bounds = sqlx::query_as::<_, ProviderCredentialImportBounds>(
        r#"
        select count(*) as row_count,
               coalesce(sum(octet_length(source_path)), 0)::bigint as source_path_bytes
        from (
            select source_path
            from gateway_provider_credentials
            order by created_at asc
            limit $1
        ) bounded
        "#,
    )
    .bind(limit)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    if bounds.row_count as usize > max_rows {
        return Err(GatewayError::bad_request(format!(
            "provider credential folder database exceeds credential metadata row limit {max_rows}"
        ))
        .with_code("provider_credential_folder_sync_database_limit"));
    }
    if bounds.source_path_bytes as usize > max_source_path_bytes {
        return Err(GatewayError::bad_request(format!(
            "provider credential folder database exceeds source path byte limit {max_source_path_bytes}"
        ))
        .with_code("provider_credential_folder_sync_source_path_limit"));
    }

    let rows = sqlx::query_as::<_, ProviderCredentialImportMetadata>(
        r#"
        select id, label, status, source_kind, source_path, source_hash,
               sync_mode, sync_state, archived_at
        from gateway_provider_credentials
        order by created_at asc
        limit $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    if rows.len() > max_rows {
        return Err(GatewayError::bad_request(format!(
            "provider credential folder database exceeds credential metadata row limit {max_rows}"
        ))
        .with_code("provider_credential_folder_sync_database_limit"));
    }
    let source_path_bytes = rows.iter().fold(0usize, |bytes, row| {
        bytes.saturating_add(row.source_path.as_deref().map_or(0, str::len))
    });
    if source_path_bytes > max_source_path_bytes {
        return Err(GatewayError::bad_request(format!(
            "provider credential folder database exceeds source path byte limit {max_source_path_bytes}"
        ))
        .with_code("provider_credential_folder_sync_source_path_limit"));
    }
    Ok(rows)
}
