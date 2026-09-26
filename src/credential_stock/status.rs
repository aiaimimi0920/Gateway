use crate::db::map_db_error;
use crate::error::GatewayError;
use crate::provider_quota::read_cached_runtime_quota_snapshot;
use deadpool_redis::Pool as RedisPool;
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::*;

pub async fn get_credential_stock_status_report(
    pool: &PgPool,
    redis_pool: &RedisPool,
    filters: CredentialStockStatusFilters,
) -> Result<CredentialStockStatusReport, GatewayError> {
    let policies = query_policy_rows(pool, &filters, true).await?;
    let mut statuses = Vec::with_capacity(policies.len());
    for row in policies {
        let status = evaluate_policy_row(pool, redis_pool, row).await?;
        if let Some(needs) = filters.needs_replenishment {
            if status.needs_replenishment != needs {
                continue;
            }
        }
        statuses.push(status);
    }
    Ok(CredentialStockStatusReport {
        summary: build_stock_summary(&statuses),
        policies: statuses,
    })
}

async fn evaluate_policy_row(
    pool: &PgPool,
    redis_pool: &RedisPool,
    row: CredentialStockPolicyRow,
) -> Result<CredentialStockStatusView, GatewayError> {
    let credentials = list_policy_credentials(pool, &row).await?;
    let mut usable_credential_count = 0usize;
    let mut active_credential_count = 0usize;
    let mut cooling_credential_count = 0usize;
    let mut disabled_credential_count = 0usize;
    let mut archived_credential_count = 0usize;
    let mut unknown_status_credential_count = 0usize;
    let mut available_tokens = Vec::new();

    let requires_quota_snapshot = metric_requires_quota_snapshot(&row.metric_kind);
    for credential in &credentials {
        if credential.archived_at.is_some() {
            archived_credential_count += 1;
            continue;
        }
        match credential.status.as_str() {
            "active" => {
                active_credential_count += 1;
                usable_credential_count += 1;
                if requires_quota_snapshot {
                    available_tokens.push(
                        read_cached_runtime_quota_snapshot(
                            redis_pool,
                            row.provider_account_id.as_deref().unwrap_or_default(),
                            Some(credential.provider_credential_id.as_str()),
                        )
                        .await
                        .ok()
                        .flatten()
                        .as_ref()
                        .and_then(remaining_tokens_for_policy_window),
                    );
                }
            }
            "cooling" => cooling_credential_count += 1,
            "disabled" | "inactive" => disabled_credential_count += 1,
            "archived" => archived_credential_count += 1,
            _ => unknown_status_credential_count += 1,
        }
    }

    let count_evaluation = evaluate_credential_count(
        usable_credential_count,
        CountWatermark {
            min: row
                .min_credential_count
                .and_then(|value| usize::try_from(value).ok()),
            target: row
                .target_credential_count
                .and_then(|value| usize::try_from(value).ok()),
            max: row
                .max_credential_count
                .and_then(|value| usize::try_from(value).ok()),
        },
    );
    let token_window_key = row.token_window_key.clone();
    let token_window_seconds = row.token_window_seconds;
    let token_evaluation = (row.metric_kind == "token_window").then(|| {
        evaluate_token_window(
            &available_tokens,
            TokenWindowWatermark {
                min_average_available_tokens: row.min_average_available_tokens,
                target_average_available_tokens: row.target_average_available_tokens,
                target_credential_count: row
                    .target_credential_count
                    .and_then(|value| usize::try_from(value).ok()),
            },
        )
    });
    let needs_replenishment =
        combined_needs_replenishment(&count_evaluation, token_evaluation.as_ref());
    let severity = combined_stock_severity(&count_evaluation, token_evaluation.as_ref());
    let suggested_credential_top_up_count = count_evaluation.suggested_credential_top_up_count.max(
        token_evaluation
            .as_ref()
            .map(|v| v.suggested_credential_top_up_count)
            .unwrap_or(0),
    );
    let signal_key = build_signal_key(
        &row.stock_class_key,
        &row.metric_kind,
        Some(suggested_credential_top_up_count),
        row.token_window_key.as_deref(),
    );
    let policy = policy_view_from_row(row);
    let signal_payload = serde_json::json!({
        "eventType": "gateway.credential_stock.replenishment_needed",
        "stockClassKey": policy.stock_class_key,
        "serviceProviderKey": policy.service_provider_key,
        "implementationLineKey": policy.implementation_line_key,
        "providerSurfaceKey": policy.provider_surface_key,
        "credentialMaterialKind": policy.credential_material_kind,
        "providerAccountId": policy.provider_account_id,
        "providerAdapter": policy.provider_adapter,
        "providerSupplyMetricKind": policy.provider_supply_metric_kind,
        "metricKind": policy.metric_kind,
        "severity": severity,
        "needsReplenishment": needs_replenishment,
        "usableCredentialCount": usable_credential_count,
        "suggestedCredentialTopUpCount": suggested_credential_top_up_count,
        "countEvaluation": count_evaluation,
        "tokenEvaluation": token_evaluation,
        "signalKey": signal_key,
    });

    Ok(CredentialStockStatusView {
        policy,
        provider_supply_metric_kind: signal_payload["providerSupplyMetricKind"]
            .as_str()
            .unwrap_or("credential_count")
            .to_string(),
        metric_kind: signal_payload["metricKind"]
            .as_str()
            .unwrap_or("credential_count")
            .to_string(),
        token_window_key,
        token_window_seconds,
        usable_credential_count,
        active_credential_count,
        cooling_credential_count,
        disabled_credential_count,
        archived_credential_count,
        unknown_status_credential_count,
        count_evaluation,
        token_evaluation,
        needs_replenishment,
        severity,
        suggested_credential_top_up_count,
        signal_key,
        signal_payload,
    })
}

