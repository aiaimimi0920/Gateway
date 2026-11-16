//! Publish one acknowledged result under its original DB authority without overwriting old objects.
use super::{
    journal::load_record,
    snapshot::{invalid, Snapshot},
    source::load_locked,
};
use crate::{
    console::refresh_grants::{GrantPhase, GrantRecord, GrantResult},
    db::map_db_error,
    error::GatewayError,
    object_storage::gateway_object_storage,
};
use serde_json::{json, Value};
use sqlx::{types::Json, PgPool};

pub(crate) async fn publish_result(
    pool: &PgPool,
    accepted: &GrantRecord,
) -> Result<Option<Snapshot>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let Some(snapshot) = load_locked(&mut tx, accepted.credential_id()).await? else {
        return Ok(None);
    };
    let family = accepted.family_id();
    let material = accepted.material_id();
    let head: Option<String> = sqlx::query_scalar(
        "select material from gateway_provider_refresh_heads where family=$1 for update",
    )
    .bind(&family)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    if head.as_deref() != Some(&material) {
        return Ok(None);
    }
    let (current, published_authority) = load_record(&mut tx, &family, &material)
        .await?
        .ok_or_else(invalid)?;
    if !current.same_attempt(accepted) || current.result() != accepted.result() {
        return Err(invalid());
    }
    if current.phase() == GrantPhase::Published {
        // A recreated or later-written credential must not impersonate the publication receipt.
        if published_authority.as_deref() != Some(snapshot.request.revision.as_str()) {
            return Ok(None);
        }
        tx.commit().await.map_err(map_db_error)?;
        return Ok(Some(snapshot));
    }
    if current.phase() != GrantPhase::Result || !current.matches_request(&snapshot.request) {
        return Ok(None);
    }
    let payload = rotated_payload(&snapshot, current.result().ok_or_else(invalid)?)?;
    let (inline, key) = if snapshot.storage_mode == "inline" {
        (Some(Json(payload)), None)
    } else {
        let key = staged_object_key(&current)?;
        // Never reuse/delete the old key. A failed transaction leaves a known staged object,
        // while the old DB pointer and its bytes remain valid for ordinary readers/rollback.
        gateway_object_storage()?.put_json(&key, &payload).await?;
        (None, Some(key))
    };
    sqlx::query("update gateway_provider_credentials set payload_inline=$2,payload_object_key=$3,payload_content_type='application/json',updated_at=$4 where id=$1")
        .bind(current.credential_id()).bind(inline).bind(key)
        .bind(time::OffsetDateTime::now_utc()).execute(&mut *tx).await.map_err(map_db_error)?;
    let successor = load_locked(&mut tx, current.credential_id())
        .await?
        .ok_or_else(invalid)?;
    let published = current
        .published_for_database(&snapshot.request, &successor.request)
        .map_err(|_| invalid())?;
    let changed = sqlx::query("update gateway_provider_refresh_grants set record=$3,published_authority=$4 where family=$1 and material=$2 and record=$5")
        .bind(&family).bind(&material).bind(published.to_json().map_err(|_| invalid())?)
        .bind(&successor.request.revision).bind(current.to_json().map_err(|_| invalid())?)
        .execute(&mut *tx).await.map_err(map_db_error)?.rows_affected();
    if changed != 1 {
        return Err(invalid());
    }
    // Credential pointer, successor authority receipt, and Published record commit together.
    tx.commit().await.map_err(map_db_error)?;
    Ok(Some(successor))
}

pub(super) fn staged_object_key(record: &GrantRecord) -> Result<String, GatewayError> {
    Ok(format!(
        "ai-gateway/provider-credential-refresh/{}/{}/{}.json",
        record.family_id(),
        record.material_id(),
        record.identity_digest().map_err(|_| invalid())?
    ))
}

fn rotated_payload(snapshot: &Snapshot, result: &GrantResult) -> Result<Value, GatewayError> {
    result
        .validate(&snapshot.request.material)
        .map_err(|_| invalid())?;
    let mut payload = snapshot.payload.clone();
    let object = payload.as_object_mut().ok_or_else(invalid)?;
    // The parser has established equivalence of these known copies. Keep rawSource as
    // historical input, not live grant authority; no unrelated policy/identity is replaced.
    let names = [
        "token",
        "accessToken",
        "access_token",
        "apiKey",
        "api_key",
        "refreshToken",
        "refresh_token",
        "accioSgcookie",
        "accio_sgcookie",
        "sgcookie",
        "expiresAt",
        "expires_at",
        "refreshEndpoint",
        "refresh_endpoint",
        "refreshClientId",
        "refresh_client_id",
        "clientId",
        "client_id",
    ];
    for key in names {
        if key != "token" {
            object.remove(key);
        }
    }
    for extra in ["extraBody", "extra_body"] {
        if let Some(extra) = object.get_mut(extra).and_then(Value::as_object_mut) {
            for key in names {
                extra.remove(key);
            }
        }
    }
    for (key, value) in [
        ("apiKey", json!(result.material.access)),
        ("refreshToken", json!(result.material.refresh)),
        ("refreshEndpoint", json!(result.material.endpoint)),
        ("refreshClientId", json!(result.material.client)),
        ("expiresAt", json!(result.expires_at)),
    ] {
        object.insert(key.into(), value);
    }
    if let Some(cookie) = &result.material.accio_sgcookie {
        object.insert("accioSgcookie".into(), json!(cookie));
    }
    Ok(payload)
}
