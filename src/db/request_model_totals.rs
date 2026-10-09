//! Unsampled, retained audit totals. Recent summary limits never apply here.
use crate::{error::GatewayError, local_runtime::LocalRuntime};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RequestModelTotals {
    pub provider_account_id: String,
    // Older audits without explicit attribution contribute only to the pool.
    pub credential_ref: Option<String>,
    pub model: String,
    pub request_count: i64,
    pub success_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryWithModelTotals {
    #[serde(flatten)]
    pub summary: super::GatewayRequestAuditSummaryView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained_model_totals: Option<Vec<RequestModelTotals>>,
}

pub async fn load_retained_model_totals(
    pool: &PgPool,
) -> Result<Vec<RequestModelTotals>, GatewayError> {
    sqlx::query_as::<_, RequestModelTotals>(
        "WITH source AS (
            SELECT trim(provider_account_id) AS provider_account_id,
                CASE WHEN jsonb_typeof(route_trace->'realCredentialRef') = 'string'
                    THEN nullif(trim(route_trace->>'realCredentialRef'), '') END AS credential_ref,
                coalesce(nullif(trim(resolved_model), ''), nullif(trim(requested_model), ''),
                    nullif(trim(model_alias), ''), 'unknown') AS model, status
            FROM gateway_request_audits WHERE nullif(trim(provider_account_id), '') IS NOT NULL
        ) SELECT provider_account_id, credential_ref, model, count(*) AS request_count,
            count(*) FILTER (WHERE status = 'completed') AS success_count
        FROM source GROUP BY provider_account_id, credential_ref, model
        ORDER BY provider_account_id, credential_ref, model",
    )
    .fetch_all(pool)
    .await
    .map_err(super::map_db_error)
}

pub async fn load_local_retained_model_totals(
    db: &LocalRuntime,
) -> Result<Vec<RequestModelTotals>, GatewayError> {
    sqlx::query_as::<_, RequestModelTotals>(
        "WITH source AS (
            SELECT trim(json_extract(payload, '$.providerAccountId')) AS provider_account_id,
                CASE WHEN json_type(payload, '$.routeTrace.realCredentialRef') = 'text'
                    THEN nullif(trim(json_extract(payload, '$.routeTrace.realCredentialRef')), '')
                    END AS credential_ref,
                coalesce(nullif(trim(json_extract(payload, '$.resolvedModel')), ''),
                    nullif(trim(json_extract(payload, '$.requestedModel')), ''),
                    nullif(trim(json_extract(payload, '$.modelAlias')), ''), 'unknown') AS model,
                status FROM request_audits
            WHERE nullif(trim(json_extract(payload, '$.providerAccountId')), '') IS NOT NULL
        ) SELECT provider_account_id, credential_ref, model, count(*) AS request_count,
            count(*) FILTER (WHERE status = 'completed') AS success_count
        FROM source GROUP BY provider_account_id, credential_ref, model
        ORDER BY provider_account_id, credential_ref, model",
    )
    .fetch_all(&db.pool)
    .await
    .map_err(crate::local_runtime::storage_error)
}

#[cfg(test)]
#[path = "request_model_totals_tests.rs"]
mod tests;
