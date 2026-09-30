use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::OffsetDateTime;

use crate::error::{sanitize_provider_error_message, GatewayError};
use crate::provider_failure::ProviderFailureClassification;
use crate::provider_health_state::{candidate_penalty_status, transition_for_failure};
use crate::routing::candidate::RouteCandidate;

use super::{format_timestamp, map_db_error};

#[derive(Debug, Clone)]
pub struct RecordCredentialModelFailureInput {
    pub provider_account_id: String,
    pub provider_credential_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_profile: Option<String>,
    pub model: String,
    pub upstream_status: Option<u16>,
    pub error_message: Option<String>,
    pub classification: ProviderFailureClassification,
}

#[derive(Debug, Clone)]
pub struct RecordCredentialModelSuccessInput {
    pub provider_account_id: String,
    pub provider_credential_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_profile: Option<String>,
    pub model: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialModelStateFilters {
    pub provider_account_id: Option<String>,
    pub provider_credential_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_profile: Option<String>,
    pub model: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderCredentialModelStateView {
    pub id: String,
    pub provider_account_id: String,
    pub provider_credential_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_profile: Option<String>,
    pub model: String,
    pub status: String,
    pub failure_class: Option<String>,
    pub failure_scope: Option<String>,
    pub failure_count: i32,
    pub last_error: Option<String>,
    pub last_upstream_status: Option<i32>,
    pub cooldown_until: Option<String>,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProviderCredentialModelStateRow {
    id: String,
    provider_account_id: String,
    provider_credential_id: Option<String>,
    provider_credential_ref: Option<String>,
    protocol_profile: Option<String>,
    model: String,
    status: String,
    failure_class: Option<String>,
    failure_scope: Option<String>,
    failure_count: i32,
    last_error: Option<String>,
    last_upstream_status: Option<i32>,
    cooldown_until: Option<OffsetDateTime>,
    last_success_at: Option<OffsetDateTime>,
    last_failure_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

pub async fn record_provider_credential_model_failure(
    pool: &PgPool,
    input: RecordCredentialModelFailureInput,
) -> Result<Option<GatewayProviderCredentialModelStateView>, GatewayError> {
    let transition = transition_for_failure(&input.classification);
    if !transition.penalizes_candidate {
        return Ok(None);
    }

    let now = OffsetDateTime::now_utc();
    let cooldown_until = transition.cooldown_until(now);
    let id = provider_credential_model_state_id(
        &input.provider_account_id,
        input.provider_credential_id.as_deref(),
        input.provider_credential_ref.as_deref(),
        input.protocol_profile.as_deref(),
        &input.model,
    );
    let last_error = input
        .error_message
        .as_deref()
        .map(durable_model_error_message);

    let row = sqlx::query_as::<_, GatewayProviderCredentialModelStateRow>(
        r#"
        insert into gateway_provider_credential_model_states (
          id,
          provider_account_id,
          provider_credential_id,
          provider_credential_ref,
          protocol_profile,
          model,
          status,
          failure_class,
          failure_scope,
          failure_count,
          last_error,
          last_upstream_status,
          cooldown_until,
          last_failure_at,
          created_at,
          updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, 1, $10, $11, $12, $13, $13, $13
        )
        on conflict (id) do update set
          status = excluded.status,
          failure_class = excluded.failure_class,
          failure_scope = excluded.failure_scope,
          failure_count = gateway_provider_credential_model_states.failure_count + 1,
          last_error = excluded.last_error,
          last_upstream_status = excluded.last_upstream_status,
          cooldown_until = excluded.cooldown_until,
          last_failure_at = excluded.last_failure_at,
          updated_at = excluded.updated_at
        returning *
        "#,
    )
    .bind(id)
    .bind(input.provider_account_id)
    .bind(input.provider_credential_id)
    .bind(input.provider_credential_ref)
    .bind(input.protocol_profile)
    .bind(input.model)
    .bind(transition.status.as_str())
    .bind(transition.failure_class)
    .bind(transition.failure_scope)
    .bind(last_error)
    .bind(input.upstream_status.map(i32::from))
    .bind(cooldown_until)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(Some(to_view(row)))
}

pub async fn record_provider_credential_model_success(
    pool: &PgPool,
    input: RecordCredentialModelSuccessInput,
) -> Result<GatewayProviderCredentialModelStateView, GatewayError> {
    let now = OffsetDateTime::now_utc();
    let id = provider_credential_model_state_id(
        &input.provider_account_id,
        input.provider_credential_id.as_deref(),
        input.provider_credential_ref.as_deref(),
        input.protocol_profile.as_deref(),
        &input.model,
    );

    let row = sqlx::query_as::<_, GatewayProviderCredentialModelStateRow>(
        r#"
        insert into gateway_provider_credential_model_states (
          id,
          provider_account_id,
          provider_credential_id,
          provider_credential_ref,
          protocol_profile,
          model,
          status,
          failure_count,
          cooldown_until,
          last_success_at,
          created_at,
          updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, 'active', 0, null, $7, $7, $7
        )
        on conflict (id) do update set
          status = 'active',
          failure_class = null,
          failure_scope = null,
          failure_count = 0,
          last_error = null,
          last_upstream_status = null,
          cooldown_until = null,
          last_success_at = excluded.last_success_at,
          updated_at = excluded.updated_at
        returning *
        "#,
    )
    .bind(id)
    .bind(input.provider_account_id)
    .bind(input.provider_credential_id)
    .bind(input.provider_credential_ref)
    .bind(input.protocol_profile)
    .bind(input.model)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(to_view(row))
}

pub async fn list_provider_credential_model_states(
    pool: &PgPool,
    filters: CredentialModelStateFilters,
) -> Result<Vec<GatewayProviderCredentialModelStateView>, GatewayError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "select * from gateway_provider_credential_model_states where 1 = 1",
    );

    push_optional_text_filter(
        &mut builder,
        "provider_account_id",
        filters.provider_account_id.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "provider_credential_id",
        filters.provider_credential_id.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "provider_credential_ref",
        filters.provider_credential_ref.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "protocol_profile",
        filters.protocol_profile.as_deref(),
    );
    push_optional_text_filter(&mut builder, "model", filters.model.as_deref());
    push_optional_text_filter(&mut builder, "status", filters.status.as_deref());

    builder.push(" order by updated_at desc limit ");
    builder.push_bind(i64::try_from(filters.limit.unwrap_or(200).clamp(1, 1000)).unwrap_or(200));

    let rows = builder
        .build_query_as::<GatewayProviderCredentialModelStateRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;

    Ok(rows.into_iter().map(to_view).collect())
}

pub async fn apply_provider_credential_model_states_to_candidates(
    pool: &PgPool,
    candidates: &mut [RouteCandidate],
) -> Result<(), GatewayError> {
    for candidate in candidates {
        let Some(model) = candidate
            .upstream_model
            .as_deref()
            .or(candidate.model_alias.as_deref())
        else {
            continue;
        };
        let id = provider_credential_model_state_id(
            &candidate.provider_account_id,
            candidate.provider_credential_id.as_deref(),
            candidate.provider_credential_id.as_deref(),
            Some(candidate.protocol_profile.as_str()),
            model,
        );
        let state = sqlx::query_as::<_, CandidateModelStateRow>(
            r#"
            select status, failure_count, cooldown_until
            from gateway_provider_credential_model_states
            where id = $1
            limit 1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?;

        let Some(state) = state else {
            continue;
        };

        if let Some(cooldown_until) = state.cooldown_until.map(format_timestamp) {
            if matches!(state.status.as_str(), "cooling" | "blocked") {
                candidate.cooldown_until = Some(cooldown_until.clone());
            }
            if let Some(reason) =
                candidate_penalty_status(state.status.as_str(), Some(cooldown_until.as_str()))
            {
                candidate.routing_degradation_reasons.push(reason);
            }
        } else if state.status == "blocked" {
            candidate.cooldown_until = Some("9999-12-31T23:59:59Z".to_string());
            candidate
                .routing_degradation_reasons
                .push("credential_model_blocked".to_string());
        } else if let Some(reason) = candidate_penalty_status(state.status.as_str(), None) {
            candidate.routing_degradation_reasons.push(reason);
        }
        candidate.failure_count = candidate
            .failure_count
            .saturating_add(u32::try_from(state.failure_count.max(0)).unwrap_or(0));
    }
    Ok(())
}

pub fn provider_credential_model_state_id(
    provider_account_id: &str,
    provider_credential_id: Option<&str>,
    provider_credential_ref: Option<&str>,
    protocol_profile: Option<&str>,
    model: &str,
) -> String {
    let mut hasher = Sha256::new();
    // Route-config credentials have a reference even when no database ID exists.
    let credential_key = provider_credential_id
        .or(provider_credential_ref)
        .unwrap_or("");
    for part in [
        provider_account_id,
        credential_key,
        protocol_profile.unwrap_or(""),
        model,
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    format!("pcms_{}", hex::encode(hasher.finalize()))
}

#[derive(Debug, Clone, FromRow)]
struct CandidateModelStateRow {
    status: String,
    failure_count: i32,
    cooldown_until: Option<OffsetDateTime>,
}

fn push_optional_text_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    column_name: &'static str,
    value: Option<&str>,
) {
    let Some(value) = value.and_then(trim_nonempty) else {
        return;
    };
    builder.push(" and ");
    builder.push(column_name);
    builder.push(" = ");
    builder.push_bind(value.to_string());
}

fn trim_nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn truncate_error(value: &str) -> String {
    value.chars().take(1000).collect()
}

pub(crate) fn durable_model_error_message(value: &str) -> String {
    truncate_error(&sanitize_provider_error_message(value))
}

fn to_view(row: GatewayProviderCredentialModelStateRow) -> GatewayProviderCredentialModelStateView {
    GatewayProviderCredentialModelStateView {
        id: row.id,
        provider_account_id: row.provider_account_id,
        provider_credential_id: row.provider_credential_id,
        provider_credential_ref: row.provider_credential_ref,
        protocol_profile: row.protocol_profile,
        model: row.model,
        status: row.status,
        failure_class: row.failure_class,
        failure_scope: row.failure_scope,
        failure_count: row.failure_count,
        last_error: row.last_error,
        last_upstream_status: row.last_upstream_status,
        cooldown_until: row.cooldown_until.map(format_timestamp),
        last_success_at: row.last_success_at.map(format_timestamp),
        last_failure_at: row.last_failure_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_model_error_message_redacts_before_truncating() {
        let raw = format!(
            "invalid api key; Authorization: Bearer model-state-secret; token=model-token-secret {}",
            "x".repeat(2_000)
        );

        let sanitized = durable_model_error_message(&raw);

        assert!(sanitized.contains("invalid api key"));
        assert!(sanitized.contains("[REDACTED]"));
        assert!(!sanitized.contains("model-state-secret"));
        assert!(!sanitized.contains("model-token-secret"));
        assert!(sanitized.chars().count() <= 1_000);
    }
}
