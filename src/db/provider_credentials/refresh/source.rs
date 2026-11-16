//! Database locks bind control state and object reads; no xmin is retained as grant authority.
use super::snapshot::{from_rows, invalid, Snapshot};
use crate::{db::map_db_error, error::GatewayError, object_storage::gateway_object_storage};
use serde_json::Value;
use sqlx::{types::Json, Postgres, Transaction};

pub(super) async fn load_locked(
    transaction: &mut Transaction<'_, Postgres>,
    credential_id: &str,
) -> Result<Option<Snapshot>, GatewayError> {
    // Account first, then credential: account deletion uses that same dependency order.
    let account: Option<Json<Value>> = sqlx::query_scalar(
        "select case when coalesce(octet_length(a.payload_inline::text),0)<=4194304 then to_jsonb(a) else null end from gateway_provider_accounts a join gateway_provider_credentials c on c.provider_account_id=a.id where c.id=$1 for share of a",
    )
    .bind(credential_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_db_error)?;
    let Some(account) = account else {
        return Ok(None);
    };
    let credential: Option<Json<Value>> = sqlx::query_scalar(
        "select case when coalesce(octet_length(c.payload_inline::text),0)<=4194304 then to_jsonb(c) else null end from gateway_provider_credentials c where id=$1 for update",
    )
    .bind(credential_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(map_db_error)?;
    let Some(credential) = credential else {
        return Ok(None);
    };
    if account.0["status"] != "active"
        || account.0["adapter"] != "accio_compatible"
        || credential.0["status"] != "active"
        || !credential.0["archived_at"].is_null()
        || credential.0["provider_account_id"] != account.0["id"]
    {
        return Ok(None);
    }
    let account_payload = row_payload(&account.0).await?;
    let credential_payload = super::super::payload::project_imported_payload(
        row_payload(&credential.0).await?,
        credential.0["source_kind"].as_str().ok_or_else(invalid)?,
        credential.0["sync_mode"].as_str().ok_or_else(invalid)?,
    )?;
    from_rows(
        &account.0,
        &credential.0,
        account_payload,
        credential_payload,
    )
    .map(Some)
}

async fn row_payload(row: &Value) -> Result<Value, GatewayError> {
    let payload = if !row["payload_inline"].is_null() {
        row["payload_inline"].clone()
    } else {
        gateway_object_storage()?
            .read_json(row["payload_object_key"].as_str().ok_or_else(invalid)?)
            .await?
    };
    if !payload.is_object()
        || serde_json::to_vec(&payload).map_err(|_| invalid())?.len() > 4_194_304
    {
        return Err(invalid());
    }
    Ok(payload)
}