async fn list_policy_credentials(
    pool: &PgPool,
    policy: &CredentialStockPolicyRow,
) -> Result<Vec<CredentialStockCredentialRow>, GatewayError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r#"
        select
          c.id as provider_credential_id,
          c.status,
          c.archived_at
        from gateway_provider_credentials c
        join gateway_provider_accounts a on a.id = c.provider_account_id
        where 1=1
        "#,
    );
    if let Some(provider_account_id) = policy.provider_account_id.as_deref() {
        builder.push(" and c.provider_account_id = ");
        builder.push_bind(provider_account_id.to_string());
    }
    if let Some(provider_adapter) = policy.provider_adapter.as_deref() {
        builder.push(" and a.adapter = ");
        builder.push_bind(provider_adapter.to_string());
    }
    builder.push(" and a.service_provider_key = ");
    builder.push_bind(policy.service_provider_key.clone());
    builder.push(" and a.protocol_profile = ");
    builder.push_bind(policy.provider_surface_key.clone());
    if policy.credential_material_kind != "*" && policy.credential_material_kind != "any" {
        builder.push(" and coalesce(c.payload_inline->>'credentialMaterialKind', c.payload_inline->>'credential_material_kind', '') = ");
        builder.push_bind(policy.credential_material_kind.clone());
    }
    builder.push(" order by c.created_at asc");

    builder
        .build_query_as::<CredentialStockCredentialRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
}

pub(super) fn metric_requires_quota_snapshot(metric_kind: &str) -> bool {
    metric_kind.trim() == "token_window"
}

fn build_stock_summary(statuses: &[CredentialStockStatusView]) -> CredentialStockSummaryView {
    CredentialStockSummaryView {
        total_policy_count: statuses.len(),
        enabled_policy_count: statuses
            .iter()
            .filter(|status| status.policy.enabled)
            .count(),
        needs_replenishment_policy_count: statuses
            .iter()
            .filter(|status| status.needs_replenishment)
            .count(),
        critical_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Critical)
            .count(),
        warning_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Warning)
            .count(),
        healthy_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Healthy)
            .count(),
        overstock_policy_count: statuses
            .iter()
            .filter(|status| status.severity == StockSeverity::Overstock)
            .count(),
        total_suggested_credential_top_up_count: statuses
            .iter()
            .map(|status| status.suggested_credential_top_up_count)
            .sum(),
    }
}
