use std::collections::{BTreeMap, BTreeSet, HashMap};

use deadpool_redis::Pool as RedisPool;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Postgres, Row, Transaction};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::access_control::{ensure_allowed_resource_ids, AccessBoundary};
use crate::error::GatewayError;
use crate::gateway_api_key::{
    build_gateway_project_api_key_with_prefix,
    normalize_bundle_scoped_gateway_project_api_key_prefix,
};
use crate::protocol::canonical::EndpointKind;
use crate::routing::candidate::{
    canonicalize_adapter_name, canonicalize_protocol_family_name, ProviderExecutionMode,
    RouteCandidate,
};
use crate::routing::config::ModelInfo;
use crate::routing::protocol_resolution::resolve_supported_wire_protocol_families_for_model;

use super::{
    format_timestamp, get_provider_payload, list_active_provider_credentials_for_accounts,
    map_db_error, merge_provider_account_and_credential_payloads, GatewayProviderCredentialView,
};

mod auth;
mod balance;
mod balance_adjustment;
pub(crate) mod balance_store;
mod bundle_compat;
mod bundles;
mod candidates;
mod catalog;
mod catalog_write;
mod key_rotation;
mod keys;
mod memberships;
mod models;
mod projection;
mod projection_queries;
mod route_context;
mod route_filter;
mod sticky;
mod views;

pub use auth::{
    find_access_key_auth_by_external_key, find_access_key_auth_by_id, scope_list_from_metadata,
    touch_access_key_last_used, validate_access_key_auth,
};
pub use balance::{
    evaluate_access_key_balance, get_access_key_balance, pre_deduct_access_key_balance,
    refund_access_key_balance, settle_access_key_balance,
};
pub use balance_adjustment::adjust_access_key_balance;
pub use bundles::{
    delete_access_bundle, ensure_default_access_bundle_for_project, replace_access_bundle_items,
    save_access_bundle,
};
pub use candidates::{preview_access_candidates, preview_route_decision};
pub use catalog::list_access_catalog;
pub use catalog_write::{save_platform_access, save_provider_capability};
pub use key_rotation::rotate_access_key;
pub use keys::{
    delete_access_key, read_access_key_secret, revoke_access_key, save_access_key,
    save_access_key_with_quota, set_access_key_enabled,
};
pub use memberships::replace_access_key_aggregate_memberships;
pub use models::{
    AccessBalanceDecision, AccessKeyAuthRecord, AccessKeyBalanceAdjustInput,
    AggregateMembershipInput, DeleteAccessBundleResult, DeleteAccessKeyResult,
    GatewayAccessBundleItemView, GatewayAccessBundleView, GatewayAccessCandidatePreviewView,
    GatewayAccessCatalogView, GatewayAccessKeyAggregateMembershipView, GatewayAccessKeyBalanceView,
    GatewayAccessKeyBundleBindingView, GatewayAccessKeyView, GatewayAccessRouteDecisionPreviewView,
    GatewayAccessStickyAffinityView, GatewayPlatformAccessView, GatewayProviderCapabilityView,
    ProjectedPlatformAccessRow, ResolvedAccessKeyRouteContext, UpsertAccessBundleInput,
    UpsertAccessKeyInput, UpsertPlatformAccessInput, UpsertProviderCapabilityInput,
};
pub use projection::list_models_for_access_key;
pub use route_context::resolve_access_key_route_context;
pub(crate) use route_context::resolve_access_key_route_context_with_reservation;
pub use sticky::{
    inspect_access_sticky_affinity, record_access_sticky_affinity, reset_access_sticky_affinity,
};

#[cfg(test)]
use projection::{
    cache_projection_is_compatible, collect_model_catalog_ids_from_access_projection,
    expand_projection_rows_with_aliases, group_projection_rows_by_model,
};
#[cfg(test)]
use projection_queries::build_projection_rows_for_normal_key;
#[cfg(test)]
use route_filter::credential_payload_supports_route_row;

