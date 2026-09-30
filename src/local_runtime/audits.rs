//! Local request lifecycle: one durable row per logical request, not per retry.
use super::{storage_error, LocalRuntime};
use crate::{
    db::{CreateRequestAuditInput, FinalizeRequestAuditInput, GatewayRequestAuditView},
    error::GatewayError,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

fn timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("valid timestamp")
}
fn count(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

impl LocalRuntime {
    pub async fn route_audit(
        &self,
        id: &str,
        provider: &str,
        model: &str,
    ) -> Result<(), GatewayError> {
        sqlx::query(
            "UPDATE request_audits SET payload = json_set(payload,
            '$.providerAccountId', ?, '$.resolvedModel', ?) WHERE id = ? AND status = 'running'",
        )
        .bind(provider)
        .bind(model)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(())
    }

    pub async fn create_audit(
        &self,
        input: CreateRequestAuditInput,
    ) -> Result<String, GatewayError> {
        let id = input.response_id.clone();
        let now = timestamp();
        let row = GatewayRequestAuditView {
            id: id.clone(),
            project_id: input.project_id,
            api_key_id: input.api_key_id,
            user_credential_id: input.user_credential_id,
            access_key_id: input.access_key_id,
            source_access_key_id: input.source_access_key_id,
            session_id: input.session_id,
            route_policy_id: input.route_policy_id,
            provider_account_id: input.provider_account_id,
            protocol_family: input.protocol_family,
            endpoint_kind: input.endpoint_kind,
            requested_model: input.requested_model,
            resolved_model: input.resolved_model,
            model_alias: input.model_alias,
            stream: input.stream,
            status: "running".into(),
            upstream_status: None,
            duration_ms: None,
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            client_has_cache_control: false,
            auto_cache_applied: false,
            error_summary: None,
            route_trace: input.route_trace,
            analysis_profile: None,
            request_artifact_object_key: None,
            response_artifact_object_key: None,
            response_id: input.response_id,
            previous_response_id: input.previous_response_id,
            client_disconnected_at: None,
            created_at: now.clone(),
            completed_at: None,
            updated_at: now.clone(),
        };
        sqlx::query(
            "INSERT INTO request_audits(id, payload, created, status, owner)
            VALUES (?, ?, ?, 'running', ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&id)
        .bind(encode(&row)?)
        .bind(now)
        .bind(&self.owner)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(id)
    }

    pub async fn finalize_audit(
        &self,
        id: &str,
        input: FinalizeRequestAuditInput,
    ) -> Result<(), GatewayError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage_error)?;
        let payload: Option<String> = sqlx::query_scalar(
            "SELECT payload FROM request_audits WHERE id = ? AND status = 'running'",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(storage_error)?;
        // Stream terminal hooks and cancellation may race; the first terminal result owns the row.
        if let Some(payload) = payload {
            let mut row = decode(&payload)?;
            row.status = input.status;
            row.upstream_status = input.upstream_status.map(i32::from);
            row.duration_ms = Some(count(input.duration_ms));
            row.prompt_tokens = input.prompt_tokens.map(count);
            row.completion_tokens = input.completion_tokens.map(count);
            row.total_tokens = input.total_tokens.map(count);
            row.cache_creation_input_tokens = input.cache_creation_input_tokens.map(count);
            row.cache_read_input_tokens = input.cache_read_input_tokens.map(count);
            row.client_has_cache_control = input.client_has_cache_control;
            row.auto_cache_applied = input.auto_cache_applied;
            row.error_summary = input
                .error_summary
                .as_deref()
                .map(crate::error::sanitize_provider_error_message);
            row.access_key_id = input.access_key_id.or(row.access_key_id);
            row.source_access_key_id = input.source_access_key_id.or(row.source_access_key_id);
            row.session_id = input.session_id.or(row.session_id);
            row.route_policy_id = input.route_policy_id.or(row.route_policy_id);
            row.provider_account_id = input.provider_account_id.or(row.provider_account_id);
            row.resolved_model = input.resolved_model.or(row.resolved_model);
            row.model_alias = input.model_alias.or(row.model_alias);
            row.route_trace = input.route_trace.or(row.route_trace);
            row.response_id = input.response_id.unwrap_or(row.response_id);
            row.updated_at = timestamp();
            row.completed_at = Some(row.updated_at.clone());
            sqlx::query("UPDATE request_audits SET payload = ?, status = ? WHERE id = ?")
                .bind(encode(&row)?)
                .bind(&row.status)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
        }
        tx.commit().await.map_err(storage_error)
    }
}

pub(super) fn encode(row: &GatewayRequestAuditView) -> Result<String, GatewayError> {
    serde_json::to_string(row)
        .map_err(|_| GatewayError::server_error("Cannot serialize local request audit"))
}
pub(super) fn decode(payload: &str) -> Result<GatewayRequestAuditView, GatewayError> {
    serde_json::from_str(payload)
        .map_err(|_| GatewayError::server_error("Local request audit is corrupt"))
}

pub async fn create(
    state: &crate::state::AppState,
    input: CreateRequestAuditInput,
) -> Result<String, GatewayError> {
    if let Some(local) = &state.local_runtime {
        return local.create_audit(input).await;
    }
    let pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("Request audit storage unavailable"))?;
    crate::db::create_request_audit(pool, input).await
}

pub async fn finalize(
    state: &crate::state::AppState,
    id: &str,
    input: FinalizeRequestAuditInput,
) -> Result<(), GatewayError> {
    if let Some(local) = &state.local_runtime {
        return local.finalize_audit(id, input).await;
    }
    let pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("Request audit storage unavailable"))?;
    crate::db::finalize_request_audit(pool, id, input).await
}
