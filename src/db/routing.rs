use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::object_storage::gateway_object_storage;
use crate::protocol::canonical::EndpointKind;
use crate::routing::candidate::{
    canonicalize_adapter_name, canonicalize_protocol_family_name, ProviderAccountPayload,
    ProviderExecutionMode, RouteCandidate,
};
use crate::routing::config::ModelInfo;
use crate::routing::protocol_resolution::{
    resolve_supported_wire_protocol_families_for_model, route_policy_family_matches_surface,
};

use super::access::bump_all_access_projection_versions;
use super::{
    format_timestamp, get_active_project, list_active_provider_credentials_for_accounts,
    map_db_error, merge_provider_account_and_credential_payloads,
    provider_accounts::recover_expired_cooling_provider_accounts,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRateLimitDefinition {
    pub window_seconds: i32,
    pub max_requests: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct GatewayRoutePolicyConfig {
    pub sticky_sessions: bool,
    pub pre_stream_fallback_enabled: bool,
    pub selection_strategy: String,
    pub provider_load_aware_routing_enabled: bool,
    pub max_concurrent_requests: Option<i32>,
    pub provider_max_concurrent_requests: Option<i32>,
    pub rate_limit_enforcement_version: Option<String>,
    pub rate_limit_window_seconds: Option<i32>,
    pub rate_limit_max_requests: Option<i32>,
    pub api_key_rate_limit: Option<GatewayRateLimitDefinition>,
    pub model_rate_limits: Option<HashMap<String, GatewayRateLimitDefinition>>,
    pub endpoint_rate_limits: Option<HashMap<String, GatewayRateLimitDefinition>>,
    pub provider_attempt_rate_limit: Option<GatewayRateLimitDefinition>,
    pub circuit_breaker_threshold: i32,
    pub circuit_breaker_cooldown_seconds: i32,
    pub allowed_provider_account_ids: Option<Vec<String>>,
    pub allowed_protocol_families: Option<Vec<String>>,
    pub allowed_model_ids: Option<Vec<String>>,
    pub blocked_model_ids: Option<Vec<String>>,
    pub max_request_body_bytes: Option<i64>,
    pub stream_idle_timeout_seconds: Option<i32>,
    pub total_request_timeout_seconds: Option<i32>,
    pub max_stream_heartbeat_gap_seconds: Option<i32>,
    pub routing_anomaly_auto_remediation: Option<Value>,
    pub rate_limit_hotspot_auto_remediation: Option<Value>,
    pub fallback_http_statuses: Vec<i32>,
    pub fallback_error_codes: Option<Vec<String>>,
}

impl Default for GatewayRoutePolicyConfig {
    fn default() -> Self {
        Self {
            sticky_sessions: true,
            pre_stream_fallback_enabled: true,
            selection_strategy: "weighted_random".to_string(),
            provider_load_aware_routing_enabled: true,
            max_concurrent_requests: Some(4),
            provider_max_concurrent_requests: None,
            rate_limit_enforcement_version: None,
            rate_limit_window_seconds: Some(60),
            rate_limit_max_requests: Some(30),
            api_key_rate_limit: None,
            model_rate_limits: None,
            endpoint_rate_limits: None,
            provider_attempt_rate_limit: None,
            circuit_breaker_threshold: 3,
            circuit_breaker_cooldown_seconds: 60,
            allowed_provider_account_ids: None,
            allowed_protocol_families: None,
            allowed_model_ids: None,
            blocked_model_ids: None,
            max_request_body_bytes: None,
            stream_idle_timeout_seconds: None,
            total_request_timeout_seconds: None,
            max_stream_heartbeat_gap_seconds: None,
            routing_anomaly_auto_remediation: None,
            rate_limit_hotspot_auto_remediation: None,
            fallback_http_statuses: vec![408, 425, 429, 500, 502, 503, 504],
            fallback_error_codes: Some(vec![
                "ECONNRESET".to_string(),
                "ECONNREFUSED".to_string(),
                "ETIMEDOUT".to_string(),
                "UND_ERR_CONNECT_TIMEOUT".to_string(),
                "UND_ERR_HEADERS_TIMEOUT".to_string(),
                "UND_ERR_BODY_TIMEOUT".to_string(),
            ]),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GatewayRateLimitDefinitionInput {
    pub window_seconds: Option<i32>,
    pub max_requests: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GatewayRoutePolicyConfigInput {
    pub sticky_sessions: Option<bool>,
    pub pre_stream_fallback_enabled: Option<bool>,
    pub selection_strategy: Option<String>,
    pub provider_load_aware_routing_enabled: Option<bool>,
    pub max_concurrent_requests: Option<i32>,
    pub provider_max_concurrent_requests: Option<i32>,
    pub rate_limit_enforcement_version: Option<String>,
    pub rate_limit_window_seconds: Option<i32>,
    pub rate_limit_max_requests: Option<i32>,
    pub api_key_rate_limit: Option<GatewayRateLimitDefinitionInput>,
    pub model_rate_limits: Option<HashMap<String, GatewayRateLimitDefinitionInput>>,
    pub endpoint_rate_limits: Option<HashMap<String, GatewayRateLimitDefinitionInput>>,
    pub provider_attempt_rate_limit: Option<GatewayRateLimitDefinitionInput>,
    pub circuit_breaker_threshold: Option<i32>,
    pub circuit_breaker_cooldown_seconds: Option<i32>,
    pub allowed_provider_account_ids: Option<Vec<String>>,
    pub allowed_protocol_families: Option<Vec<String>>,
    pub allowed_model_ids: Option<Vec<String>>,
    pub blocked_model_ids: Option<Vec<String>>,
    pub max_request_body_bytes: Option<i64>,
    pub stream_idle_timeout_seconds: Option<i32>,
    pub total_request_timeout_seconds: Option<i32>,
    pub max_stream_heartbeat_gap_seconds: Option<i32>,
    pub routing_anomaly_auto_remediation: Option<Value>,
    pub rate_limit_hotspot_auto_remediation: Option<Value>,
    pub fallback_http_statuses: Option<Vec<i32>>,
    pub fallback_error_codes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayRoutePolicyView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub is_default: bool,
    pub enabled: bool,
    pub config: GatewayRoutePolicyConfig,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayModelAliasView {
    pub id: String,
    pub project_id: Option<String>,
    pub scope_type: String,
    pub alias: String,
    pub provider_account_id: String,
    pub upstream_model: Option<String>,
    pub priority: i32,
    pub weight: i32,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct SaveRoutePolicyInput {
    pub project_id: String,
    pub name: String,
    pub is_default: bool,
    pub enabled: bool,
    pub config: GatewayRoutePolicyConfig,
}

#[derive(Debug, Clone)]
pub struct SaveModelAliasInput {
    pub project_id: Option<String>,
    pub scope_type: String,
    pub alias: String,
    pub provider_account_id: String,
    pub upstream_model: Option<String>,
    pub priority: i32,
    pub weight: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedProjectRouteContext {
    pub route_policy_id: String,
    pub route_policy_config: GatewayRoutePolicyConfig,
    pub selection_strategy: String,
    pub candidates: Vec<RouteCandidate>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayRoutePolicyRow {
    id: String,
    project_id: String,
    name: String,
    is_default: bool,
    enabled: bool,
    config: Json<Value>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayModelAliasRow {
    id: String,
    project_id: Option<String>,
    scope_type: String,
    alias: String,
    provider_account_id: String,
    upstream_model: Option<String>,
    priority: i32,
    weight: i32,
    enabled: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct ProviderCapabilityModelRow {
    provider_account_id: String,
    model_code: String,
    upstream_model: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct GatewayProviderRouteRow {
    id: String,
    label: String,
    adapter: String,
    protocol_family: String,
    protocol_profile: String,
    cooldown_until: Option<OffsetDateTime>,
    failure_count: i32,
    execution_mode: String,
    endpoint_execution_modes: Option<Json<Value>>,
    payload_inline: Option<Json<Value>>,
    payload_object_key: Option<String>,
}

pub fn normalize_route_policy_config(
    input: GatewayRoutePolicyConfigInput,
) -> Result<GatewayRoutePolicyConfig, GatewayError> {
    let defaults = GatewayRoutePolicyConfig::default();
    Ok(GatewayRoutePolicyConfig {
        sticky_sessions: input.sticky_sessions.unwrap_or(true),
        pre_stream_fallback_enabled: input.pre_stream_fallback_enabled.unwrap_or(true),
        selection_strategy: normalize_selection_strategy(input.selection_strategy.as_deref()),
        provider_load_aware_routing_enabled: input
            .provider_load_aware_routing_enabled
            .unwrap_or(true),
        max_concurrent_requests: normalize_non_negative_i32(input.max_concurrent_requests)?,
        provider_max_concurrent_requests: normalize_non_negative_i32(
            input.provider_max_concurrent_requests,
        )?,
        rate_limit_enforcement_version: normalize_rate_limit_enforcement_version(
            input.rate_limit_enforcement_version.as_deref(),
        )?,
        rate_limit_window_seconds: normalize_non_negative_i32(input.rate_limit_window_seconds)?,
        rate_limit_max_requests: normalize_non_negative_i32(input.rate_limit_max_requests)?,
        api_key_rate_limit: normalize_rate_limit_definition(
            "apiKeyRateLimit",
            input.api_key_rate_limit,
            defaults.api_key_rate_limit,
        )?,
        model_rate_limits: normalize_rate_limit_map(
            "modelRateLimits",
            input.model_rate_limits,
            defaults.model_rate_limits,
            true,
        )?,
        endpoint_rate_limits: normalize_rate_limit_map(
            "endpointRateLimits",
            input.endpoint_rate_limits,
            defaults.endpoint_rate_limits,
            true,
        )?,
        provider_attempt_rate_limit: normalize_rate_limit_definition(
            "providerAttemptRateLimit",
            input.provider_attempt_rate_limit,
            defaults.provider_attempt_rate_limit,
        )?,
        circuit_breaker_threshold: normalize_non_negative_i32(input.circuit_breaker_threshold)?
            .unwrap_or(3),
        circuit_breaker_cooldown_seconds: normalize_non_negative_i32(
            input.circuit_breaker_cooldown_seconds,
        )?
        .unwrap_or(60),
        allowed_provider_account_ids: normalize_string_list(
            input.allowed_provider_account_ids,
            false,
        ),
        allowed_protocol_families: normalize_string_list(input.allowed_protocol_families, true),
        allowed_model_ids: normalize_string_list(input.allowed_model_ids, true),
        blocked_model_ids: normalize_string_list(input.blocked_model_ids, true),
        max_request_body_bytes: normalize_positive_i64(
            "maxRequestBodyBytes",
            input.max_request_body_bytes,
            1_000_000_000,
        )?,
        stream_idle_timeout_seconds: normalize_positive_i32(
            "streamIdleTimeoutSeconds",
            input.stream_idle_timeout_seconds,
            300,
        )?,
        total_request_timeout_seconds: normalize_positive_i32(
            "totalRequestTimeoutSeconds",
            input.total_request_timeout_seconds,
            300,
        )?,
        max_stream_heartbeat_gap_seconds: normalize_positive_i32(
            "maxStreamHeartbeatGapSeconds",
            input.max_stream_heartbeat_gap_seconds,
            60,
        )?,
        routing_anomaly_auto_remediation: input.routing_anomaly_auto_remediation,
        rate_limit_hotspot_auto_remediation: input.rate_limit_hotspot_auto_remediation,
        fallback_http_statuses: input
            .fallback_http_statuses
            .map(|values| {
                values
                    .into_iter()
                    .filter(|value| (100..=599).contains(value))
                    .collect::<Vec<_>>()
            })
            .unwrap_or(defaults.fallback_http_statuses),
        fallback_error_codes: normalize_string_list(input.fallback_error_codes, true)
            .or(defaults.fallback_error_codes),
    })
}

pub async fn list_route_policies(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<GatewayRoutePolicyView>, GatewayError> {
    let rows = sqlx::query_as::<_, GatewayRoutePolicyRow>(
        r#"
        select id, project_id, name, is_default, enabled, config, created_at, updated_at
        from gateway_route_policies
        where project_id = $1
        order by updated_at desc, created_at desc
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    rows.into_iter().map(route_policy_view_from_row).collect()
}

pub async fn save_route_policy(
    pool: &PgPool,
    policy_id: Option<&str>,
    input: SaveRoutePolicyInput,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    get_active_project(pool, &input.project_id).await?;

    let config_json = serde_json::to_value(&input.config).map_err(|error| {
        GatewayError::server_error(format!("serialize route policy config: {error}"))
    })?;
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let timestamp = OffsetDateTime::now_utc();

    let view = if let Some(policy_id) = policy_id {
        let updated = sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            update gateway_route_policies
            set
              project_id = $2,
              name = $3,
              is_default = $4,
              enabled = $5,
              config = $6,
              updated_at = $7
            where id = $1
            returning id, project_id, name, is_default, enabled, config, created_at, updated_at
            "#,
        )
        .bind(policy_id)
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(input.is_default)
        .bind(input.enabled)
        .bind(Json(config_json))
        .bind(timestamp)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("Route policy 不存在"))?;

        if updated.is_default {
            set_project_default_route_policy(&mut tx, &updated.project_id, &updated.id, timestamp)
                .await?;
        }
        route_policy_view_from_row(updated)?
    } else {
        let created = sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            insert into gateway_route_policies (
              id, project_id, name, is_default, enabled, config, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $7)
            returning id, project_id, name, is_default, enabled, config, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&input.project_id)
        .bind(&input.name)
        .bind(input.is_default)
        .bind(input.enabled)
        .bind(Json(config_json))
        .bind(timestamp)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        if created.is_default {
            set_project_default_route_policy(&mut tx, &created.project_id, &created.id, timestamp)
                .await?;
        }
        route_policy_view_from_row(created)?
    };

    bump_all_access_projection_versions(&mut tx, timestamp).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(view)
}

pub async fn list_model_aliases(
    pool: &PgPool,
    project_id: Option<&str>,
) -> Result<Vec<GatewayModelAliasView>, GatewayError> {
    let rows = if let Some(project_id) = project_id {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            where project_id = $1 or project_id is null
            order by alias asc, priority asc, weight desc, created_at asc
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    } else {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            order by alias asc, priority asc, weight desc, created_at asc
            "#,
        )
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    };

    Ok(rows.into_iter().map(model_alias_view_from_row).collect())
}

pub async fn save_model_alias(
    pool: &PgPool,
    alias_id: Option<&str>,
    input: SaveModelAliasInput,
) -> Result<GatewayModelAliasView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let timestamp = OffsetDateTime::now_utc();
    let project_id = input.project_id.as_deref();

    let row = if let Some(alias_id) = alias_id {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            update gateway_model_aliases
            set
              project_id = $2,
              scope_type = $3,
              alias = $4,
              provider_account_id = $5,
              upstream_model = $6,
              priority = $7,
              weight = $8,
              enabled = $9,
              updated_at = $10
            where id = $1
            returning id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            "#,
        )
        .bind(alias_id)
        .bind(project_id)
        .bind(&input.scope_type)
        .bind(&input.alias)
        .bind(&input.provider_account_id)
        .bind(input.upstream_model.as_deref())
        .bind(input.priority)
        .bind(input.weight)
        .bind(input.enabled)
        .bind(timestamp)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("模型关联不存在"))?
    } else {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            insert into gateway_model_aliases (
              id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            returning id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(project_id)
        .bind(&input.scope_type)
        .bind(&input.alias)
        .bind(&input.provider_account_id)
        .bind(input.upstream_model.as_deref())
        .bind(input.priority)
        .bind(input.weight)
        .bind(input.enabled)
        .bind(timestamp)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };

    bump_all_access_projection_versions(&mut tx, timestamp).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(model_alias_view_from_row(row))
}

pub async fn delete_model_alias(
    pool: &PgPool,
    alias_id: &str,
) -> Result<GatewayModelAliasView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let row = sqlx::query_as::<_, GatewayModelAliasRow>(
        r#"
        delete from gateway_model_aliases
        where id = $1
        returning id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
        "#,
    )
    .bind(alias_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("模型关联不存在"))?;

    bump_all_access_projection_versions(&mut tx, OffsetDateTime::now_utc()).await?;
    tx.commit().await.map_err(map_db_error)?;
    Ok(model_alias_view_from_row(row))
}

pub async fn list_models_for_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<ModelInfo>, GatewayError> {
    let project = get_active_project(pool, project_id).await?;
    let route_policy = get_route_policy_for_models(
        pool,
        &project.id,
        project.default_route_policy_id.as_deref(),
    )
    .await?;
    let alias_rows = sqlx::query_as::<_, GatewayModelAliasRow>(
        r#"
        select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
        from gateway_model_aliases
        where project_id = $1 or project_id is null
        order by alias asc, priority asc, weight desc, created_at asc
        "#,
    )
    .bind(&project.id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    let provider_rows = fetch_active_provider_rows(pool).await?;
    let provider_rows_by_id = provider_rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<HashMap<_, _>>();
    let provider_account_ids = provider_rows
        .iter()
        .map(|row| row.id.clone())
        .collect::<Vec<_>>();
    let capability_rows = if provider_account_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, ProviderCapabilityModelRow>(
            r#"
            select
              provider_account_id,
              model_code,
              upstream_model
            from gateway_provider_capability_catalog
            where enabled = true
              and provider_account_id = any($1)
            group by provider_account_id, model_code, upstream_model
            order by provider_account_id asc, model_code asc
            "#,
        )
        .bind(&provider_account_ids)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?
    };
    let shadowed_upstream_models = collect_shadowed_upstream_models_for_catalog(
        route_policy.as_ref().map(|policy| &policy.config),
        &alias_rows,
        &capability_rows,
        &provider_rows_by_id,
    );
    let mut model_ids = BTreeSet::new();

    for row in alias_rows {
        if row.enabled
            && route_policy_allows_models(
                route_policy.as_ref().map(|policy| &policy.config),
                [Some(row.alias.as_str()), row.upstream_model.as_deref()],
            )
        {
            model_ids.insert(row.alias);
        }
    }

    for row in capability_rows {
        let Some(provider_row) = provider_rows_by_id.get(row.provider_account_id.as_str()) else {
            continue;
        };
        if !provider_allowed_by_route_policy(
            provider_row,
            route_policy.as_ref().map(|policy| &policy.config),
        ) {
            continue;
        }
        if !route_policy_allows_models(
            route_policy.as_ref().map(|policy| &policy.config),
            [Some(row.model_code.as_str()), row.upstream_model.as_deref()],
        ) {
            continue;
        }
        let upstream_model = row
            .upstream_model
            .as_deref()
            .unwrap_or(row.model_code.as_str());
        if row.model_code == upstream_model
            && shadowed_upstream_models
                .contains(&(row.provider_account_id.clone(), upstream_model.to_string()))
        {
            continue;
        }
        model_ids.insert(row.model_code.clone());
    }

    for provider_row in provider_rows {
        if !provider_allowed_by_route_policy(
            &provider_row,
            route_policy.as_ref().map(|policy| &policy.config),
        ) {
            continue;
        }
        let Some(account_payload) = provider_payload_value_from_row(&provider_row).await? else {
            continue;
        };
        let Some(payload) = provider_payload_from_value(&provider_row, account_payload)? else {
            continue;
        };
        if let Some(default_model) = payload.default_model.as_deref() {
            if route_policy_allows_models(
                route_policy.as_ref().map(|policy| &policy.config),
                [Some(default_model), None],
            ) && !shadowed_upstream_models
                .contains(&(provider_row.id.clone(), default_model.to_string()))
            {
                model_ids.insert(default_model.to_string());
            }
        }
    }

    let created = OffsetDateTime::now_utc().unix_timestamp();
    Ok(model_ids
        .into_iter()
        .map(|id| ModelInfo {
            id,
            object: "model".to_string(),
            created,
            owned_by: "neuro-gateway".to_string(),
        })
        .collect())
}

pub async fn resolve_route_candidates(
    pool: &PgPool,
    project_id: &str,
    requested_model: Option<&str>,
    endpoint_kind: EndpointKind,
) -> Result<ResolvedProjectRouteContext, GatewayError> {
    let project = get_active_project(pool, project_id).await?;
    let route_policy = get_active_route_policy_for_requests(
        pool,
        &project.id,
        project.default_route_policy_id.as_deref(),
    )
    .await?;
    let alias_rows = fetch_alias_rows(pool, &project.id, requested_model).await?;
    let provider_rows = fetch_active_provider_rows(pool).await?;
    let eligible_provider_rows = provider_rows
        .into_iter()
        .filter(|row| provider_allowed_by_route_policy(row, Some(&route_policy.config)))
        .collect::<Vec<_>>();
    let provider_credentials = list_active_provider_credentials_for_accounts(
        pool,
        &eligible_provider_rows
            .iter()
            .map(|row| row.id.clone())
            .collect::<Vec<_>>(),
    )
    .await?;
    let provider_map = eligible_provider_rows
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<HashMap<_, _>>();
    let mut credential_map = HashMap::<String, Vec<super::GatewayProviderCredentialView>>::new();
    for credential in provider_credentials {
        credential_map
            .entry(credential.provider_account_id.clone())
            .or_default()
            .push(credential);
    }

    let mut candidates = Vec::new();
    for alias_row in alias_rows {
        let Some(provider_row) = provider_map.get(alias_row.provider_account_id.as_str()) else {
            continue;
        };
        if !provider_allowed_by_route_policy(provider_row, Some(&route_policy.config)) {
            continue;
        }
        if !route_policy_allows_models(
            Some(&route_policy.config),
            [
                Some(alias_row.alias.as_str()),
                alias_row.upstream_model.as_deref(),
            ],
        ) {
            continue;
        }
        candidates.extend(
            build_route_candidates_for_provider(
                provider_row,
                credential_map.get(provider_row.id.as_str()),
                endpoint_kind,
                Some(alias_row.alias.clone()),
                alias_row.upstream_model.clone(),
                -alias_row.priority,
                alias_row.weight.max(1),
            )
            .await?,
        );
    }

    if candidates.is_empty() {
        for provider_row in eligible_provider_rows {
            let provider_candidates = build_route_candidates_for_provider(
                &provider_row,
                credential_map.get(provider_row.id.as_str()),
                endpoint_kind,
                requested_model.map(str::to_string),
                None,
                -100,
                1,
            )
            .await?;
            for candidate in provider_candidates {
                let default_model = candidate
                    .payload
                    .default_model
                    .clone()
                    .or_else(|| requested_model.map(str::to_string));
                if !route_policy_allows_models(
                    Some(&route_policy.config),
                    [requested_model, default_model.as_deref()],
                ) {
                    continue;
                }
                let mut candidate = candidate;
                candidate.upstream_model = default_model;
                candidates.push(candidate);
            }
        }
    }

    if candidates.is_empty() {
        if requested_model.is_some()
            && route_policy_has_model_restrictions(Some(&route_policy.config))
        {
            return Err(GatewayError::conflict(format!(
                "当前 route policy 不允许模型 {}",
                requested_model.unwrap_or_default()
            )));
        }
        return Err(GatewayError::conflict(
            "当前网关没有可用的 provider account",
        ));
    }

    Ok(ResolvedProjectRouteContext {
        route_policy_id: route_policy.id,
        route_policy_config: route_policy.config.clone(),
        selection_strategy: queue_strategy_for_route_policy(&route_policy.config).to_string(),
        candidates,
    })
}

pub async fn find_active_route_policy_for_rate_limits(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<GatewayRoutePolicyView>, GatewayError> {
    let project = get_active_project(pool, project_id).await?;
    let route_policy = get_route_policy_for_models(
        pool,
        &project.id,
        project.default_route_policy_id.as_deref(),
    )
    .await?;
    Ok(route_policy.filter(|policy| policy.enabled))
}

fn normalize_selection_strategy(value: Option<&str>) -> String {
    match value.map(|raw| raw.trim().to_lowercase()) {
        Some(value) if value == "priority" => "priority".to_string(),
        _ => "weighted_random".to_string(),
    }
}

fn queue_strategy_for_route_policy(config: &GatewayRoutePolicyConfig) -> &'static str {
    if config.selection_strategy == "priority" {
        "round_robin"
    } else {
        "priority_weighted"
    }
}

fn normalize_non_negative_i32(value: Option<i32>) -> Result<Option<i32>, GatewayError> {
    match value {
        Some(value) if value < 0 => Err(GatewayError::bad_request("数值必须是非负整数")),
        other => Ok(other),
    }
}

fn normalize_rate_limit_enforcement_version(
    value: Option<&str>,
) -> Result<Option<String>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }
    if value != "v1" {
        return Err(
            GatewayError::bad_request("rateLimitEnforcementVersion 当前仅支持 v1")
                .with_code("rate_limit_enforcement_version_invalid"),
        );
    }
    Ok(Some(value))
}

fn normalize_positive_i32(
    label: &str,
    value: Option<i32>,
    max_value: i32,
) -> Result<Option<i32>, GatewayError> {
    match value {
        None => Ok(None),
        Some(value) if value <= 0 => {
            Err(GatewayError::bad_request(format!("{label} 必须是正整数")))
        }
        Some(value) if value > max_value => Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_value}"
        ))),
        Some(value) => Ok(Some(value)),
    }
}

fn normalize_positive_i64(
    label: &str,
    value: Option<i64>,
    max_value: i64,
) -> Result<Option<i64>, GatewayError> {
    match value {
        None => Ok(None),
        Some(value) if value <= 0 => {
            Err(GatewayError::bad_request(format!("{label} 必须是正整数")))
        }
        Some(value) if value > max_value => Err(GatewayError::bad_request(format!(
            "{label} 不能超过 {max_value}"
        ))),
        Some(value) => Ok(Some(value)),
    }
}

fn normalize_rate_limit_definition(
    label: &str,
    value: Option<GatewayRateLimitDefinitionInput>,
    fallback: Option<GatewayRateLimitDefinition>,
) -> Result<Option<GatewayRateLimitDefinition>, GatewayError> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    let window_seconds = normalize_positive_i32(
        &format!("{label}.windowSeconds"),
        value.window_seconds,
        86_400,
    )?;
    let max_requests = normalize_positive_i32(
        &format!("{label}.maxRequests"),
        value.max_requests,
        1_000_000,
    )?;
    match (window_seconds, max_requests) {
        (None, None) => Ok(fallback),
        (Some(window_seconds), Some(max_requests)) => Ok(Some(GatewayRateLimitDefinition {
            window_seconds,
            max_requests,
        })),
        _ => Err(GatewayError::bad_request(format!(
            "{label} 需要同时指定 windowSeconds 和 maxRequests"
        ))),
    }
}

fn normalize_rate_limit_map(
    label: &str,
    value: Option<HashMap<String, GatewayRateLimitDefinitionInput>>,
    fallback: Option<HashMap<String, GatewayRateLimitDefinition>>,
    normalize_key_lowercase: bool,
) -> Result<Option<HashMap<String, GatewayRateLimitDefinition>>, GatewayError> {
    let Some(value) = value else {
        return Ok(fallback);
    };
    if value.is_empty() {
        return Ok(fallback);
    }
    let mut normalized = HashMap::new();
    for (raw_key, definition) in value {
        let mut key = raw_key.trim().to_string();
        if normalize_key_lowercase {
            key = key.to_lowercase();
        }
        if key.is_empty() {
            return Err(GatewayError::bad_request(format!(
                "{label} 条目的 key 不能为空"
            )));
        }
        if let Some(entry) =
            normalize_rate_limit_definition(&format!("{label}.{key}"), Some(definition), None)?
        {
            normalized.insert(key, entry);
        }
    }
    Ok((!normalized.is_empty()).then_some(normalized))
}

fn normalize_string_list(values: Option<Vec<String>>, lower_case: bool) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values.unwrap_or_default() {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized_value = if lower_case {
            trimmed.to_lowercase()
        } else {
            trimmed.to_string()
        };
        if seen.insert(normalized_value.clone()) {
            normalized.push(normalized_value);
        }
    }
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_model_hint(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_ascii_lowercase())
}