#[derive(Debug, Clone, FromRow)]
struct ProviderCapabilityRow {
    id: String,
    provider_account_id: String,
    model_code: String,
    endpoint_kind: String,
    upstream_model: Option<String>,
    enabled: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct PlatformAccessRow {
    id: String,
    provider_capability_id: String,
    provider_account_id: String,
    model_code: String,
    endpoint_kind: String,
    upstream_model: Option<String>,
    platform_tier: String,
    status: String,
    operator_weight: i32,
    routing_priority: i32,
    enabled_for_sale: bool,
    notes: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AccessBundleRow {
    id: String,
    project_id: Option<String>,
    slug: String,
    display_name: String,
    billing_mode: String,
    status: String,
    description: Option<String>,
    metadata: Option<Json<Value>>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AccessBundleItemRow {
    bundle_id: String,
    platform_access_id: String,
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AccessKeyBundleBindingRow {
    access_key_id: String,
    bundle_id: String,
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AccessKeyRow {
    id: String,
    owner_type: String,
    owner_id: String,
    resolved_project_id: String,
    resolved_tenant_id: String,
    key_kind: String,
    status: String,
    public_key_prefix: String,
    display_name: String,
    external_key: Option<String>,
    rotated_from_access_key_id: Option<String>,
    legacy_gateway_api_key_id: Option<String>,
    legacy_user_credential_id: Option<String>,
    expires_at: Option<OffsetDateTime>,
    last_used_at: Option<OffsetDateTime>,
    metadata: Option<Json<Value>>,
    revoked_at: Option<OffsetDateTime>,
    revoke_reason: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AccessKeyBalanceRow {
    access_key_id: String,
    balance_mode: String,
    status: String,
    unlimited_until: Option<OffsetDateTime>,
    period_starts_at: Option<OffsetDateTime>,
    period_ends_at: Option<OffsetDateTime>,
    total_tokens: Option<i64>,
    remaining_tokens: Option<i64>,
    total_messages: Option<i64>,
    remaining_messages: Option<i64>,
    updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct AggregateMembershipRow {
    aggregate_access_key_id: String,
    member_access_key_id: String,
    priority: i32,
    created_at: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow)]
struct ProviderCandidateMetaRow {
    id: String,
    label: String,
    adapter: String,
    protocol_family: String,
    protocol_profile: String,
    cooldown_until: Option<OffsetDateTime>,
    failure_count: i32,
    execution_mode: String,
}

fn endpoint_kind_name(endpoint_kind: EndpointKind) -> &'static str {
    match endpoint_kind {
        EndpointKind::ChatCompletions => "chat_completions",
        EndpointKind::Completions => "completions",
        EndpointKind::Embeddings => "embeddings",
        EndpointKind::ImagesGenerations => "images_generations",
        EndpointKind::ImagesEdits => "images_edits",
        EndpointKind::MusicGenerations => "music_generations",
        EndpointKind::VideosGenerations => "videos_generations",
        EndpointKind::AudioTranscriptions => "audio_transcriptions",
        EndpointKind::AudioSpeech => "audio_speech",
        EndpointKind::Messages => "messages",
        EndpointKind::Responses => "responses",
        EndpointKind::Search => "search",
        EndpointKind::Fetch => "fetch",
        EndpointKind::ResearchCreate => "research_create",
        EndpointKind::ResearchList => "research_list",
        EndpointKind::ResearchGet => "research_get",
        EndpointKind::CreditsBalance => "credits_balance",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedAccessProjection {
    schema_version: u16,
    access_key_id: String,
    key_kind: String,
    resolved_tenant_id: String,
    resolved_project_id: String,
    projection_version: String,
    rows_by_model: BTreeMap<String, Vec<ProjectedPlatformAccessRow>>,
}

#[derive(Debug, Clone, FromRow)]
struct ModelAliasProjectionRow {
    alias: String,
    provider_account_id: String,
    upstream_model: Option<String>,
}

fn parse_optional_timestamp(value: Option<&str>) -> Option<OffsetDateTime> {
    value.and_then(|raw| OffsetDateTime::parse(raw, &Rfc3339).ok())
}

fn now_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn normalize_platform_tier(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "high" => "high",
        "mid" | "medium" => "mid",
        _ => "low",
    }
}

fn platform_tier_rank(value: &str) -> i32 {
    match normalize_platform_tier(value) {
        "high" => 3,
        "mid" => 2,
        _ => 1,
    }
}

fn normalize_status(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "active".to_string()
    } else {
        normalized
    }
}

fn normalize_balance_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "time_pass" => "time_pass".to_string(),
        "message_prepaid" | "request_prepaid" => "message_prepaid".to_string(),
        _ => "token_prepaid".to_string(),
    }
}

fn normalize_key_kind(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto_route" => "auto_route".to_string(),
        _ => "normal".to_string(),
    }
}

/// Advance the database-side projection version before committing a mutation
/// that can change any access-key route. Redis invalidation is only an
/// optimization; readers compare this version and rebuild when Redis cleanup
/// is unavailable or a stale entry survives.
pub(crate) async fn bump_all_access_projection_versions(
    tx: &mut Transaction<'_, Postgres>,
    version: OffsetDateTime,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_access_keys
        set updated_at = greatest(updated_at + interval '1 microsecond', $1)
        "#,
    )
    .bind(version)
    .execute(&mut **tx)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

#[cfg(test)]
mod tests;
