use crate::db::{format_timestamp, map_db_error};
use crate::error::GatewayError;
use sqlx::{PgPool, Postgres, QueryBuilder};
use time::OffsetDateTime;

use super::*;

pub async fn list_credential_stock_policies(
    pool: &PgPool,
    filters: &CredentialStockStatusFilters,
) -> Result<Vec<CredentialStockPolicyView>, GatewayError> {
    let rows = query_policy_rows(pool, filters, false).await?;
    Ok(rows.into_iter().map(policy_view_from_row).collect())
}

pub async fn upsert_credential_stock_policy(
    pool: &PgPool,
    input: UpsertCredentialStockPolicyInput,
) -> Result<CredentialStockPolicyView, GatewayError> {
    let stock_class_key = normalize_required_key(&input.stock_class_key, "stockClassKey", 160)?;
    let display_name = normalize_required_text(&input.display_name, "displayName", 200)?;
    let service_provider_key =
        normalize_required_key(&input.service_provider_key, "serviceProviderKey", 120)?;
    let implementation_line_key =
        normalize_required_key(&input.implementation_line_key, "implementationLineKey", 160)?;
    let provider_surface_key =
        normalize_required_key(&input.provider_surface_key, "providerSurfaceKey", 160)?;
    let credential_material_kind =
        normalize_credential_material_kind(&input.credential_material_kind)?;
    let metric_kind = resolve_provider_supply_metric_kind(
        input.provider_supply_metric_kind.as_deref(),
        input.legacy_metric_kind.as_deref(),
    )?;
    let id = input
        .id
        .as_deref()
        .and_then(trim_nonempty)
        .map(str::to_string)
        .unwrap_or_else(|| format!("credential_stock_policy_{}", uuid::Uuid::new_v4()));
    let selector = input.selector.unwrap_or_else(|| serde_json::json!({}));
    let signal_stream = input
        .signal_stream
        .as_deref()
        .and_then(trim_nonempty)
        .unwrap_or(DEFAULT_SIGNAL_STREAM)
        .to_string();
    let now = OffsetDateTime::now_utc();

    let row = sqlx::query_as::<_, CredentialStockPolicyRow>(
        r#"
        insert into gateway_credential_stock_policies (
          id,
          stock_class_key,
          display_name,
          service_provider_key,
          implementation_line_key,
          provider_surface_key,
          credential_material_kind,
          provider_account_id,
          provider_adapter,
          selector,
          metric_kind,
          token_window_key,
          token_window_seconds,
          min_credential_count,
          target_credential_count,
          max_credential_count,
          min_average_available_tokens,
          target_average_available_tokens,
          signal_enabled,
          signal_stream,
          signal_cooldown_secs,
          enabled,
          created_at,
          updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
          $11, $12, $13, $14, $15, $16, $17, $18,
          $19, $20, $21, $22, $23, $23
        )
        on conflict (stock_class_key) do update set
          display_name = excluded.display_name,
          service_provider_key = excluded.service_provider_key,
          implementation_line_key = excluded.implementation_line_key,
          provider_surface_key = excluded.provider_surface_key,
          credential_material_kind = excluded.credential_material_kind,
          provider_account_id = excluded.provider_account_id,
          provider_adapter = excluded.provider_adapter,
          selector = excluded.selector,
          metric_kind = excluded.metric_kind,
          token_window_key = excluded.token_window_key,
          token_window_seconds = excluded.token_window_seconds,
          min_credential_count = excluded.min_credential_count,
          target_credential_count = excluded.target_credential_count,
          max_credential_count = excluded.max_credential_count,
          min_average_available_tokens = excluded.min_average_available_tokens,
          target_average_available_tokens = excluded.target_average_available_tokens,
          signal_enabled = excluded.signal_enabled,
          signal_stream = excluded.signal_stream,
          signal_cooldown_secs = excluded.signal_cooldown_secs,
          enabled = excluded.enabled,
          updated_at = excluded.updated_at
        returning *
        "#,
    )
    .bind(id)
    .bind(stock_class_key)
    .bind(display_name)
    .bind(service_provider_key)
    .bind(implementation_line_key)
    .bind(provider_surface_key)
    .bind(credential_material_kind)
    .bind(normalize_optional_key(
        input.provider_account_id.as_deref(),
        "providerAccountId",
        160,
    )?)
    .bind(normalize_optional_key(
        input.provider_adapter.as_deref(),
        "providerAdapter",
        120,
    )?)
    .bind(sqlx::types::Json(selector))
    .bind(metric_kind)
    .bind(normalize_optional_key(
        input.token_window_key.as_deref(),
        "tokenWindowKey",
        120,
    )?)
    .bind(input.token_window_seconds.filter(|value| *value > 0))
    .bind(input.min_credential_count.filter(|value| *value >= 0))
    .bind(input.target_credential_count.filter(|value| *value >= 0))
    .bind(input.max_credential_count.filter(|value| *value >= 0))
    .bind(
        input
            .min_average_available_tokens
            .filter(|value| *value >= 0),
    )
    .bind(
        input
            .target_average_available_tokens
            .filter(|value| *value >= 0),
    )
    .bind(input.signal_enabled.unwrap_or(true))
    .bind(signal_stream)
    .bind(input.signal_cooldown_secs.unwrap_or(300).max(0))
    .bind(input.enabled.unwrap_or(true))
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(map_db_error)?;

    Ok(policy_view_from_row(row))
}