fn collect_model_hint_values(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            for item in text.split([',', '\n']).filter_map(normalize_model_hint) {
                output.push(item);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_model_hint_values(item, output);
            }
        }
        _ => {}
    }
}

fn read_model_allow_list_from_payload(payload: &Value) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in [
        "supportedModels",
        "supported_models",
        "allowedModels",
        "allowed_models",
        "modelCode",
        "model_code",
        "modelId",
        "model_id",
        "model",
        "defaultModel",
        "default_model",
    ] {
        if let Some(value) = object.get(key) {
            collect_model_hint_values(value, &mut values);
        }
    }
    values.sort();
    values.dedup();
    values
}

fn read_model_block_list_from_payload(payload: &Value) -> Vec<String> {
    let Some(object) = payload.as_object() else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for key in [
        "excludedModels",
        "excluded_models",
        "blockedModels",
        "blocked_models",
    ] {
        if let Some(value) = object.get(key) {
            collect_model_hint_values(value, &mut values);
        }
    }
    values.sort();
    values.dedup();
    values
}

fn credential_payload_supports_model(
    credential_payload: &Value,
    model_alias: Option<&str>,
    upstream_model: Option<&str>,
) -> bool {
    let model_candidates = [model_alias, upstream_model]
        .into_iter()
        .flatten()
        .filter_map(normalize_model_hint)
        .collect::<Vec<_>>();
    if model_candidates.is_empty() {
        return true;
    }

    let block_list = read_model_block_list_from_payload(credential_payload);
    if model_candidates
        .iter()
        .any(|candidate| block_list.iter().any(|blocked| blocked == candidate))
    {
        return false;
    }

    let allow_list = read_model_allow_list_from_payload(credential_payload);
    if allow_list.is_empty() {
        return true;
    }

    model_candidates
        .iter()
        .any(|candidate| allow_list.iter().any(|allowed| allowed == candidate))
}

