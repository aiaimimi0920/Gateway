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
use super::GatewayProviderCredentialView;
use super::{
    format_timestamp, get_active_project, list_active_provider_credentials_for_accounts,
    map_db_error, merge_provider_account_and_credential_payloads,
    provider_accounts::recover_expired_cooling_provider_accounts,
};

mod aliases;
mod candidates;
mod catalog;
mod model_filters;
mod normalization;
mod policies;
mod providers;

pub use aliases::{delete_model_alias, list_model_aliases, save_model_alias};
pub use candidates::{resolve_route_candidates, resolve_route_candidates_allowing_empty};
pub use catalog::list_models_for_project;
pub use normalization::normalize_route_policy_config;
pub use policies::{
    find_active_route_policy_for_rate_limits, list_route_policies, save_route_policy,
};

use aliases::fetch_alias_rows;
use model_filters::{
    credential_payload_supports_model, provider_allowed_by_route_policy,
    route_policy_allows_models, route_policy_has_model_restrictions,
};
use normalization::{normalize_string_list, parse_route_policy_config};
use policies::{get_active_route_policy_for_requests, get_route_policy_for_models};
use providers::{
    build_route_candidates_for_provider, fetch_active_provider_rows, provider_payload_from_value,
    provider_payload_value_from_row,
};

#[cfg(test)]
use catalog::collect_shadowed_upstream_models_for_catalog;

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

#[cfg(test)]
mod tests;
