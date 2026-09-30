//! Durable credential/model health with the shared server classification rules.
use super::{storage_error, LocalRuntime};
use crate::db::provider_credential_model_states::{
    durable_model_error_message, provider_credential_model_state_id,
};
use crate::{
    db::*, error::GatewayError, provider_health_state::transition_for_failure, state::AppState,
};
use sqlx::{QueryBuilder, Sqlite};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

impl LocalRuntime {
    pub async fn model_success(
        &self,
        input: RecordCredentialModelSuccessInput,
    ) -> Result<GatewayProviderCredentialModelStateView, GatewayError> {
        self.update_model(input, None).await
    }

    pub async fn model_failure(
        &self,
        input: RecordCredentialModelFailureInput,
    ) -> Result<Option<GatewayProviderCredentialModelStateView>, GatewayError> {
        if !transition_for_failure(&input.classification).penalizes_candidate {
            return Ok(None);
        }
        let identity = RecordCredentialModelSuccessInput {
            provider_account_id: input.provider_account_id.clone(),
            provider_credential_id: input.provider_credential_id.clone(),
            provider_credential_ref: input.provider_credential_ref.clone(),
            protocol_profile: input.protocol_profile.clone(),
            model: input.model.clone(),
        };
        self.update_model(identity, Some(input)).await.map(Some)
    }

    async fn update_model(
        &self,
        input: RecordCredentialModelSuccessInput,
        failure: Option<RecordCredentialModelFailureInput>,
    ) -> Result<GatewayProviderCredentialModelStateView, GatewayError> {
        let now = OffsetDateTime::now_utc();
        let time = now.format(&Rfc3339).expect("timestamp");
        let id = provider_credential_model_state_id(
            &input.provider_account_id,
            input.provider_credential_id.as_deref(),
            input.provider_credential_ref.as_deref(),
            input.protocol_profile.as_deref(),
            &input.model,
        );
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let payload: Option<String> =
            sqlx::query_scalar("SELECT payload FROM credential_model_states WHERE id = ?")
                .bind(&id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?;
        let mut row = if let Some(payload) = payload {
            decode(&payload)?
        } else {
            GatewayProviderCredentialModelStateView {
                id,
                provider_account_id: input.provider_account_id,
                provider_credential_id: input.provider_credential_id,
                provider_credential_ref: input.provider_credential_ref,
                protocol_profile: input.protocol_profile,
                model: input.model,
                status: "active".into(),
                failure_class: None,
                failure_scope: None,
                failure_count: 0,
                last_error: None,
                last_upstream_status: None,
                cooldown_until: None,
                last_success_at: None,
                last_failure_at: None,
                created_at: time.clone(),
                updated_at: time.clone(),
            }
        };
        row.updated_at = time.clone();
        if let Some(failure) = failure {
            let transition = transition_for_failure(&failure.classification);
            row.status = transition.status.as_str().into();
            row.failure_class = transition.failure_class.clone();
            row.failure_scope = transition.failure_scope.clone();
            row.failure_count = row.failure_count.saturating_add(1);
            row.last_error = failure
                .error_message
                .as_deref()
                .map(durable_model_error_message);
            row.last_upstream_status = failure.upstream_status.map(i32::from);
            row.cooldown_until = transition
                .cooldown_until(now)
                .map(|time| time.format(&Rfc3339).expect("timestamp"));
            row.last_failure_at = Some(time.clone());
        } else {
            row.status = "active".into();
            row.failure_class = None;
            row.failure_scope = None;
            row.failure_count = 0;
            row.last_error = None;
            row.last_upstream_status = None;
            row.cooldown_until = None;
            row.last_success_at = Some(time.clone());
        }
        let payload = serde_json::to_string(&row)
            .map_err(|_| GatewayError::server_error("Cannot serialize model state"))?;
        sqlx::query(
            "INSERT INTO credential_model_states(id, payload, updated) VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, updated = excluded.updated",
        )
        .bind(&row.id)
        .bind(payload)
        .bind(time)
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
        tx.commit().await.map_err(storage_error)?;
        Ok(row)
    }

    pub async fn list_model_states(
        &self,
        filters: CredentialModelStateFilters,
    ) -> Result<Vec<GatewayProviderCredentialModelStateView>, GatewayError> {
        let mut query =
            QueryBuilder::<Sqlite>::new("SELECT payload FROM credential_model_states WHERE 1 = 1");
        for (field, value) in [
            ("providerAccountId", filters.provider_account_id),
            ("providerCredentialId", filters.provider_credential_id),
            ("providerCredentialRef", filters.provider_credential_ref),
            ("protocolProfile", filters.protocol_profile),
            ("model", filters.model),
            ("status", filters.status),
        ] {
            if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                query
                    .push(" AND json_extract(payload, '$.")
                    .push(field)
                    .push("') = ")
                    .push_bind(value.to_string());
            }
        }
        query
            .push(" ORDER BY updated DESC LIMIT ")
            .push_bind(filters.limit.unwrap_or(200).clamp(1, 1000) as i64);
        let rows: Vec<String> = query
            .build_query_scalar()
            .fetch_all(&self.pool)
            .await
            .map_err(storage_error)?;
        rows.iter().map(|payload| decode(payload)).collect()
    }
}

fn decode(payload: &str) -> Result<GatewayProviderCredentialModelStateView, GatewayError> {
    serde_json::from_str(payload)
        .map_err(|_| GatewayError::server_error("Local credential model state is corrupt"))
}

pub async fn success(
    state: &AppState,
    input: RecordCredentialModelSuccessInput,
) -> Result<(), GatewayError> {
    if let Some(local) = &state.local_runtime {
        local.model_success(input).await?;
    } else if let Some(pool) = &state.pg_pool {
        record_provider_credential_model_success(pool, input).await?;
    }
    Ok(())
}
pub async fn failure(
    state: &AppState,
    input: RecordCredentialModelFailureInput,
) -> Result<(), GatewayError> {
    if let Some(local) = &state.local_runtime {
        local.model_failure(input).await?;
    } else if let Some(pool) = &state.pg_pool {
        record_provider_credential_model_failure(pool, input).await?;
    }
    Ok(())
}