fn parse_route_policy_config(value: &Value) -> Result<GatewayRoutePolicyConfig, GatewayError> {
    serde_json::from_value::<GatewayRoutePolicyConfig>(value.clone()).map_err(|_| {
        GatewayError::server_error("persisted route policy config is invalid")
            .with_code("route_policy_config_invalid")
    })
}

fn route_policy_view_from_row(
    row: GatewayRoutePolicyRow,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    Ok(GatewayRoutePolicyView {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        is_default: row.is_default,
        enabled: row.enabled,
        config: parse_route_policy_config(&row.config.0)?,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

fn model_alias_view_from_row(row: GatewayModelAliasRow) -> GatewayModelAliasView {
    GatewayModelAliasView {
        id: row.id,
        project_id: row.project_id,
        scope_type: row.scope_type,
        alias: row.alias,
        provider_account_id: row.provider_account_id,
        upstream_model: row.upstream_model,
        priority: row.priority,
        weight: row.weight,
        enabled: row.enabled,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

async fn set_project_default_route_policy(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: &str,
    route_policy_id: &str,
    timestamp: OffsetDateTime,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_projects
        set default_route_policy_id = $2, updated_at = $3
        where id = $1
        "#,
    )
    .bind(project_id)
    .bind(route_policy_id)
    .bind(timestamp)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        update gateway_route_policies
        set is_default = case when id = $2 then true else false end,
            updated_at = $3
        where project_id = $1
        "#,
    )
    .bind(project_id)
    .bind(route_policy_id)
    .bind(timestamp)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;

    Ok(())
}

async fn get_route_policy_for_models(
    pool: &PgPool,
    project_id: &str,
    default_route_policy_id: Option<&str>,
) -> Result<Option<GatewayRoutePolicyView>, GatewayError> {
    let row = if let Some(default_route_policy_id) = default_route_policy_id {
        sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            select id, project_id, name, is_default, enabled, config, created_at, updated_at
            from gateway_route_policies
            where id = $1
            limit 1
            "#,
        )
        .bind(default_route_policy_id)
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?
    } else {
        sqlx::query_as::<_, GatewayRoutePolicyRow>(
            r#"
            select id, project_id, name, is_default, enabled, config, created_at, updated_at
            from gateway_route_policies
            where project_id = $1 and is_default = true and enabled = true
            limit 1
            "#,
        )
        .bind(project_id)
        .fetch_optional(pool)
        .await
        .map_err(map_db_error)?
    };

    row.map(route_policy_view_from_row).transpose()
}

async fn get_active_route_policy_for_requests(
    pool: &PgPool,
    project_id: &str,
    default_route_policy_id: Option<&str>,
) -> Result<GatewayRoutePolicyView, GatewayError> {
    let route_policy = get_route_policy_for_models(pool, project_id, default_route_policy_id)
        .await?
        .ok_or_else(|| {
            GatewayError::conflict("当前 project 尚未配置可用的 AI gateway route policy")
        })?;
    if !route_policy.enabled {
        return Err(GatewayError::conflict(
            "当前 project 尚未配置可用的 AI gateway route policy",
        ));
    }
    Ok(route_policy)
}

async fn fetch_alias_rows(
    pool: &PgPool,
    project_id: &str,
    requested_model: Option<&str>,
) -> Result<Vec<GatewayModelAliasRow>, GatewayError> {
    if let Some(requested_model) = requested_model {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            where enabled = true
              and alias = $2
              and (project_id = $1 or project_id is null)
            order by priority asc, weight desc, created_at asc
            "#,
        )
        .bind(project_id)
        .bind(requested_model)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
    } else {
        sqlx::query_as::<_, GatewayModelAliasRow>(
            r#"
            select id, project_id, scope_type, alias, provider_account_id, upstream_model, priority, weight, enabled, created_at, updated_at
            from gateway_model_aliases
            where enabled = true
              and (project_id = $1 or project_id is null)
            order by priority asc, weight desc, created_at asc
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)
    }
}

async fn fetch_active_provider_rows(
    pool: &PgPool,
) -> Result<Vec<GatewayProviderRouteRow>, GatewayError> {
    recover_expired_cooling_provider_accounts(pool).await?;
    sqlx::query_as::<_, GatewayProviderRouteRow>(
        r#"
        select
          id,
          label,
          adapter,
          protocol_family,
          protocol_profile,
          cooldown_until,
          failure_count,
          execution_mode,
          endpoint_execution_modes,
          payload_inline,
          payload_object_key
        from gateway_provider_accounts
        where status = 'active'
          and (cooldown_until is null or cooldown_until <= now())
        order by created_at asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

async fn provider_payload_value_from_row(
    row: &GatewayProviderRouteRow,
) -> Result<Option<Value>, GatewayError> {
    let payload_value = match (&row.payload_inline, row.payload_object_key.as_deref()) {
        (Some(payload_inline), _) => payload_inline.0.clone(),
        (None, Some(object_key)) => gateway_object_storage()?.read_json(object_key).await?,
        (None, None) => return Ok(None),
    };
    Ok(Some(payload_value))
}

fn provider_payload_from_value(
    row: &GatewayProviderRouteRow,
    payload_value: Value,
) -> Result<Option<ProviderAccountPayload>, GatewayError> {
    let mut payload: ProviderAccountPayload = match serde_json::from_value(payload_value) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(
                provider_account_id = %row.id,
                error = %error,
                "skip malformed provider payload while building gateway model and route catalog"
            );
            return Ok(None);
        }
    };
    payload.adapter = canonicalize_adapter_name(&row.adapter);
    payload.execution_mode = parse_execution_mode(&row.execution_mode).or(payload.execution_mode);
    if let Some(endpoint_execution_modes) = &row.endpoint_execution_modes {
        payload.endpoint_execution_modes = Some(
            serde_json::from_value(endpoint_execution_modes.0.clone()).map_err(|error| {
                GatewayError::server_error(format!(
                    "deserialize endpoint execution modes for {}: {error}",
                    row.id
                ))
            })?,
        );
    }
    Ok(Some(payload))
}

async fn build_route_candidates_for_provider(
    provider_row: &GatewayProviderRouteRow,
    credentials: Option<&Vec<super::GatewayProviderCredentialView>>,
    endpoint_kind: EndpointKind,
    model_alias: Option<String>,
    upstream_model: Option<String>,
    priority: i32,
    weight: i32,
) -> Result<Vec<RouteCandidate>, GatewayError> {
    let Some(account_payload) = provider_payload_value_from_row(provider_row).await? else {
        return Ok(Vec::new());
    };
    let mut candidates = Vec::new();
    if let Some(credentials) = credentials {
        for credential in credentials {
            if !credential_payload_supports_model(
                &credential.payload,
                model_alias.as_deref(),
                upstream_model.as_deref(),
            ) {
                continue;
            }
            let merged_payload = merge_provider_account_and_credential_payloads(
                &account_payload,
                &credential.payload,
            );
            let supported_protocol_families = resolve_supported_wire_protocol_families_for_model(
                Some(&merged_payload),
                model_alias.as_deref(),
                upstream_model.as_deref(),
                &provider_row.adapter,
                &provider_row.protocol_family,
            );
            let Some(mut payload) = provider_payload_from_value(provider_row, merged_payload)?
            else {
                continue;
            };
            payload.credential_id = Some(credential.id.clone());
            candidates.push(RouteCandidate {
                provider_account_id: provider_row.id.clone(),
                provider_credential_id: Some(credential.id.clone()),
                label: format!("{} / {}", provider_row.label, credential.label),
                payload: payload.clone(),
                protocol_family: canonicalize_protocol_family_name(&provider_row.protocol_family),
                protocol_profile: provider_row.protocol_profile.clone(),
                supported_protocol_families,
                adapter: canonicalize_adapter_name(&provider_row.adapter),
                model_alias: model_alias.clone(),
                upstream_model: upstream_model.clone(),
                resolved_execution_mode: payload.resolve_execution_mode(endpoint_kind),
                priority,
                weight,
                failure_count: credential.failure_count.max(0) as u32,
                cooldown_until: credential.cooldown_until.clone(),
                routing_score: None,
                routing_health_weight: None,
                routing_capacity_weight: None,
                routing_degraded: None,
                routing_breaker_open: None,
                routing_degradation_reasons: Vec::new(),
            });
        }
        return Ok(candidates);
    }

    let supported_protocol_families = resolve_supported_wire_protocol_families_for_model(
        Some(&account_payload),
        model_alias.as_deref(),
        upstream_model.as_deref(),
        &provider_row.adapter,
        &provider_row.protocol_family,
    );
    let Some(payload) = provider_payload_from_value(provider_row, account_payload)? else {
        return Ok(Vec::new());
    };
    candidates.push(RouteCandidate {
        provider_account_id: provider_row.id.clone(),
        provider_credential_id: None,
        label: provider_row.label.clone(),
        payload: payload.clone(),
        protocol_family: canonicalize_protocol_family_name(&provider_row.protocol_family),
        protocol_profile: provider_row.protocol_profile.clone(),
        supported_protocol_families,
        adapter: canonicalize_adapter_name(&provider_row.adapter),
        model_alias,
        upstream_model,
        resolved_execution_mode: payload.resolve_execution_mode(endpoint_kind),
        priority,
        weight,
        failure_count: provider_row.failure_count.max(0) as u32,
        cooldown_until: provider_row.cooldown_until.map(format_timestamp),
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: Vec::new(),
    });
    Ok(candidates)
}

fn parse_execution_mode(value: &str) -> Option<ProviderExecutionMode> {
    match value.trim().to_lowercase().as_str() {
        "browser_backed" => Some(ProviderExecutionMode::BrowserBacked),
        "direct_http" => Some(ProviderExecutionMode::DirectHttp),
        _ => None,
    }
}

fn provider_allowed_by_route_policy(
    provider_account: &GatewayProviderRouteRow,
    route_policy: Option<&GatewayRoutePolicyConfig>,
) -> bool {
    let allowed_provider_ids = normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_provider_account_ids.clone()),
        false,
    );
    if let Some(allowed_provider_ids) = allowed_provider_ids {
        if !allowed_provider_ids
            .iter()
            .any(|value| value == &provider_account.id)
        {
            return false;
        }
    }

    let allowed_protocol_families = normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_protocol_families.clone()),
        true,
    );
    if let Some(allowed_protocol_families) = allowed_protocol_families {
        if !allowed_protocol_families.iter().any(|value| {
            route_policy_family_matches_surface(
                value,
                &provider_account.adapter,
                &provider_account.protocol_family,
            )
        }) {
            return false;
        }
    }

    true
}

