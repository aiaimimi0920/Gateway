//! SQLite reset/undo use the original health rows and advance their CAS payload, including clock rollback.
use super::*;
use crate::db::provider_credential_model_states::control::{clear, HealthRow};

fn encode(
    mut row: GatewayProviderCredentialModelStateView,
    previous: &str,
) -> Result<String, GatewayError> {
    let at = OffsetDateTime::parse(previous, &Rfc3339)
        .ok()
        .and_then(|at| at.checked_add(time::Duration::nanoseconds(1)))
        .ok_or_else(|| GatewayError::server_error("Cannot advance model health timestamp"))?
        .max(OffsetDateTime::now_utc());
    row.updated_at = at
        .format(&Rfc3339)
        .map_err(|_| GatewayError::server_error("Cannot format health timestamp"))?;
    serde_json::to_string(&row)
        .map_err(|_| GatewayError::server_error("Cannot encode health checkpoint"))
}

impl LocalRuntime {
    pub(crate) async fn record_window_failure(
        &self,
        input: RecordCredentialModelFailureInput,
        before: Option<&HealthRow>,
    ) -> Result<Option<HealthRow>, GatewayError> {
        if before.is_some_and(|row| row.row.status == "blocked")
            || !transition_for_failure_with_retry_after(
                &input.classification,
                input.upstream_retry_after_ms,
            )
            .penalizes_candidate
        {
            return Ok(None);
        }
        let expected = ModelHealthSnapshot {
            identity: RecordCredentialModelSuccessInput {
                provider_account_id: input.provider_account_id.clone(),
                provider_credential_id: input.provider_credential_id.clone(),
                provider_credential_ref: input.provider_credential_ref.clone(),
                protocol_profile: input.protocol_profile.clone(),
                model: input.model.clone(),
            },
            version: before.map(|row| row.version.clone()),
            protected: false,
        };
        // Checkpoint capture and the normal writer are joined by the existing expected-payload CAS.
        self.update_model(expected.identity.clone(), Some(input), Some(&expected))
            .await?
            .map(|row| {
                let version = serde_json::to_string(&row)
                    .map_err(|_| GatewayError::server_error("Cannot encode health checkpoint"))?;
                Ok(HealthRow { row, version })
            })
            .transpose()
    }
    pub(crate) async fn read_health_row(
        &self,
        id: &str,
    ) -> Result<Option<HealthRow>, GatewayError> {
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM credential_model_states WHERE id=COALESCE((SELECT canonical FROM model_state_aliases WHERE alias=?),?)")
                .bind(id)
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage_error)?;
        payload
            .map(|version| {
                Ok(HealthRow {
                    row: decode(&version)?,
                    version,
                })
            })
            .transpose()
    }
    pub(crate) async fn restore_health_row(
        &self,
        expected: &HealthRow,
        before: Option<&HealthRow>,
    ) -> Result<(), GatewayError> {
        let mut row = before
            .map(|row| row.row.clone())
            .unwrap_or_else(|| expected.row.clone());
        if before.is_none() {
            clear(&mut row);
        }
        let payload = encode(row, &expected.row.updated_at)?;
        sqlx::query("UPDATE credential_model_states SET payload=?,updated=json_extract(?,'$.updatedAt') WHERE id=? AND payload=?")
            .bind(&payload).bind(&payload).bind(&expected.row.id).bind(&expected.version)
            .execute(&self.pool).await.map_err(storage_error)?;
        Ok(())
    }
    pub(crate) async fn reset_provider_health(&self, provider: &str) -> Result<(), GatewayError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let provider = super::super::model_identity::resolve(&mut tx, provider).await?;
        let mut cursor = String::new();
        loop {
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT id,payload FROM credential_model_states
                WHERE json_extract(payload,'$.providerAccountId')=? AND id>? ORDER BY id LIMIT 256",
            )
            .bind(&provider)
            .bind(&cursor)
            .fetch_all(&mut *tx)
            .await
            .map_err(storage_error)?;
            if rows.is_empty() {
                break;
            }
            for (id, payload) in rows {
                let mut row = decode(&payload)?;
                let previous = row.updated_at.clone();
                clear(&mut row);
                let payload = encode(row, &previous)?;
                sqlx::query("UPDATE credential_model_states SET payload=?,updated=json_extract(?,'$.updatedAt') WHERE id=?")
                    .bind(&payload).bind(&payload).bind(&id).execute(&mut *tx).await.map_err(storage_error)?;
                cursor = id;
            }
        }
        tx.commit().await.map_err(storage_error)
    }
}