pub(super) async fn query_policy_rows(
    pool: &PgPool,
    filters: &CredentialStockStatusFilters,
    only_enabled: bool,
) -> Result<Vec<CredentialStockPolicyRow>, GatewayError> {
    let mut builder =
        QueryBuilder::<Postgres>::new("select * from gateway_credential_stock_policies where 1=1");
    if only_enabled {
        builder.push(" and enabled = true");
    }
    push_optional_text_filter(
        &mut builder,
        "stock_class_key",
        filters.stock_class_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "service_provider_key",
        filters.service_provider_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "implementation_line_key",
        filters.implementation_line_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "provider_surface_key",
        filters.provider_surface_key.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "credential_material_kind",
        filters.credential_material_kind.as_deref(),
    );
    push_optional_text_filter(
        &mut builder,
        "provider_account_id",
        filters.provider_account_id.as_deref(),
    );
    builder.push(" order by stock_class_key asc limit ");
    builder.push_bind(i64::try_from(filters.limit.unwrap_or(500).clamp(1, 2000)).unwrap_or(500));
    builder
        .build_query_as::<CredentialStockPolicyRow>()
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
}

pub(super) fn policy_view_from_row(row: CredentialStockPolicyRow) -> CredentialStockPolicyView {
    CredentialStockPolicyView {
        id: row.id,
        stock_class_key: row.stock_class_key,
        display_name: row.display_name,
        service_provider_key: row.service_provider_key,
        implementation_line_key: row.implementation_line_key,
        provider_surface_key: row.provider_surface_key,
        credential_material_kind: row.credential_material_kind,
        provider_account_id: row.provider_account_id,
        provider_adapter: row.provider_adapter,
        selector: row.selector.0,
        provider_supply_metric_kind: row.metric_kind.clone(),
        metric_kind: row.metric_kind,
        token_window_key: row.token_window_key,
        token_window_seconds: row.token_window_seconds,
        min_credential_count: row.min_credential_count,
        target_credential_count: row.target_credential_count,
        max_credential_count: row.max_credential_count,
        min_average_available_tokens: row.min_average_available_tokens,
        target_average_available_tokens: row.target_average_available_tokens,
        signal_enabled: row.signal_enabled,
        signal_stream: row.signal_stream,
        signal_cooldown_secs: row.signal_cooldown_secs,
        enabled: row.enabled,
        last_signal_key: row.last_signal_key,
        last_signal_at: row.last_signal_at.map(format_timestamp),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
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