fn route_policy_has_model_restrictions(route_policy: Option<&GatewayRoutePolicyConfig>) -> bool {
    normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_model_ids.clone()),
        true,
    )
    .is_some()
        || normalize_string_list(
            route_policy.and_then(|policy| policy.blocked_model_ids.clone()),
            true,
        )
        .is_some()
}

fn route_policy_allows_models<'a>(
    route_policy: Option<&GatewayRoutePolicyConfig>,
    candidate_model_ids: [Option<&'a str>; 2],
) -> bool {
    let blocked_model_ids = normalize_string_list(
        route_policy.and_then(|policy| policy.blocked_model_ids.clone()),
        true,
    );
    let normalized_candidates = candidate_model_ids
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if let Some(blocked_model_ids) = blocked_model_ids {
        if normalized_candidates
            .iter()
            .any(|value| blocked_model_ids.iter().any(|blocked| blocked == value))
        {
            return false;
        }
    }

    let allowed_model_ids = normalize_string_list(
        route_policy.and_then(|policy| policy.allowed_model_ids.clone()),
        true,
    );
    let Some(allowed_model_ids) = allowed_model_ids else {
        return true;
    };
    if normalized_candidates.is_empty() {
        return false;
    }
    normalized_candidates
        .iter()
        .any(|value| allowed_model_ids.iter().any(|allowed| allowed == value))
}

fn collect_shadowed_upstream_models_for_catalog(
    route_policy: Option<&GatewayRoutePolicyConfig>,
    alias_rows: &[GatewayModelAliasRow],
    capability_rows: &[ProviderCapabilityModelRow],
    provider_rows_by_id: &HashMap<&str, &GatewayProviderRouteRow>,
) -> HashSet<(String, String)> {
    let mut shadowed = HashSet::new();

    for row in alias_rows {
        let Some(upstream_model) = row.upstream_model.as_deref() else {
            continue;
        };
        if !row.enabled || row.alias == upstream_model {
            continue;
        }
        if !route_policy_allows_models(
            route_policy,
            [Some(row.alias.as_str()), Some(upstream_model)],
        ) {
            continue;
        }
        shadowed.insert((row.provider_account_id.clone(), upstream_model.to_string()));
    }

    for row in capability_rows {
        let Some(provider_row) = provider_rows_by_id.get(row.provider_account_id.as_str()) else {
            continue;
        };
        let Some(upstream_model) = row.upstream_model.as_deref() else {
            continue;
        };
        if row.model_code == upstream_model {
            continue;
        }
        if !provider_allowed_by_route_policy(provider_row, route_policy) {
            continue;
        }
        if !route_policy_allows_models(
            route_policy,
            [Some(row.model_code.as_str()), Some(upstream_model)],
        ) {
            continue;
        }
        shadowed.insert((row.provider_account_id.clone(), upstream_model.to_string()));
    }

    shadowed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_model_catalog_ids_for_tests(
        route_policy: Option<&GatewayRoutePolicyConfig>,
        alias_rows: Vec<GatewayModelAliasRow>,
        provider_rows: Vec<GatewayProviderRouteRow>,
        capability_rows: Vec<ProviderCapabilityModelRow>,
        provider_default_models: HashMap<String, Option<String>>,
    ) -> Vec<String> {
        let provider_rows_by_id = provider_rows
            .iter()
            .map(|row| (row.id.as_str(), row))
            .collect::<HashMap<_, _>>();
        let shadowed_upstream_models = collect_shadowed_upstream_models_for_catalog(
            route_policy,
            &alias_rows,
            &capability_rows,
            &provider_rows_by_id,
        );
        let mut model_ids = BTreeSet::new();

        for row in alias_rows {
            if row.enabled
                && route_policy_allows_models(
                    route_policy,
                    [Some(row.alias.as_str()), row.upstream_model.as_deref()],
                )
            {
                model_ids.insert(row.alias);
            }
        }

        for row in capability_rows {
            let Some(provider_row) = provider_rows_by_id.get(row.provider_account_id.as_str())
            else {
                continue;
            };
            if !provider_allowed_by_route_policy(provider_row, route_policy) {
                continue;
            }
            if !route_policy_allows_models(
                route_policy,
                [Some(row.model_code.as_str()), row.upstream_model.as_deref()],
            ) {
                continue;
            }
            let upstream_model = row
                .upstream_model
                .as_deref()
                .unwrap_or(row.model_code.as_str());
            if row.model_code == upstream_model
                && shadowed_upstream_models
                    .contains(&(row.provider_account_id.clone(), upstream_model.to_string()))
            {
                continue;
            }
            model_ids.insert(row.model_code.clone());
        }

        for provider_row in provider_rows {
            if !provider_allowed_by_route_policy(&provider_row, route_policy) {
                continue;
            }
            let Some(Some(default_model)) = provider_default_models.get(provider_row.id.as_str())
            else {
                continue;
            };
            if route_policy_allows_models(route_policy, [Some(default_model.as_str()), None])
                && !shadowed_upstream_models
                    .contains(&(provider_row.id.clone(), default_model.to_string()))
            {
                model_ids.insert(default_model.clone());
            }
        }

        model_ids.into_iter().collect()
    }

    fn make_provider_row(id: &str, protocol_family: &str) -> GatewayProviderRouteRow {
        GatewayProviderRouteRow {
            id: id.to_string(),
            label: id.to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: protocol_family.to_string(),
            protocol_profile: "openai".to_string(),
            cooldown_until: None,
            failure_count: 0,
            execution_mode: "direct_http".to_string(),
            endpoint_execution_modes: None,
            payload_inline: None,
            payload_object_key: None,
        }
    }

    #[test]
    fn normalize_route_policy_defaults_match_rust_baseline() {
        let config =
            normalize_route_policy_config(GatewayRoutePolicyConfigInput::default()).unwrap();
        assert_eq!(config.rate_limit_enforcement_version, None);
        assert_eq!(config.selection_strategy, "weighted_random");
        assert_eq!(config.circuit_breaker_threshold, 3);
        assert_eq!(
            config.fallback_http_statuses,
            vec![408, 425, 429, 500, 502, 503, 504]
        );
    }

    #[test]
    fn malformed_route_policy_config_is_rejected_instead_of_defaulted() {
        let error = parse_route_policy_config(&serde_json::json!({
            "rateLimitEnforcementVersion": ["v1"]
        }))
        .expect_err("malformed persisted route policy must not silently use defaults");

        assert_eq!(error.code.as_deref(), Some("route_policy_config_invalid"));
    }

    #[test]
    fn route_policy_blocks_disallowed_model() {
        let config = GatewayRoutePolicyConfig {
            allowed_model_ids: Some(vec!["gpt-4o".to_string()]),
            ..GatewayRoutePolicyConfig::default()
        };
        assert!(route_policy_allows_models(
            Some(&config),
            [Some("gpt-4o"), None]
        ));
        assert!(!route_policy_allows_models(
            Some(&config),
            [Some("claude-3-7"), None]
        ));
    }

    #[test]
    fn route_policy_filters_provider_and_protocol() {
        let row = GatewayProviderRouteRow {
            id: "provider-a".to_string(),
            label: "Provider A".to_string(),
            adapter: "openai_compatible".to_string(),
            protocol_family: "openai".to_string(),
            protocol_profile: "openai".to_string(),
            cooldown_until: None,
            failure_count: 0,
            execution_mode: "direct_http".to_string(),
            endpoint_execution_modes: None,
            payload_inline: None,
            payload_object_key: None,
        };
        let config = GatewayRoutePolicyConfig {
            allowed_provider_account_ids: Some(vec!["provider-a".to_string()]),
            allowed_protocol_families: Some(vec!["openai".to_string()]),
            ..GatewayRoutePolicyConfig::default()
        };
        assert!(provider_allowed_by_route_policy(&row, Some(&config)));
    }

    #[test]
    fn credential_model_allow_list_matches_alias_or_upstream_model() {
        let payload = serde_json::json!({
            "supportedModels": ["qwen3.5-35b-a3b", "astron-code-latest"]
        });
        assert!(credential_payload_supports_model(
            &payload,
            Some("qwen3.5-35b-a3b"),
            Some("astron-code-latest"),
        ));
        assert!(!credential_payload_supports_model(
            &payload,
            Some("qwen3.5-2b"),
            Some("xop35qwen2b"),
        ));
    }

    #[test]
    fn credential_model_block_list_overrides_allow_list() {
        let payload = serde_json::json!({
            "supportedModels": ["qwen3.5-35b-a3b"],
            "excludedModels": ["astron-code-latest"]
        });
        assert!(!credential_payload_supports_model(
            &payload,
            Some("qwen3.5-35b-a3b"),
            Some("astron-code-latest"),
        ));
    }

    #[test]
    fn model_catalog_prefers_platform_model_code_over_upstream_default_model() {
        let alias_rows = vec![GatewayModelAliasRow {
            id: "alias-1".to_string(),
            project_id: Some("project-1".to_string()),
            scope_type: "project".to_string(),
            alias: "qwen3.5-2b-xfyun-ws".to_string(),
            provider_account_id: "provider-xfyun-ws".to_string(),
            upstream_model: Some("xop35qwen2b".to_string()),
            priority: 0,
            weight: 1,
            enabled: true,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }];
        let capability_rows = vec![ProviderCapabilityModelRow {
            provider_account_id: "provider-xfyun-ws".to_string(),
            model_code: "qwen3.5-2b-xfyun-ws".to_string(),
            upstream_model: Some("xop35qwen2b".to_string()),
        }];
        let provider_rows = vec![make_provider_row("provider-xfyun-ws", "xfyun_websocket")];
        let default_models = HashMap::from([(
            "provider-xfyun-ws".to_string(),
            Some("xop35qwen2b".to_string()),
        )]);

        let model_ids = collect_model_catalog_ids_for_tests(
            None,
            alias_rows,
            provider_rows,
            capability_rows,
            default_models,
        );

        assert!(model_ids.contains(&"qwen3.5-2b-xfyun-ws".to_string()));
        assert!(!model_ids.contains(&"xop35qwen2b".to_string()));
    }

    #[test]
    fn model_catalog_hides_upstream_capability_id_when_platform_alias_exists() {
        let alias_rows = vec![GatewayModelAliasRow {
            id: "alias-1".to_string(),
            project_id: Some("project-1".to_string()),
            scope_type: "project".to_string(),
            alias: "nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string(),
            provider_account_id: "provider-nvidia".to_string(),
            upstream_model: Some("nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string()),
            priority: 0,
            weight: 1,
            enabled: true,
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }];
        let capability_rows = vec![
            ProviderCapabilityModelRow {
                provider_account_id: "provider-nvidia".to_string(),
                model_code: "nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string(),
                upstream_model: Some("nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string()),
            },
            ProviderCapabilityModelRow {
                provider_account_id: "provider-nvidia".to_string(),
                model_code: "nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string(),
                upstream_model: Some("nvidia/llama-3.3-nemotron-super-49b-v1.5".to_string()),
            },
        ];
        let provider_rows = vec![make_provider_row("provider-nvidia", "openai")];

        let model_ids = collect_model_catalog_ids_for_tests(
            None,
            alias_rows,
            provider_rows,
            capability_rows,
            HashMap::new(),
        );

        assert_eq!(
            model_ids,
            vec!["nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string()]
        );
    }

    #[test]
    fn model_catalog_falls_back_to_provider_default_when_no_platform_model_code_exists() {
        let provider_rows = vec![make_provider_row("provider-openai", "openai")];
        let default_models =
            HashMap::from([("provider-openai".to_string(), Some("gpt-5.4".to_string()))]);

        let model_ids = collect_model_catalog_ids_for_tests(
            None,
            Vec::new(),
            provider_rows,
            Vec::new(),
            default_models,
        );

        assert_eq!(model_ids, vec!["gpt-5.4".to_string()]);
    }
}
