use std::collections::{BTreeMap, BTreeSet, HashMap};

use deadpool_redis::Pool as RedisPool;
use redis::AsyncCommands;
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
use crate::redis::keys;
use crate::routing::candidate::{
    canonicalize_adapter_name, canonicalize_protocol_family_name, ProviderExecutionMode,
    RouteCandidate,
};
use crate::routing::config::ModelInfo;
use crate::routing::protocol_resolution::resolve_supported_wire_protocol_families_for_model;

use super::{
    format_timestamp, get_provider_payload, list_active_provider_credentials_for_accounts,
    map_db_error, merge_provider_account_and_credential_payloads,
};

const ACCESS_AFFINITY_TTL_SECS: u64 = 3600;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProviderCapabilityView {
    pub id: String,
    pub provider_account_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayPlatformAccessView {
    pub id: String,
    pub provider_capability_id: String,
    pub provider_account_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub platform_tier: String,
    pub status: String,
    pub operator_weight: i32,
    pub routing_priority: i32,
    pub enabled_for_sale: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessBundleView {
    pub id: String,
    pub project_id: Option<String>,
    pub slug: String,
    pub display_name: String,
    pub billing_mode: String,
    pub status: String,
    pub description: Option<String>,
    pub metadata: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessBundleItemView {
    pub bundle_id: String,
    pub platform_access_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccessBundleResult {
    pub bundle_id: String,
    pub display_name: String,
    pub deleted_platform_key_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccessKeyResult {
    pub access_key_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessKeyBundleBindingView {
    pub access_key_id: String,
    pub bundle_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessKeyView {
    pub id: String,
    pub owner_type: String,
    pub owner_id: String,
    pub resolved_project_id: String,
    pub resolved_tenant_id: String,
    pub key_kind: String,
    pub status: String,
    pub public_key_prefix: String,
    pub display_name: String,
    pub token: Option<String>,
    pub external_key: Option<String>,
    pub rotated_from_access_key_id: Option<String>,
    pub legacy_gateway_api_key_id: Option<String>,
    pub legacy_user_credential_id: Option<String>,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub metadata: Option<Value>,
    pub revoked_at: Option<String>,
    pub revoke_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessKeyBalanceView {
    pub access_key_id: String,
    pub balance_mode: String,
    pub status: String,
    pub unlimited_until: Option<String>,
    pub period_starts_at: Option<String>,
    pub period_ends_at: Option<String>,
    pub total_tokens: Option<i64>,
    pub remaining_tokens: Option<i64>,
    pub total_messages: Option<i64>,
    pub remaining_messages: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessKeyAggregateMembershipView {
    pub aggregate_access_key_id: String,
    pub member_access_key_id: String,
    pub priority: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessCatalogView {
    pub provider_capabilities: Vec<GatewayProviderCapabilityView>,
    pub platform_access_rows: Vec<GatewayPlatformAccessView>,
    pub bundles: Vec<GatewayAccessBundleView>,
    pub bundle_items: Vec<GatewayAccessBundleItemView>,
    pub access_keys: Vec<GatewayAccessKeyView>,
    pub key_bundle_bindings: Vec<GatewayAccessKeyBundleBindingView>,
    pub balances: Vec<GatewayAccessKeyBalanceView>,
    pub aggregate_memberships: Vec<GatewayAccessKeyAggregateMembershipView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessStickyAffinityView {
    pub scope: String,
    pub model: String,
    pub requesting_access_key_id: String,
    pub source_access_key_id: String,
    pub platform_access_id: String,
    pub provider_account_id: String,
    pub real_credential_ref: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessCandidatePreviewView {
    pub requesting_access_key_id: String,
    pub source_access_key_id: String,
    pub platform_access_id: String,
    pub provider_account_id: String,
    pub provider_credential_id: Option<String>,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub platform_tier: String,
    pub operator_weight: i32,
    pub routing_priority: i32,
    pub sticky_matched: bool,
    pub available_by_balance: bool,
    pub balance_mode: Option<String>,
    pub remaining_tokens: Option<i64>,
    pub remaining_messages: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayAccessRouteDecisionPreviewView {
    pub requesting_access_key_id: String,
    pub requested_model: String,
    pub endpoint_kind: String,
    pub sticky_matched: bool,
    pub selected: Option<GatewayAccessCandidatePreviewView>,
    pub candidates: Vec<GatewayAccessCandidatePreviewView>,
}

#[derive(Debug, Clone)]
pub struct UpsertProviderCapabilityInput {
    pub provider_account_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UpsertPlatformAccessInput {
    pub provider_capability_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub platform_tier: String,
    pub status: String,
    pub operator_weight: i32,
    pub routing_priority: i32,
    pub enabled_for_sale: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertAccessBundleInput {
    pub project_id: Option<String>,
    pub slug: String,
    pub display_name: String,
    pub billing_mode: String,
    pub status: String,
    pub description: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct UpsertAccessKeyInput {
    pub owner_type: String,
    pub owner_id: String,
    pub resolved_project_id: String,
    pub resolved_tenant_id: String,
    pub key_kind: String,
    pub public_key_prefix: String,
    pub display_name: String,
    pub expires_at: Option<String>,
    pub metadata: Option<Value>,
    pub bundle_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AccessKeyBalanceAdjustInput {
    pub balance_mode: Option<String>,
    pub status: Option<String>,
    pub unlimited_until: Option<String>,
    pub period_starts_at: Option<String>,
    pub period_ends_at: Option<String>,
    pub token_delta: Option<i64>,
    pub message_delta: Option<i64>,
    pub total_tokens: Option<i64>,
    pub remaining_tokens: Option<i64>,
    pub total_messages: Option<i64>,
    pub remaining_messages: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AggregateMembershipInput {
    pub member_access_key_id: String,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct AccessKeyAuthRecord {
    pub id: String,
    pub owner_type: String,
    pub owner_id: String,
    pub resolved_project_id: String,
    pub resolved_tenant_id: String,
    pub key_kind: String,
    pub status: String,
    pub public_key_prefix: String,
    pub display_name: String,
    pub external_key: Option<String>,
    pub expires_at: Option<String>,
    pub metadata: Option<Value>,
    pub legacy_gateway_api_key_id: Option<String>,
    pub legacy_user_credential_id: Option<String>,
    /// Monotonic database version for projection-affecting access-key changes.
    /// `last_used_at` updates intentionally do not advance this value.
    pub projection_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedPlatformAccessRow {
    pub requesting_access_key_id: String,
    pub source_access_key_id: String,
    pub platform_access_id: String,
    pub provider_account_id: String,
    pub model_code: String,
    pub endpoint_kind: String,
    pub upstream_model: Option<String>,
    pub platform_tier: String,
    pub operator_weight: i32,
    pub routing_priority: i32,
}

#[derive(Debug, Clone)]
pub struct ResolvedAccessKeyRouteContext {
    pub requesting_access_key_id: String,
    pub key_kind: String,
    pub model: String,
    pub sticky: Option<GatewayAccessStickyAffinityView>,
    pub selected: Option<ProjectedPlatformAccessRow>,
    pub candidates: Vec<ProjectedPlatformAccessRow>,
    pub route_candidates: Vec<RouteCandidate>,
}

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
struct BundleItemExpansionSourceRow {
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
    capability_enabled: bool,
    protocol_family: String,
    adapter: String,
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

fn to_provider_capability_view(row: ProviderCapabilityRow) -> GatewayProviderCapabilityView {
    GatewayProviderCapabilityView {
        id: row.id,
        provider_account_id: row.provider_account_id,
        model_code: row.model_code,
        endpoint_kind: row.endpoint_kind,
        upstream_model: row.upstream_model,
        enabled: row.enabled,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn to_platform_access_view(row: PlatformAccessRow) -> GatewayPlatformAccessView {
    GatewayPlatformAccessView {
        id: row.id,
        provider_capability_id: row.provider_capability_id,
        provider_account_id: row.provider_account_id,
        model_code: row.model_code,
        endpoint_kind: row.endpoint_kind,
        upstream_model: row.upstream_model,
        platform_tier: row.platform_tier,
        status: row.status,
        operator_weight: row.operator_weight,
        routing_priority: row.routing_priority,
        enabled_for_sale: row.enabled_for_sale,
        notes: row.notes,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn to_access_bundle_view(row: AccessBundleRow) -> GatewayAccessBundleView {
    GatewayAccessBundleView {
        id: row.id,
        project_id: row.project_id,
        slug: row.slug,
        display_name: row.display_name,
        billing_mode: row.billing_mode,
        status: row.status,
        description: row.description,
        metadata: row.metadata.map(|v| v.0),
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    }
}

fn to_access_bundle_item_view(row: AccessBundleItemRow) -> GatewayAccessBundleItemView {
    GatewayAccessBundleItemView {
        bundle_id: row.bundle_id,
        platform_access_id: row.platform_access_id,
        created_at: format_timestamp(row.created_at),
    }
}

fn to_access_key_bundle_binding_view(
    row: AccessKeyBundleBindingRow,
) -> GatewayAccessKeyBundleBindingView {
    GatewayAccessKeyBundleBindingView {
        access_key_id: row.access_key_id,
        bundle_id: row.bundle_id,
        created_at: format_timestamp(row.created_at),
    }
}

fn build_access_key_token(
    row: &AccessKeyRow,
    api_key_secret: Option<&str>,
) -> Result<Option<String>, GatewayError> {
    match row.public_key_prefix.as_str() {
        "gw-user" => Ok(row.external_key.clone()),
        prefix
            if prefix == "neuro"
                || prefix == "new_api"
                || normalize_bundle_scoped_gateway_project_api_key_prefix(prefix).is_some() =>
        {
            let secret = api_key_secret
                .ok_or_else(|| GatewayError::conflict("当前环境尚未配置 GATEWAY_API_KEY_SECRET"))?;
            Ok(Some(build_gateway_project_api_key_with_prefix(
                &row.id,
                &row.resolved_project_id,
                &row.resolved_tenant_id,
                secret,
                prefix,
            )))
        }
        _ => Ok(row.external_key.clone()),
    }
}

fn normalize_public_key_prefix(owner_type: &str, raw_prefix: &str) -> String {
    let normalized = raw_prefix.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "gw-user" => "gw-user".to_string(),
        "new_api" | "neuro" => "neuro".to_string(),
        _ => normalize_bundle_scoped_gateway_project_api_key_prefix(raw_prefix).unwrap_or_else(
            || {
                if owner_type == "platform" || owner_type == "project" {
                    "neuro".to_string()
                } else {
                    raw_prefix.trim().to_string()
                }
            },
        ),
    }
}

fn to_access_key_view(
    row: AccessKeyRow,
    api_key_secret: Option<&str>,
) -> Result<GatewayAccessKeyView, GatewayError> {
    let token = build_access_key_token(&row, api_key_secret)?;
    Ok(GatewayAccessKeyView {
        id: row.id,
        owner_type: row.owner_type,
        owner_id: row.owner_id,
        resolved_project_id: row.resolved_project_id,
        resolved_tenant_id: row.resolved_tenant_id,
        key_kind: row.key_kind,
        status: row.status,
        public_key_prefix: row.public_key_prefix,
        display_name: row.display_name,
        token,
        external_key: row.external_key,
        rotated_from_access_key_id: row.rotated_from_access_key_id,
        legacy_gateway_api_key_id: row.legacy_gateway_api_key_id,
        legacy_user_credential_id: row.legacy_user_credential_id,
        expires_at: row.expires_at.map(format_timestamp),
        last_used_at: row.last_used_at.map(format_timestamp),
        metadata: row.metadata.map(|value| value.0),
        revoked_at: row.revoked_at.map(format_timestamp),
        revoke_reason: row.revoke_reason,
        created_at: format_timestamp(row.created_at),
        updated_at: format_timestamp(row.updated_at),
    })
}

fn to_access_key_balance_view(row: AccessKeyBalanceRow) -> GatewayAccessKeyBalanceView {
    GatewayAccessKeyBalanceView {
        access_key_id: row.access_key_id,
        balance_mode: row.balance_mode,
        status: row.status,
        unlimited_until: row.unlimited_until.map(format_timestamp),
        period_starts_at: row.period_starts_at.map(format_timestamp),
        period_ends_at: row.period_ends_at.map(format_timestamp),
        total_tokens: row.total_tokens,
        remaining_tokens: row.remaining_tokens,
        total_messages: row.total_messages,
        remaining_messages: row.remaining_messages,
        updated_at: format_timestamp(row.updated_at),
    }
}

fn to_aggregate_membership_view(
    row: AggregateMembershipRow,
) -> GatewayAccessKeyAggregateMembershipView {
    GatewayAccessKeyAggregateMembershipView {
        aggregate_access_key_id: row.aggregate_access_key_id,
        member_access_key_id: row.member_access_key_id,
        priority: row.priority,
        created_at: format_timestamp(row.created_at),
    }
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

pub async fn list_access_catalog(
    pool: &PgPool,
    api_key_secret: Option<&str>,
) -> Result<GatewayAccessCatalogView, GatewayError> {
    let provider_capabilities = sqlx::query_as::<_, ProviderCapabilityRow>(
        r#"
        select id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
        from gateway_provider_capability_catalog
        order by provider_account_id asc, model_code asc, endpoint_kind asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_provider_capability_view)
    .collect();

    let platform_access_rows = sqlx::query_as::<_, PlatformAccessRow>(
        r#"
        select
          pac.id,
          pac.provider_capability_id,
          pc.provider_account_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.status,
          pac.operator_weight,
          pac.routing_priority,
          pac.enabled_for_sale,
          pac.notes,
          pac.created_at,
          pac.updated_at
        from gateway_platform_access_catalog pac
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        order by pac.model_code asc, pac.endpoint_kind asc, pac.routing_priority desc, pac.id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_platform_access_view)
    .collect();

    let bundles = sqlx::query_as::<_, AccessBundleRow>(
        r#"
        select id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
        from gateway_access_bundles
        order by updated_at desc, id desc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_bundle_view)
    .collect();

    let bundle_items = sqlx::query_as::<_, AccessBundleItemRow>(
        r#"
        select bundle_id, platform_access_id, created_at
        from gateway_access_bundle_items
        order by bundle_id asc, platform_access_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_bundle_item_view)
    .collect();

    let access_keys = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        order by updated_at desc, id desc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(|row| to_access_key_view(row, api_key_secret))
    .collect::<Result<Vec<_>, _>>()?;

    let key_bundle_bindings = sqlx::query_as::<_, AccessKeyBundleBindingRow>(
        r#"
        select access_key_id, bundle_id, created_at
        from gateway_access_key_bundle_bindings
        order by access_key_id asc, bundle_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_key_bundle_binding_view)
    .collect::<Vec<_>>();

    let balances = sqlx::query_as::<_, AccessKeyBalanceRow>(
        r#"
        select
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        from gateway_access_key_balances
        order by updated_at desc, access_key_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_access_key_balance_view)
    .collect();

    let aggregate_memberships = sqlx::query_as::<_, AggregateMembershipRow>(
        r#"
        select aggregate_access_key_id, member_access_key_id, priority, created_at
        from gateway_access_key_aggregate_memberships
        order by aggregate_access_key_id asc, priority asc, member_access_key_id asc
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?
    .into_iter()
    .map(to_aggregate_membership_view)
    .collect();

    Ok(GatewayAccessCatalogView {
        provider_capabilities,
        platform_access_rows,
        bundles,
        bundle_items,
        access_keys,
        key_bundle_bindings,
        balances,
        aggregate_memberships,
    })
}

const ACCESS_PROJECTION_CACHE_SCHEMA_VERSION: u16 = 2;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedAccessKeyBalance {
    access_key_id: String,
    balance_mode: String,
    status: String,
    unlimited_until: Option<String>,
    period_starts_at: Option<String>,
    period_ends_at: Option<String>,
    total_tokens: Option<i64>,
    remaining_tokens: Option<i64>,
    total_messages: Option<i64>,
    remaining_messages: Option<i64>,
    updated_at: String,
}

#[derive(Debug, Clone)]
pub struct AccessBalanceDecision {
    pub allowed: bool,
    pub balance_mode: Option<String>,
    pub pre_deduct_amount: u64,
    pub remaining_tokens: Option<i64>,
    pub remaining_messages: Option<i64>,
    pub reason: Option<String>,
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

fn normalize_owner_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "platform" => "platform".to_string(),
        "project" => "project".to_string(),
        _ => "user".to_string(),
    }
}

fn access_projection_cache_key(access_key_id: &str) -> String {
    keys::access_projection_key(access_key_id)
}

fn sticky_affinity_scope(explicit_session_key: Option<&str>) -> String {
    explicit_session_key
        .map(|value| format!("session:{value}"))
        .unwrap_or_else(|| "key".to_string())
}

fn sticky_affinity_key(
    requesting_access_key_id: &str,
    model: &str,
    explicit_session_key: Option<&str>,
) -> String {
    keys::access_affinity_key(
        &sticky_affinity_scope(explicit_session_key),
        requesting_access_key_id,
        model,
    )
}

fn balance_runtime_key(access_key_id: &str) -> String {
    keys::access_balance_key(access_key_id)
}

fn map_balance_row_to_cache(row: &AccessKeyBalanceRow) -> CachedAccessKeyBalance {
    CachedAccessKeyBalance {
        access_key_id: row.access_key_id.clone(),
        balance_mode: row.balance_mode.clone(),
        status: row.status.clone(),
        unlimited_until: row.unlimited_until.map(format_timestamp),
        period_starts_at: row.period_starts_at.map(format_timestamp),
        period_ends_at: row.period_ends_at.map(format_timestamp),
        total_tokens: row.total_tokens,
        remaining_tokens: row.remaining_tokens,
        total_messages: row.total_messages,
        remaining_messages: row.remaining_messages,
        updated_at: format_timestamp(row.updated_at),
    }
}

fn project_balance_decision(
    balance: &CachedAccessKeyBalance,
    estimated_tokens: u64,
) -> AccessBalanceDecision {
    let now = now_utc();
    if balance.status != "active" {
        return AccessBalanceDecision {
            allowed: false,
            balance_mode: Some(balance.balance_mode.clone()),
            pre_deduct_amount: 0,
            remaining_tokens: balance.remaining_tokens,
            remaining_messages: balance.remaining_messages,
            reason: Some("balance_inactive".to_string()),
        };
    }

    let within_period = |starts_at: Option<&str>, ends_at: Option<&str>| -> bool {
        let starts_at = parse_optional_timestamp(starts_at);
        let ends_at = parse_optional_timestamp(ends_at);
        if starts_at.is_some_and(|value| now < value) {
            return false;
        }
        if ends_at.is_some_and(|value| now > value) {
            return false;
        }
        true
    };

    let normalized_mode = normalize_balance_mode(&balance.balance_mode);
    match normalized_mode.as_str() {
        "time_pass" => {
            let unlimited_until = parse_optional_timestamp(balance.unlimited_until.as_deref());
            let allowed = unlimited_until.is_some_and(|value| value > now)
                && within_period(
                    balance.period_starts_at.as_deref(),
                    balance.period_ends_at.as_deref(),
                );
            AccessBalanceDecision {
                allowed,
                balance_mode: Some(normalized_mode.clone()),
                pre_deduct_amount: 0,
                remaining_tokens: balance.remaining_tokens,
                remaining_messages: balance.remaining_messages,
                reason: if allowed {
                    None
                } else {
                    Some("time_pass_inactive".to_string())
                },
            }
        }
        "message_prepaid" => {
            let remaining = balance.remaining_messages.unwrap_or(0);
            let allowed = remaining > 0
                && within_period(
                    balance.period_starts_at.as_deref(),
                    balance.period_ends_at.as_deref(),
                );
            AccessBalanceDecision {
                allowed,
                balance_mode: Some(normalized_mode.clone()),
                pre_deduct_amount: if allowed { 1 } else { 0 },
                remaining_tokens: balance.remaining_tokens,
                remaining_messages: Some(remaining),
                reason: if allowed {
                    None
                } else {
                    Some("message_balance_exhausted".to_string())
                },
            }
        }
        _ => {
            let remaining = balance.remaining_tokens.unwrap_or(0);
            let estimate = estimated_tokens.max(1) as i64;
            let allowed = remaining >= estimate
                && within_period(
                    balance.period_starts_at.as_deref(),
                    balance.period_ends_at.as_deref(),
                );
            AccessBalanceDecision {
                allowed,
                balance_mode: Some("token_prepaid".to_string()),
                pre_deduct_amount: if allowed { estimated_tokens.max(1) } else { 0 },
                remaining_tokens: Some(remaining),
                remaining_messages: balance.remaining_messages,
                reason: if allowed {
                    None
                } else {
                    Some("token_balance_exhausted".to_string())
                },
            }
        }
    }
}

async fn cache_balance(
    redis_pool: &RedisPool,
    balance: &CachedAccessKeyBalance,
) -> Result<(), GatewayError> {
    let payload = serde_json::to_string(balance)
        .map_err(|error| GatewayError::server_error(format!("serialize balance cache: {error}")))?;
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = conn
        .set(balance_runtime_key(&balance.access_key_id), payload)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write access balance cache: {error}"))
        })?;
    Ok(())
}

async fn load_cached_balance(
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Option<CachedAccessKeyBalance>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> =
        conn.get(balance_runtime_key(access_key_id))
            .await
            .map_err(|error| {
                GatewayError::server_error(format!("read access balance cache: {error}"))
            })?;
    raw.map(|value| {
        serde_json::from_str::<CachedAccessKeyBalance>(&value).map_err(|error| {
            GatewayError::server_error(format!("deserialize access balance cache: {error}"))
        })
    })
    .transpose()
}

async fn load_balance_from_db(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Option<CachedAccessKeyBalance>, GatewayError> {
    let row = sqlx::query_as::<_, AccessKeyBalanceRow>(
        r#"
        select
          access_key_id,
          balance_mode,
          status,
          unlimited_until,
          period_starts_at,
          period_ends_at,
          total_tokens,
          remaining_tokens,
          total_messages,
          remaining_messages,
          updated_at
        from gateway_access_key_balances
        where access_key_id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let cached = map_balance_row_to_cache(&row);
    cache_balance(redis_pool, &cached).await?;
    Ok(Some(cached))
}

pub async fn get_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Option<GatewayAccessKeyBalanceView>, GatewayError> {
    let cached = match load_cached_balance(redis_pool, access_key_id).await? {
        Some(value) => value,
        None => match load_balance_from_db(pool, redis_pool, access_key_id).await? {
            Some(value) => value,
            None => return Ok(None),
        },
    };
    Ok(Some(GatewayAccessKeyBalanceView {
        access_key_id: cached.access_key_id,
        balance_mode: cached.balance_mode,
        status: cached.status,
        unlimited_until: cached.unlimited_until,
        period_starts_at: cached.period_starts_at,
        period_ends_at: cached.period_ends_at,
        total_tokens: cached.total_tokens,
        remaining_tokens: cached.remaining_tokens,
        total_messages: cached.total_messages,
        remaining_messages: cached.remaining_messages,
        updated_at: cached.updated_at,
    }))
}

pub async fn evaluate_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    estimated_tokens: u64,
) -> Result<AccessBalanceDecision, GatewayError> {
    let balance = match load_cached_balance(redis_pool, access_key_id).await? {
        Some(value) => value,
        None => load_balance_from_db(pool, redis_pool, access_key_id)
            .await?
            .ok_or_else(|| {
                GatewayError::quota_exceeded("当前 key 尚未初始化额度")
                    .with_code("balance_not_initialized")
            })?,
    };
    Ok(project_balance_decision(&balance, estimated_tokens))
}

fn apply_balance_delta(balance: &mut CachedAccessKeyBalance, token_delta: i64, message_delta: i64) {
    if let Some(value) = balance.remaining_tokens.as_mut() {
        *value += token_delta;
    }
    if let Some(value) = balance.total_tokens.as_mut() {
        if token_delta > 0 {
            *value += token_delta;
        }
    }
    if let Some(value) = balance.remaining_messages.as_mut() {
        *value += message_delta;
    }
    if let Some(value) = balance.total_messages.as_mut() {
        if message_delta > 0 {
            *value += message_delta;
        }
    }
    balance.updated_at = format_timestamp(now_utc());
}

async fn persist_cached_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    balance: &CachedAccessKeyBalance,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_access_key_balances
        set
          balance_mode = $2,
          status = $3,
          unlimited_until = $4,
          period_starts_at = $5,
          period_ends_at = $6,
          total_tokens = $7,
          remaining_tokens = $8,
          total_messages = $9,
          remaining_messages = $10,
          updated_at = $11
        where access_key_id = $1
        "#,
    )
    .bind(&balance.access_key_id)
    .bind(&balance.balance_mode)
    .bind(&balance.status)
    .bind(parse_optional_timestamp(balance.unlimited_until.as_deref()))
    .bind(parse_optional_timestamp(
        balance.period_starts_at.as_deref(),
    ))
    .bind(parse_optional_timestamp(balance.period_ends_at.as_deref()))
    .bind(balance.total_tokens)
    .bind(balance.remaining_tokens)
    .bind(balance.total_messages)
    .bind(balance.remaining_messages)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    cache_balance(redis_pool, balance).await
}

pub async fn pre_deduct_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    estimated_tokens: u64,
) -> Result<AccessBalanceDecision, GatewayError> {
    let mut balance = match load_cached_balance(redis_pool, access_key_id).await? {
        Some(value) => value,
        None => load_balance_from_db(pool, redis_pool, access_key_id)
            .await?
            .ok_or_else(|| {
                GatewayError::quota_exceeded("当前 key 尚未初始化额度")
                    .with_code("balance_not_initialized")
            })?,
    };
    let decision = project_balance_decision(&balance, estimated_tokens);
    if !decision.allowed {
        return Ok(decision);
    }
    match decision.balance_mode.as_deref() {
        Some("message_prepaid") => {
            apply_balance_delta(&mut balance, 0, -(decision.pre_deduct_amount as i64));
            persist_cached_balance(pool, redis_pool, &balance).await?;
        }
        Some("token_prepaid") => {
            apply_balance_delta(&mut balance, -(decision.pre_deduct_amount as i64), 0);
            persist_cached_balance(pool, redis_pool, &balance).await?;
        }
        _ => {}
    }
    Ok(decision)
}

pub async fn refund_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    pre_deduct_amount: u64,
) -> Result<(), GatewayError> {
    if pre_deduct_amount == 0 {
        return Ok(());
    }
    let Some(mut balance) = load_balance_from_db(pool, redis_pool, access_key_id).await? else {
        return Ok(());
    };
    match normalize_balance_mode(&balance.balance_mode).as_str() {
        "message_prepaid" => apply_balance_delta(&mut balance, 0, pre_deduct_amount as i64),
        "token_prepaid" => apply_balance_delta(&mut balance, pre_deduct_amount as i64, 0),
        _ => {}
    }
    persist_cached_balance(pool, redis_pool, &balance).await
}

pub async fn settle_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    pre_deduct_amount: u64,
    actual_total_tokens: u64,
) -> Result<(), GatewayError> {
    let Some(mut balance) = load_balance_from_db(pool, redis_pool, access_key_id).await? else {
        return Ok(());
    };
    if normalize_balance_mode(&balance.balance_mode) != "token_prepaid" {
        return Ok(());
    }
    if actual_total_tokens > pre_deduct_amount {
        apply_balance_delta(
            &mut balance,
            -((actual_total_tokens - pre_deduct_amount) as i64),
            0,
        );
    } else if pre_deduct_amount > actual_total_tokens {
        apply_balance_delta(
            &mut balance,
            (pre_deduct_amount - actual_total_tokens) as i64,
            0,
        );
    }
    persist_cached_balance(pool, redis_pool, &balance).await
}

async fn clear_access_projection_cache(redis_pool: &RedisPool) -> Result<(), GatewayError> {
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(error) => {
            tracing::warn!(error = %error, "access projection cache cleanup skipped");
            return Ok(());
        }
    };
    let mut cursor: u64 = 0;
    loop {
        let scan_result: Result<(u64, Vec<String>), _> = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("gw:access:projection:*")
            .arg("COUNT")
            .arg(200)
            .query_async(&mut conn)
            .await;
        let (next_cursor, keys) = match scan_result {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(error = %error, "access projection cache scan failed");
                return Ok(());
            }
        };
        if !keys.is_empty() {
            let delete_result: Result<(), _> =
                redis::cmd("DEL").arg(keys).query_async(&mut conn).await;
            if let Err(error) = delete_result {
                tracing::warn!(error = %error, "access projection cache delete failed");
                return Ok(());
            }
        }
        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }
    Ok(())
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

async fn load_provider_candidate_meta_map(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, ProviderCandidateMetaRow>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, ProviderCandidateMetaRow>(
        r#"
        select
          id,
          label,
          adapter,
          protocol_family,
          protocol_profile,
          cooldown_until,
          failure_count,
          execution_mode
        from gateway_provider_accounts
        where id = any($1)
          and status = 'active'
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect::<HashMap<_, _>>())
}

async fn build_projection_rows_for_normal_key(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<Vec<ProjectedPlatformAccessRow>, GatewayError> {
    sqlx::query_as::<_, PlatformAccessRow>(
        r#"
        select
          pac.id,
          pac.provider_capability_id,
          pc.provider_account_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.status,
          pac.operator_weight,
          pac.routing_priority,
          pac.enabled_for_sale,
          pac.notes,
          pac.created_at,
          pac.updated_at
        from gateway_access_keys access_key
        join gateway_access_key_bundle_bindings kb on kb.access_key_id = access_key.id
        join gateway_access_bundles b on b.id = kb.bundle_id
          and b.status = 'active'
          and (b.project_id = access_key.resolved_project_id or b.project_id is null)
        join gateway_access_bundle_items bi on bi.bundle_id = b.id
        join gateway_platform_access_catalog pac on pac.id = bi.platform_access_id
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        where access_key.id = $1
          and pac.status = 'active'
          and pac.enabled_for_sale = true
          and pc.enabled = true
        order by pac.model_code asc, pac.endpoint_kind asc, pac.routing_priority desc, pac.operator_weight desc, pac.id asc
        "#,
    )
    .bind(access_key_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
    .map(|rows| {
        rows.into_iter()
            .map(|row| ProjectedPlatformAccessRow {
                requesting_access_key_id: access_key_id.to_string(),
                source_access_key_id: access_key_id.to_string(),
                platform_access_id: row.id,
                provider_account_id: row.provider_account_id,
                model_code: row.model_code,
                endpoint_kind: row.endpoint_kind,
                upstream_model: row.upstream_model,
                platform_tier: row.platform_tier,
                operator_weight: row.operator_weight,
                routing_priority: row.routing_priority,
            })
            .collect()
    })
}

async fn build_projection_rows_for_auto_route_key(
    pool: &PgPool,
    aggregate_access_key_id: &str,
) -> Result<Vec<ProjectedPlatformAccessRow>, GatewayError> {
    let rows = sqlx::query(
        r#"
        select
          member.id as source_access_key_id,
          pac.id as platform_access_id,
          pc.provider_account_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.operator_weight,
          pac.routing_priority
        from gateway_access_key_aggregate_memberships m
        join gateway_access_keys aggregate on aggregate.id = m.aggregate_access_key_id
        join gateway_access_keys member on member.id = m.member_access_key_id
          and member.resolved_project_id = aggregate.resolved_project_id
          and member.resolved_tenant_id = aggregate.resolved_tenant_id
        join gateway_access_key_bundle_bindings kb on kb.access_key_id = member.id
        join gateway_access_bundles b on b.id = kb.bundle_id
          and b.status = 'active'
          and (b.project_id = aggregate.resolved_project_id or b.project_id is null)
        join gateway_access_bundle_items bi on bi.bundle_id = b.id
        join gateway_platform_access_catalog pac on pac.id = bi.platform_access_id
        join gateway_provider_capability_catalog pc on pc.id = pac.provider_capability_id
        where m.aggregate_access_key_id = $1
          and aggregate.status = 'active'
          and member.status = 'active'
          and (member.expires_at is null or member.expires_at > now())
          and pac.status = 'active'
          and pac.enabled_for_sale = true
          and pc.enabled = true
        order by m.priority asc, pac.model_code asc, pac.endpoint_kind asc, pac.routing_priority desc, pac.operator_weight desc, pac.id asc
        "#,
    )
    .bind(aggregate_access_key_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| ProjectedPlatformAccessRow {
            requesting_access_key_id: aggregate_access_key_id.to_string(),
            source_access_key_id: row.get::<String, _>("source_access_key_id"),
            platform_access_id: row.get::<String, _>("platform_access_id"),
            provider_account_id: row.get::<String, _>("provider_account_id"),
            model_code: row.get::<String, _>("model_code"),
            endpoint_kind: row.get::<String, _>("endpoint_kind"),
            upstream_model: row.get::<Option<String>, _>("upstream_model"),
            platform_tier: row.get::<String, _>("platform_tier"),
            operator_weight: row.get::<i32, _>("operator_weight"),
            routing_priority: row.get::<i32, _>("routing_priority"),
        })
        .collect())
}

async fn load_provider_credential_map_for_rows(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<HashMap<String, Vec<super::GatewayProviderCredentialView>>, GatewayError> {
    let credentials =
        list_active_provider_credentials_for_accounts(pool, provider_account_ids).await?;
    let mut result = HashMap::<String, Vec<super::GatewayProviderCredentialView>>::new();
    for credential in credentials {
        result
            .entry(credential.provider_account_id.clone())
            .or_default()
            .push(credential);
    }
    Ok(result)
}

async fn load_provider_accounts_with_any_credentials(
    pool: &PgPool,
    provider_account_ids: &[String],
) -> Result<BTreeSet<String>, GatewayError> {
    if provider_account_ids.is_empty() {
        return Ok(BTreeSet::new());
    }

    let rows = sqlx::query_scalar::<_, String>(
        r#"
        select distinct provider_account_id
        from gateway_provider_credentials
        where provider_account_id = any($1)
          and archived_at is null
        "#,
    )
    .bind(provider_account_ids)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;

    Ok(rows.into_iter().collect())
}

async fn load_model_alias_projection_rows(
    pool: &PgPool,
    resolved_project_id: &str,
) -> Result<Vec<ModelAliasProjectionRow>, GatewayError> {
    sqlx::query_as::<_, ModelAliasProjectionRow>(
        r#"
        select alias, provider_account_id, upstream_model
        from gateway_model_aliases
        where enabled = true
          and (project_id = $1 or project_id is null)
        order by alias asc, priority asc, weight desc, created_at asc
        "#,
    )
    .bind(resolved_project_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)
}

fn expand_projection_rows_with_aliases(
    rows: Vec<ProjectedPlatformAccessRow>,
    alias_rows: &[ModelAliasProjectionRow],
) -> Vec<ProjectedPlatformAccessRow> {
    let mut expanded = rows.clone();
    let mut seen = rows
        .iter()
        .map(|row| {
            (
                row.requesting_access_key_id.clone(),
                row.source_access_key_id.clone(),
                row.platform_access_id.clone(),
                row.model_code.clone(),
                row.endpoint_kind.clone(),
            )
        })
        .collect::<BTreeSet<_>>();

    for row in rows {
        let row_upstream = row
            .upstream_model
            .as_deref()
            .unwrap_or(row.model_code.as_str());
        for alias_row in alias_rows {
            if alias_row.provider_account_id != row.provider_account_id {
                continue;
            }
            let Some(alias_upstream) = alias_row.upstream_model.as_deref() else {
                continue;
            };
            if alias_upstream != row_upstream {
                continue;
            }
            let key = (
                row.requesting_access_key_id.clone(),
                row.source_access_key_id.clone(),
                row.platform_access_id.clone(),
                alias_row.alias.clone(),
                row.endpoint_kind.clone(),
            );
            if !seen.insert(key) {
                continue;
            }
            let mut alias_projection = row.clone();
            alias_projection.model_code = alias_row.alias.clone();
            expanded.push(alias_projection);
        }
    }

    expanded
}

fn group_projection_rows_by_model(
    access_key: &AccessKeyAuthRecord,
    rows: Vec<ProjectedPlatformAccessRow>,
) -> CachedAccessProjection {
    let mut rows_by_model = BTreeMap::<String, Vec<ProjectedPlatformAccessRow>>::new();
    for row in rows {
        rows_by_model
            .entry(row.model_code.clone())
            .or_default()
            .push(row);
    }
    for rows in rows_by_model.values_mut() {
        rows.sort_by(|left, right| {
            right
                .routing_priority
                .cmp(&left.routing_priority)
                .then(right.operator_weight.cmp(&left.operator_weight))
                .then(
                    platform_tier_rank(&right.platform_tier)
                        .cmp(&platform_tier_rank(&left.platform_tier)),
                )
                .then(left.provider_account_id.cmp(&right.provider_account_id))
                .then(left.source_access_key_id.cmp(&right.source_access_key_id))
        });
    }
    CachedAccessProjection {
        schema_version: ACCESS_PROJECTION_CACHE_SCHEMA_VERSION,
        access_key_id: access_key.id.clone(),
        key_kind: access_key.key_kind.clone(),
        resolved_tenant_id: access_key.resolved_tenant_id.clone(),
        resolved_project_id: access_key.resolved_project_id.clone(),
        projection_version: access_key.projection_version.clone(),
        rows_by_model,
    }
}

async fn rebuild_access_projection(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key: &AccessKeyAuthRecord,
) -> Result<CachedAccessProjection, GatewayError> {
    let rows = if access_key.key_kind == "auto_route" {
        build_projection_rows_for_auto_route_key(pool, &access_key.id).await?
    } else {
        build_projection_rows_for_normal_key(pool, &access_key.id).await?
    };
    let alias_rows =
        load_model_alias_projection_rows(pool, &access_key.resolved_project_id).await?;
    let rows = expand_projection_rows_with_aliases(rows, &alias_rows);
    let projection = group_projection_rows_by_model(access_key, rows);
    let payload = serde_json::to_string(&projection).map_err(|error| {
        GatewayError::server_error(format!("serialize access projection: {error}"))
    })?;
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = conn
        .set(access_projection_cache_key(&access_key.id), payload)
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("write access projection cache: {error}"))
        })?;
    Ok(projection)
}

async fn get_cached_access_projection(
    redis_pool: &RedisPool,
    access_key: &AccessKeyAuthRecord,
) -> Result<Option<CachedAccessProjection>, GatewayError> {
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(access_projection_cache_key(&access_key.id))
        .await
        .map_err(|error| {
            GatewayError::server_error(format!("read access projection cache: {error}"))
        })?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let cached = match serde_json::from_str::<CachedAccessProjection>(&raw) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                access_key_id = %access_key.id,
                error = %error,
                "discarding malformed access projection cache"
            );
            return Ok(None);
        }
    };
    Ok(cache_projection_is_compatible(&cached, access_key).then_some(cached))
}

fn cache_projection_is_compatible(
    cached: &CachedAccessProjection,
    access_key: &AccessKeyAuthRecord,
) -> bool {
    cached.schema_version == ACCESS_PROJECTION_CACHE_SCHEMA_VERSION
        && cached.access_key_id == access_key.id
        && cached.key_kind == access_key.key_kind
        && cached.resolved_tenant_id == access_key.resolved_tenant_id
        && cached.resolved_project_id == access_key.resolved_project_id
        && cached.projection_version == access_key.projection_version
}

async fn get_or_rebuild_access_projection(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key: &AccessKeyAuthRecord,
) -> Result<CachedAccessProjection, GatewayError> {
    if let Some(projection) = get_cached_access_projection(redis_pool, access_key).await? {
        return Ok(projection);
    }
    rebuild_access_projection(pool, redis_pool, access_key).await
}

fn collect_model_catalog_ids_from_access_projection(
    projection: &CachedAccessProjection,
) -> Vec<String> {
    let shadowed_upstream_models = projection
        .rows_by_model
        .values()
        .flat_map(|rows| rows.iter())
        .filter_map(|row| {
            let upstream_model = row
                .upstream_model
                .as_deref()
                .unwrap_or(row.model_code.as_str());
            (row.model_code != upstream_model)
                .then_some((row.provider_account_id.clone(), upstream_model.to_string()))
        })
        .collect::<BTreeSet<_>>();

    projection
        .rows_by_model
        .iter()
        .filter_map(|(model_id, rows)| {
            rows.iter()
                .any(|row| {
                    let upstream_model = row
                        .upstream_model
                        .as_deref()
                        .unwrap_or(row.model_code.as_str());
                    row.model_code != upstream_model
                        || !shadowed_upstream_models.contains(&(
                            row.provider_account_id.clone(),
                            upstream_model.to_string(),
                        ))
                })
                .then_some(model_id.clone())
        })
        .collect()
}

pub async fn list_models_for_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<Vec<ModelInfo>, GatewayError> {
    let access_key = find_access_key_auth_by_id(pool, access_key_id)
        .await?
        .ok_or_else(|| GatewayError::unauthorized("Access key not found"))?;
    validate_access_key_auth(&access_key)?;
    let projection = get_or_rebuild_access_projection(pool, redis_pool, &access_key).await?;
    let created = OffsetDateTime::now_utc().unix_timestamp();
    Ok(
        collect_model_catalog_ids_from_access_projection(&projection)
            .into_iter()
            .map(|id| ModelInfo {
                id,
                object: "model".to_string(),
                created,
                owned_by: "neuro-gateway".to_string(),
            })
            .collect(),
    )
}

pub async fn find_access_key_auth_by_id(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<Option<AccessKeyAuthRecord>, GatewayError> {
    let row = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    Ok(row.map(|row| AccessKeyAuthRecord {
        id: row.id,
        owner_type: row.owner_type,
        owner_id: row.owner_id,
        resolved_project_id: row.resolved_project_id,
        resolved_tenant_id: row.resolved_tenant_id,
        key_kind: row.key_kind,
        status: row.status,
        public_key_prefix: row.public_key_prefix,
        display_name: row.display_name,
        external_key: row.external_key,
        expires_at: row.expires_at.map(format_timestamp),
        metadata: row.metadata.map(|value| value.0),
        legacy_gateway_api_key_id: row.legacy_gateway_api_key_id,
        legacy_user_credential_id: row.legacy_user_credential_id,
        projection_version: format_timestamp(row.updated_at),
    }))
}

pub async fn find_access_key_auth_by_external_key(
    pool: &PgPool,
    external_key: &str,
) -> Result<Option<AccessKeyAuthRecord>, GatewayError> {
    let row = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where external_key = $1
        limit 1
        "#,
    )
    .bind(external_key)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    Ok(row.map(|row| AccessKeyAuthRecord {
        id: row.id,
        owner_type: row.owner_type,
        owner_id: row.owner_id,
        resolved_project_id: row.resolved_project_id,
        resolved_tenant_id: row.resolved_tenant_id,
        key_kind: row.key_kind,
        status: row.status,
        public_key_prefix: row.public_key_prefix,
        display_name: row.display_name,
        external_key: row.external_key,
        expires_at: row.expires_at.map(format_timestamp),
        metadata: row.metadata.map(|value| value.0),
        legacy_gateway_api_key_id: row.legacy_gateway_api_key_id,
        legacy_user_credential_id: row.legacy_user_credential_id,
        projection_version: format_timestamp(row.updated_at),
    }))
}

pub fn validate_access_key_auth(record: &AccessKeyAuthRecord) -> Result<(), GatewayError> {
    if normalize_status(&record.status) != "active" {
        return Err(GatewayError::unauthorized("Access key is not active"));
    }
    if parse_optional_timestamp(record.expires_at.as_deref())
        .is_some_and(|value| value <= now_utc())
    {
        return Err(GatewayError::unauthorized("Access key expired"));
    }
    Ok(())
}

pub async fn touch_access_key_last_used(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<(), GatewayError> {
    sqlx::query(
        r#"
        update gateway_access_keys
        set last_used_at = $2
        where id = $1
        "#,
    )
    .bind(access_key_id)
    .bind(now_utc())
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    Ok(())
}

pub fn scope_list_from_metadata(metadata: Option<&Value>) -> Vec<String> {
    metadata
        .and_then(|value| value.get("scope").or_else(|| value.get("scopes")))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["relay".to_string()])
}

pub async fn save_provider_capability(
    pool: &PgPool,
    redis_pool: &RedisPool,
    provider_capability_id: Option<&str>,
    input: UpsertProviderCapabilityInput,
) -> Result<GatewayProviderCapabilityView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = now_utc();
    let row = if let Some(id) = provider_capability_id {
        sqlx::query_as::<_, ProviderCapabilityRow>(
            r#"
            update gateway_provider_capability_catalog
            set
              provider_account_id = $2,
              model_code = $3,
              endpoint_kind = $4,
              upstream_model = $5,
              enabled = $6,
              updated_at = $7
            where id = $1
            returning id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.provider_account_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(input.enabled)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("provider capability 不存在"))?
    } else {
        sqlx::query_as::<_, ProviderCapabilityRow>(
            r#"
            insert into gateway_provider_capability_catalog (
              id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $7)
            returning id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(input.provider_account_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(input.enabled)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    Ok(to_provider_capability_view(row))
}

pub async fn save_platform_access(
    pool: &PgPool,
    redis_pool: &RedisPool,
    platform_access_id: Option<&str>,
    input: UpsertPlatformAccessInput,
) -> Result<GatewayPlatformAccessView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = now_utc();
    let row = if let Some(id) = platform_access_id {
        sqlx::query_as::<_, PlatformAccessRow>(
            r#"
            update gateway_platform_access_catalog
            set
              provider_capability_id = $2,
              model_code = $3,
              endpoint_kind = $4,
              upstream_model = $5,
              platform_tier = $6,
              status = $7,
              operator_weight = $8,
              routing_priority = $9,
              enabled_for_sale = $10,
              notes = $11,
              updated_at = $12
            where id = $1
            returning
              id, provider_capability_id,
              (select provider_account_id from gateway_provider_capability_catalog where id = provider_capability_id) as provider_account_id,
              model_code, endpoint_kind, upstream_model, platform_tier, status, operator_weight, routing_priority, enabled_for_sale,
              notes, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.provider_capability_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(normalize_platform_tier(&input.platform_tier))
        .bind(normalize_status(&input.status))
        .bind(input.operator_weight)
        .bind(input.routing_priority)
        .bind(input.enabled_for_sale)
        .bind(input.notes.as_deref())
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("platform access 不存在"))?
    } else {
        sqlx::query_as::<_, PlatformAccessRow>(
            r#"
            insert into gateway_platform_access_catalog (
              id, provider_capability_id, model_code, endpoint_kind, upstream_model, platform_tier, status,
              operator_weight, routing_priority, enabled_for_sale, notes, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
            returning
              id, provider_capability_id,
              (select provider_account_id from gateway_provider_capability_catalog where id = provider_capability_id) as provider_account_id,
              model_code, endpoint_kind, upstream_model, platform_tier, status, operator_weight, routing_priority, enabled_for_sale,
              notes, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(input.provider_capability_id.trim())
        .bind(input.model_code.trim())
        .bind(input.endpoint_kind.trim())
        .bind(input.upstream_model.as_deref())
        .bind(normalize_platform_tier(&input.platform_tier))
        .bind(normalize_status(&input.status))
        .bind(input.operator_weight)
        .bind(input.routing_priority)
        .bind(input.enabled_for_sale)
        .bind(input.notes.as_deref())
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    Ok(to_platform_access_view(row))
}

pub async fn save_access_bundle(
    pool: &PgPool,
    redis_pool: &RedisPool,
    bundle_id: Option<&str>,
    input: UpsertAccessBundleInput,
) -> Result<GatewayAccessBundleView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let now = now_utc();
    let row = if let Some(id) = bundle_id {
        sqlx::query_as::<_, AccessBundleRow>(
            r#"
            update gateway_access_bundles
            set
              project_id = $2,
              slug = $3,
              display_name = $4,
              billing_mode = $5,
              status = $6,
              description = $7,
              metadata = $8,
              updated_at = $9
            where id = $1
            returning id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(input.project_id.as_deref())
        .bind(input.slug.trim())
        .bind(input.display_name.trim())
        .bind(normalize_balance_mode(&input.billing_mode))
        .bind(normalize_status(&input.status))
        .bind(input.description.as_deref())
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| GatewayError::not_found("access bundle 不存在"))?
    } else {
        sqlx::query_as::<_, AccessBundleRow>(
            r#"
            insert into gateway_access_bundles (
              id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
            returning id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(input.project_id.as_deref())
        .bind(input.slug.trim())
        .bind(input.display_name.trim())
        .bind(normalize_balance_mode(&input.billing_mode))
        .bind(normalize_status(&input.status))
        .bind(input.description.as_deref())
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?
    };
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    Ok(to_access_bundle_view(row))
}

fn openai_text_endpoint_family(endpoint_kind: &str) -> &'static [&'static str] {
    match endpoint_kind.trim().to_ascii_lowercase().as_str() {
        "responses" => &["responses", "chat_completions", "messages", "completions"],
        "messages" => &["messages", "responses", "chat_completions", "completions"],
        "completions" => &["completions", "chat_completions", "responses", "messages"],
        "chat_completions" => &["chat_completions", "responses", "messages", "completions"],
        _ => &[],
    }
}

fn supports_openai_bundle_endpoint_pair(protocol_family: &str, adapter: &str) -> bool {
    canonicalize_protocol_family_name(protocol_family) == "openai"
        || matches!(
            canonicalize_adapter_name(adapter).as_str(),
            "openai_compatible" | "freebuff_compatible"
        )
}

async fn ensure_companion_provider_capability_id(
    tx: &mut Transaction<'_, Postgres>,
    row: &BundleItemExpansionSourceRow,
    endpoint_kind: &str,
    now: OffsetDateTime,
) -> Result<String, GatewayError> {
    let existing = sqlx::query(
        r#"
        select id, upstream_model, enabled
        from gateway_provider_capability_catalog
        where provider_account_id = $1
          and model_code = $2
          and endpoint_kind = $3
        order by
          case
            when (
              (upstream_model is null and $4 is null)
              or upstream_model = $4
            ) then 0
            else 1
          end asc,
          created_at asc
        limit 1
        "#,
    )
    .bind(&row.provider_account_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?;

    if let Some(existing) = existing {
        let id = existing.get::<String, _>("id");
        let existing_upstream_model = existing.get::<Option<String>, _>("upstream_model");
        let existing_enabled = existing.get::<bool, _>("enabled");
        if existing_upstream_model.as_deref() != row.upstream_model.as_deref()
            || existing_enabled != row.capability_enabled
        {
            sqlx::query(
                r#"
                update gateway_provider_capability_catalog
                set upstream_model = $2,
                    enabled = $3,
                    updated_at = $4
                where id = $1
                "#,
            )
            .bind(&id)
            .bind(row.upstream_model.as_deref())
            .bind(row.capability_enabled)
            .bind(now)
            .execute(&mut **tx)
            .await
            .map_err(map_db_error)?;
        }
        return Ok(id);
    }

    sqlx::query_scalar::<_, String>(
        r#"
        insert into gateway_provider_capability_catalog (
          id, provider_account_id, model_code, endpoint_kind, upstream_model, enabled, created_at, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $7)
        returning id
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&row.provider_account_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .bind(row.capability_enabled)
    .bind(now)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_db_error)
}

async fn ensure_companion_platform_access_id(
    tx: &mut Transaction<'_, Postgres>,
    row: &BundleItemExpansionSourceRow,
    provider_capability_id: &str,
    endpoint_kind: &str,
    now: OffsetDateTime,
) -> Result<String, GatewayError> {
    let existing = sqlx::query(
        r#"
        select
          id,
          upstream_model,
          platform_tier,
          status,
          operator_weight,
          routing_priority,
          enabled_for_sale,
          notes
        from gateway_platform_access_catalog
        where provider_capability_id = $1
          and model_code = $2
          and endpoint_kind = $3
        order by
          case
            when (
              (upstream_model is null and $4 is null)
              or upstream_model = $4
            ) then 0
            else 1
          end asc,
          created_at asc
        limit 1
        "#,
    )
    .bind(provider_capability_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?;

    if let Some(existing) = existing {
        let id = existing.get::<String, _>("id");
        let existing_upstream_model = existing.get::<Option<String>, _>("upstream_model");
        let existing_platform_tier = existing.get::<String, _>("platform_tier");
        let existing_status = existing.get::<String, _>("status");
        let existing_operator_weight = existing.get::<i32, _>("operator_weight");
        let existing_routing_priority = existing.get::<i32, _>("routing_priority");
        let existing_enabled_for_sale = existing.get::<bool, _>("enabled_for_sale");
        let existing_notes = existing.get::<Option<String>, _>("notes");
        if existing_upstream_model.as_deref() != row.upstream_model.as_deref()
            || existing_platform_tier != row.platform_tier
            || existing_status != row.status
            || existing_operator_weight != row.operator_weight
            || existing_routing_priority != row.routing_priority
            || existing_enabled_for_sale != row.enabled_for_sale
            || existing_notes.as_deref() != row.notes.as_deref()
        {
            sqlx::query(
                r#"
                update gateway_platform_access_catalog
                set upstream_model = $2,
                    platform_tier = $3,
                    status = $4,
                    operator_weight = $5,
                    routing_priority = $6,
                    enabled_for_sale = $7,
                    notes = $8,
                    updated_at = $9
                where id = $1
                "#,
            )
            .bind(&id)
            .bind(row.upstream_model.as_deref())
            .bind(&row.platform_tier)
            .bind(&row.status)
            .bind(row.operator_weight)
            .bind(row.routing_priority)
            .bind(row.enabled_for_sale)
            .bind(row.notes.as_deref())
            .bind(now)
            .execute(&mut **tx)
            .await
            .map_err(map_db_error)?;
        }
        return Ok(id);
    }

    sqlx::query_scalar::<_, String>(
        r#"
        insert into gateway_platform_access_catalog (
          id, provider_capability_id, model_code, endpoint_kind, upstream_model, platform_tier, status,
          operator_weight, routing_priority, enabled_for_sale, notes, created_at, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
        returning id
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(provider_capability_id)
    .bind(&row.model_code)
    .bind(endpoint_kind)
    .bind(row.upstream_model.as_deref())
    .bind(&row.platform_tier)
    .bind(&row.status)
    .bind(row.operator_weight)
    .bind(row.routing_priority)
    .bind(row.enabled_for_sale)
    .bind(row.notes.as_deref())
    .bind(now)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_db_error)
}

async fn expand_bundle_platform_access_ids_for_openai_compatibility(
    tx: &mut Transaction<'_, Postgres>,
    platform_access_ids: &[String],
) -> Result<Vec<String>, GatewayError> {
    let mut expanded_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for platform_access_id in platform_access_ids {
        if seen.insert(platform_access_id.clone()) {
            expanded_ids.push(platform_access_id.clone());
        }
    }

    if expanded_ids.is_empty() {
        return Ok(expanded_ids);
    }

    let rows = sqlx::query_as::<_, BundleItemExpansionSourceRow>(
        r#"
        select
          pcc.provider_account_id,
          pac.model_code,
          pac.endpoint_kind,
          pac.upstream_model,
          pac.platform_tier,
          pac.status,
          pac.operator_weight,
          pac.routing_priority,
          pac.enabled_for_sale,
          pac.notes,
          pcc.enabled as capability_enabled,
          gpa.protocol_family,
          gpa.adapter
        from gateway_platform_access_catalog pac
        join gateway_provider_capability_catalog pcc
          on pcc.id = pac.provider_capability_id
        join gateway_provider_accounts gpa
          on gpa.id = pcc.provider_account_id
        where pac.id = any($1)
        "#,
    )
    .bind(&expanded_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(map_db_error)?;

    let now = now_utc();
    for row in rows {
        if !supports_openai_bundle_endpoint_pair(&row.protocol_family, &row.adapter) {
            continue;
        }
        for companion_endpoint_kind in openai_text_endpoint_family(&row.endpoint_kind) {
            if *companion_endpoint_kind == row.endpoint_kind {
                continue;
            }

            let companion_provider_capability_id =
                ensure_companion_provider_capability_id(tx, &row, companion_endpoint_kind, now)
                    .await?;
            let companion_platform_access_id = ensure_companion_platform_access_id(
                tx,
                &row,
                &companion_provider_capability_id,
                companion_endpoint_kind,
                now,
            )
            .await?;
            if seen.insert(companion_platform_access_id.clone()) {
                expanded_ids.push(companion_platform_access_id);
            }
        }
    }

    Ok(expanded_ids)
}

pub async fn delete_access_bundle(
    pool: &PgPool,
    redis_pool: &RedisPool,
    bundle_id: &str,
) -> Result<DeleteAccessBundleResult, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let bundle_row = sqlx::query(
        r#"
        select id, display_name
        from gateway_access_bundles
        where id = $1
        limit 1
        "#,
    )
    .bind(bundle_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access bundle 不存在"))?;

    let deleted_platform_key_count = sqlx::query(
        r#"
        with target_keys as (
          select distinct ak.id
          from gateway_access_key_bundle_bindings kb
          join gateway_access_keys ak on ak.id = kb.access_key_id
          where kb.bundle_id = $1
            and ak.owner_type = 'platform'
            and ak.owner_id = 'bundle-platform-key'
            and ak.key_kind = 'normal'
        )
        delete from gateway_access_keys ak
        using target_keys tk
        where ak.id = tk.id
        "#,
    )
    .bind(bundle_id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?
    .rows_affected() as i64;

    sqlx::query("delete from gateway_access_bundles where id = $1")
        .bind(bundle_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    bump_all_access_projection_versions(&mut tx, now_utc()).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;

    Ok(DeleteAccessBundleResult {
        bundle_id: bundle_row.get::<String, _>("id"),
        display_name: bundle_row.get::<String, _>("display_name"),
        deleted_platform_key_count,
    })
}

pub async fn delete_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
) -> Result<DeleteAccessKeyResult, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let key_row = sqlx::query(
        r#"
        select id, display_name
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access key 不存在"))?;

    sqlx::query("delete from gateway_access_keys where id = $1")
        .bind(access_key_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    bump_all_access_projection_versions(&mut tx, now_utc()).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;

    Ok(DeleteAccessKeyResult {
        access_key_id: key_row.get::<String, _>("id"),
        display_name: key_row.get::<String, _>("display_name"),
    })
}

pub async fn replace_access_bundle_items(
    pool: &PgPool,
    redis_pool: &RedisPool,
    bundle_id: &str,
    platform_access_ids: &[String],
) -> Result<Vec<GatewayAccessBundleItemView>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let platform_access_ids =
        expand_bundle_platform_access_ids_for_openai_compatibility(&mut tx, platform_access_ids)
            .await?;
    sqlx::query("delete from gateway_access_bundle_items where bundle_id = $1")
        .bind(bundle_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    let now = now_utc();
    for platform_access_id in &platform_access_ids {
        sqlx::query(
            r#"
            insert into gateway_access_bundle_items (bundle_id, platform_access_id, created_at)
            values ($1, $2, $3)
            on conflict (bundle_id, platform_access_id) do nothing
            "#,
        )
        .bind(bundle_id)
        .bind(platform_access_id)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    let rows = sqlx::query_as::<_, AccessBundleItemRow>(
        r#"
        select bundle_id, platform_access_id, created_at
        from gateway_access_bundle_items
        where bundle_id = $1
        order by platform_access_id asc
        "#,
    )
    .bind(bundle_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows.into_iter().map(to_access_bundle_item_view).collect())
}

async fn resolve_bundle_ids_for_key(
    tx: &mut Transaction<'_, Postgres>,
    resolved_project_id: &str,
    explicit_bundle_ids: &[String],
) -> Result<Vec<String>, GatewayError> {
    if !explicit_bundle_ids.is_empty() {
        let requested_ids = explicit_bundle_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        let allowed_ids = sqlx::query_scalar::<_, String>(
            r#"
            select id
            from gateway_access_bundles
            where id = any($1)
              and status = 'active'
              and (project_id = $2 or project_id is null)
            order by id asc
            "#,
        )
        .bind(&requested_ids)
        .bind(resolved_project_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_db_error)?;
        return ensure_allowed_resource_ids(&requested_ids, &allowed_ids, "access bundle");
    }
    sqlx::query_scalar::<_, String>(
        r#"
        select id
        from gateway_access_bundles
        where project_id = $1 and status = 'active'
        order by created_at asc, id asc
        "#,
    )
    .bind(resolved_project_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(map_db_error)
}

async fn list_access_key_rows(
    pool: &PgPool,
    access_key_id: &str,
) -> Result<AccessKeyRow, GatewayError> {
    sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access key 不存在"))
}

pub async fn save_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: Option<&str>,
    api_key_secret: Option<&str>,
    input: UpsertAccessKeyInput,
) -> Result<GatewayAccessKeyView, GatewayError> {
    let boundary = AccessBoundary::new(&input.resolved_tenant_id, &input.resolved_project_id)?;
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    ensure_project_tenant_boundary(&mut tx, &boundary).await?;
    let bundle_ids =
        resolve_bundle_ids_for_key(&mut tx, &input.resolved_project_id, &input.bundle_ids).await?;
    let now = now_utc();
    let key_kind = normalize_key_kind(&input.key_kind);
    let owner_type = normalize_owner_type(&input.owner_type);
    let public_key_prefix = normalize_public_key_prefix(&owner_type, &input.public_key_prefix);
    let expires_at = parse_optional_timestamp(input.expires_at.as_deref());
    let access_key_id = if let Some(id) = access_key_id {
        sqlx::query(
            r#"
            update gateway_access_keys
            set
              owner_type = $2,
              owner_id = $3,
              resolved_project_id = $4,
              resolved_tenant_id = $5,
              key_kind = $6,
              public_key_prefix = $7,
              display_name = $8,
              expires_at = $9,
              metadata = $10,
              updated_at = $11
            where id = $1
            "#,
        )
        .bind(id)
        .bind(&owner_type)
        .bind(input.owner_id.trim())
        .bind(input.resolved_project_id.trim())
        .bind(input.resolved_tenant_id.trim())
        .bind(&key_kind)
        .bind(&public_key_prefix)
        .bind(input.display_name.trim())
        .bind(expires_at)
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        id.to_string()
    } else {
        let id = Uuid::new_v4().to_string();
        let external_key = if public_key_prefix == "gw-user" {
            Some(format!("gw-user-{}", Uuid::new_v4().simple()))
        } else {
            None
        };
        sqlx::query(
            r#"
            insert into gateway_access_keys (
              id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status,
              public_key_prefix, external_key, display_name, rotated_from_access_key_id,
              legacy_gateway_api_key_id, legacy_user_credential_id, expires_at, last_used_at, metadata,
              revoked_at, revoke_reason, created_at, updated_at
            ) values (
              $1, $2, $3, $4, $5, $6, 'active', $7, $8, $9, null, null, null, $10, null, $11, null, null, $12, $12
            )
            "#,
        )
        .bind(&id)
        .bind(&owner_type)
        .bind(input.owner_id.trim())
        .bind(input.resolved_project_id.trim())
        .bind(input.resolved_tenant_id.trim())
        .bind(&key_kind)
        .bind(&public_key_prefix)
        .bind(external_key)
        .bind(input.display_name.trim())
        .bind(expires_at)
        .bind(input.metadata.clone().map(Json))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        id
    };

    sqlx::query("delete from gateway_access_key_bundle_bindings where access_key_id = $1")
        .bind(&access_key_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    if key_kind != "auto_route" {
        for bundle_id in &bundle_ids {
            sqlx::query(
                r#"
                insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id, created_at)
                values ($1, $2, $3)
                on conflict (access_key_id, bundle_id) do nothing
                "#,
            )
            .bind(&access_key_id)
            .bind(bundle_id)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }
    }

    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    let row = list_access_key_rows(pool, &access_key_id).await?;
    to_access_key_view(row, api_key_secret)
}

pub async fn rotate_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    api_key_secret: Option<&str>,
) -> Result<GatewayAccessKeyView, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let current = sqlx::query_as::<_, AccessKeyRow>(
        r#"
        select
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status, public_key_prefix,
          display_name, external_key, rotated_from_access_key_id, legacy_gateway_api_key_id, legacy_user_credential_id,
          expires_at, last_used_at, metadata, revoked_at, revoke_reason, created_at, updated_at
        from gateway_access_keys
        where id = $1
        for update
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("access key 不存在"))?;
    let now = now_utc();
    sqlx::query(
        r#"
        update gateway_access_keys
        set
          status = 'revoked',
          revoked_at = $2,
          revoke_reason = 'rotated',
          legacy_gateway_api_key_id = null,
          legacy_user_credential_id = null,
          updated_at = $2
        where id = $1
        "#,
    )
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let next_id = Uuid::new_v4().to_string();
    let external_key = if current.public_key_prefix == "gw-user" {
        Some(format!("gw-user-{}", Uuid::new_v4().simple()))
    } else {
        None
    };
    sqlx::query(
        r#"
        insert into gateway_access_keys (
          id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status,
          public_key_prefix, external_key, display_name, rotated_from_access_key_id,
          legacy_gateway_api_key_id, legacy_user_credential_id, expires_at, last_used_at, metadata,
          revoked_at, revoke_reason, created_at, updated_at
        ) values (
          $1, $2, $3, $4, $5, $6, 'active', $7, $8, $9, $10, $11, $12, $13, null, $14, null, null, $15, $15
        )
        "#,
    )
    .bind(&next_id)
    .bind(&current.owner_type)
    .bind(&current.owner_id)
    .bind(&current.resolved_project_id)
    .bind(&current.resolved_tenant_id)
    .bind(&current.key_kind)
    .bind(&current.public_key_prefix)
    .bind(external_key)
    .bind(&current.display_name)
    .bind(&current.id)
    .bind(&current.legacy_gateway_api_key_id)
    .bind(&current.legacy_user_credential_id)
    .bind(current.expires_at)
    .bind(current.metadata.clone())
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id, created_at)
        select $1, bundle_id, $3
        from gateway_access_key_bundle_bindings
        where access_key_id = $2
        on conflict (access_key_id, bundle_id) do nothing
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_balances (
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        )
        select
          $1, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, $3
        from gateway_access_key_balances
        where access_key_id = $2
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_aggregate_memberships (
          aggregate_access_key_id, member_access_key_id, priority, created_at
        )
        select $1, member_access_key_id, priority, $3
        from gateway_access_key_aggregate_memberships
        where aggregate_access_key_id = $2
        on conflict (aggregate_access_key_id, member_access_key_id) do nothing
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        insert into gateway_access_key_aggregate_memberships (
          aggregate_access_key_id, member_access_key_id, priority, created_at
        )
        select aggregate_access_key_id, $1, priority, $3
        from gateway_access_key_aggregate_memberships
        where member_access_key_id = $2
        on conflict (aggregate_access_key_id, member_access_key_id) do nothing
        "#,
    )
    .bind(&next_id)
    .bind(access_key_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;

    clear_access_projection_cache(redis_pool).await?;
    let row = list_access_key_rows(pool, &next_id).await?;
    to_access_key_view(row, api_key_secret)
}

pub async fn revoke_access_key(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    reason: Option<&str>,
) -> Result<(), GatewayError> {
    let result = sqlx::query(
        r#"
        update gateway_access_keys
        set status = 'revoked', revoked_at = $2, revoke_reason = $3, updated_at = $2
        where id = $1
        "#,
    )
    .bind(access_key_id)
    .bind(now_utc())
    .bind(reason)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    if result.rows_affected() == 0 {
        return Err(GatewayError::not_found("access key 不存在"));
    }
    clear_access_projection_cache(redis_pool).await?;
    Ok(())
}

pub async fn adjust_access_key_balance(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    input: AccessKeyBalanceAdjustInput,
) -> Result<GatewayAccessKeyBalanceView, GatewayError> {
    let now = now_utc();
    let current = sqlx::query_as::<_, AccessKeyBalanceRow>(
        r#"
        select
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        from gateway_access_key_balances
        where access_key_id = $1
        limit 1
        "#,
    )
    .bind(access_key_id)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    let mut cached =
        current
            .as_ref()
            .map(map_balance_row_to_cache)
            .unwrap_or(CachedAccessKeyBalance {
                access_key_id: access_key_id.to_string(),
                balance_mode: input
                    .balance_mode
                    .clone()
                    .map(|value| normalize_balance_mode(&value))
                    .unwrap_or_else(|| "token_prepaid".to_string()),
                status: input.status.clone().unwrap_or_else(|| "active".to_string()),
                unlimited_until: input.unlimited_until.clone(),
                period_starts_at: input.period_starts_at.clone(),
                period_ends_at: input.period_ends_at.clone(),
                total_tokens: input.total_tokens,
                remaining_tokens: input.remaining_tokens.or(input.total_tokens),
                total_messages: input.total_messages,
                remaining_messages: input.remaining_messages.or(input.total_messages),
                updated_at: format_timestamp(now),
            });

    if let Some(value) = input.balance_mode {
        cached.balance_mode = normalize_balance_mode(&value);
    }
    if let Some(value) = input.status {
        cached.status = value;
    }
    if input.unlimited_until.is_some() {
        cached.unlimited_until = input.unlimited_until;
    }
    if input.period_starts_at.is_some() {
        cached.period_starts_at = input.period_starts_at;
    }
    if input.period_ends_at.is_some() {
        cached.period_ends_at = input.period_ends_at;
    }
    if let Some(value) = input.total_tokens {
        cached.total_tokens = Some(value);
        if current.is_none() && input.remaining_tokens.is_none() {
            cached.remaining_tokens = Some(value);
        }
    }
    if let Some(value) = input.remaining_tokens {
        cached.remaining_tokens = Some(value);
    }
    if let Some(value) = input.total_messages {
        cached.total_messages = Some(value);
        if current.is_none() && input.remaining_messages.is_none() {
            cached.remaining_messages = Some(value);
        }
    }
    if let Some(value) = input.remaining_messages {
        cached.remaining_messages = Some(value);
    }
    apply_balance_delta(
        &mut cached,
        input.token_delta.unwrap_or_default(),
        input.message_delta.unwrap_or_default(),
    );

    sqlx::query(
        r#"
        insert into gateway_access_key_balances (
          access_key_id, balance_mode, status, unlimited_until, period_starts_at, period_ends_at,
          total_tokens, remaining_tokens, total_messages, remaining_messages, updated_at
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        on conflict (access_key_id) do update
        set
          balance_mode = excluded.balance_mode,
          status = excluded.status,
          unlimited_until = excluded.unlimited_until,
          period_starts_at = excluded.period_starts_at,
          period_ends_at = excluded.period_ends_at,
          total_tokens = excluded.total_tokens,
          remaining_tokens = excluded.remaining_tokens,
          total_messages = excluded.total_messages,
          remaining_messages = excluded.remaining_messages,
          updated_at = excluded.updated_at
        "#,
    )
    .bind(access_key_id)
    .bind(&cached.balance_mode)
    .bind(&cached.status)
    .bind(parse_optional_timestamp(cached.unlimited_until.as_deref()))
    .bind(parse_optional_timestamp(cached.period_starts_at.as_deref()))
    .bind(parse_optional_timestamp(cached.period_ends_at.as_deref()))
    .bind(cached.total_tokens)
    .bind(cached.remaining_tokens)
    .bind(cached.total_messages)
    .bind(cached.remaining_messages)
    .bind(now)
    .execute(pool)
    .await
    .map_err(map_db_error)?;
    cache_balance(redis_pool, &cached).await?;
    Ok(GatewayAccessKeyBalanceView {
        access_key_id: cached.access_key_id,
        balance_mode: cached.balance_mode,
        status: cached.status,
        unlimited_until: cached.unlimited_until,
        period_starts_at: cached.period_starts_at,
        period_ends_at: cached.period_ends_at,
        total_tokens: cached.total_tokens,
        remaining_tokens: cached.remaining_tokens,
        total_messages: cached.total_messages,
        remaining_messages: cached.remaining_messages,
        updated_at: cached.updated_at,
    })
}

pub async fn replace_access_key_aggregate_memberships(
    pool: &PgPool,
    redis_pool: &RedisPool,
    aggregate_access_key_id: &str,
    memberships: &[AggregateMembershipInput],
) -> Result<Vec<GatewayAccessKeyAggregateMembershipView>, GatewayError> {
    let mut tx = pool.begin().await.map_err(map_db_error)?;
    let memberships =
        validate_aggregate_membership_boundaries(&mut tx, aggregate_access_key_id, memberships)
            .await?;
    sqlx::query(
        "delete from gateway_access_key_aggregate_memberships where aggregate_access_key_id = $1",
    )
    .bind(aggregate_access_key_id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;
    let now = now_utc();
    for membership in &memberships {
        sqlx::query(
            r#"
            insert into gateway_access_key_aggregate_memberships (
              aggregate_access_key_id, member_access_key_id, priority, created_at
            ) values ($1, $2, $3, $4)
            "#,
        )
        .bind(aggregate_access_key_id)
        .bind(membership.member_access_key_id.trim())
        .bind(membership.priority)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }
    bump_all_access_projection_versions(&mut tx, now).await?;
    tx.commit().await.map_err(map_db_error)?;
    clear_access_projection_cache(redis_pool).await?;
    let rows = sqlx::query_as::<_, AggregateMembershipRow>(
        r#"
        select aggregate_access_key_id, member_access_key_id, priority, created_at
        from gateway_access_key_aggregate_memberships
        where aggregate_access_key_id = $1
        order by priority asc, member_access_key_id asc
        "#,
    )
    .bind(aggregate_access_key_id)
    .fetch_all(pool)
    .await
    .map_err(map_db_error)?;
    Ok(rows.into_iter().map(to_aggregate_membership_view).collect())
}

async fn ensure_project_tenant_boundary(
    tx: &mut Transaction<'_, Postgres>,
    expected: &AccessBoundary,
) -> Result<(), GatewayError> {
    let actual_tenant_id = sqlx::query_scalar::<_, String>(
        r#"
        select project.tenant_id
        from gateway_projects project
        join gateway_tenants tenant on tenant.id = project.tenant_id
        where project.id = $1
          and project.status = 'active'
          and tenant.status = 'active'
        limit 1
        "#,
    )
    .bind(&expected.project_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| {
        GatewayError::bad_request("resolved project is not active")
            .with_code("access_boundary_mismatch")
    })?;
    let actual = AccessBoundary::new(&actual_tenant_id, &expected.project_id)?;
    expected.ensure_same(&actual, "access key")
}

async fn validate_aggregate_membership_boundaries(
    tx: &mut Transaction<'_, Postgres>,
    aggregate_access_key_id: &str,
    memberships: &[AggregateMembershipInput],
) -> Result<Vec<AggregateMembershipInput>, GatewayError> {
    let aggregate = sqlx::query(
        r#"
        select key_kind, status
        from gateway_access_keys
        where id = $1
        limit 1
        "#,
    )
    .bind(aggregate_access_key_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_db_error)?
    .ok_or_else(|| GatewayError::not_found("aggregate access key does not exist"))?;
    if normalize_key_kind(aggregate.get::<String, _>("key_kind").as_str()) != "auto_route"
        || normalize_status(aggregate.get::<String, _>("status").as_str()) != "active"
    {
        return Err(GatewayError::bad_request(
            "aggregate access key must be an active auto_route key",
        )
        .with_code("access_boundary_mismatch"));
    }

    let requested_ids = memberships
        .iter()
        .map(|membership| membership.member_access_key_id.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let allowed_ids = if requested_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar::<_, String>(
            r#"
            select member.id
            from gateway_access_keys member
            join gateway_access_keys aggregate on aggregate.id = $1
            where member.id = any($2)
              and member.id <> aggregate.id
              and member.status = 'active'
              and member.resolved_project_id = aggregate.resolved_project_id
              and member.resolved_tenant_id = aggregate.resolved_tenant_id
            order by member.id asc
            "#,
        )
        .bind(aggregate_access_key_id)
        .bind(&requested_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_db_error)?
    };
    let allowed_ids =
        ensure_allowed_resource_ids(&requested_ids, &allowed_ids, "aggregate access key member")?;
    let priorities = memberships
        .iter()
        .map(|membership| (membership.member_access_key_id.trim(), membership.priority))
        .collect::<HashMap<_, _>>();

    Ok(allowed_ids
        .into_iter()
        .map(|member_access_key_id| AggregateMembershipInput {
            priority: priorities
                .get(member_access_key_id.as_str())
                .copied()
                .unwrap_or_default(),
            member_access_key_id,
        })
        .collect())
}

pub async fn inspect_access_sticky_affinity(
    redis_pool: &RedisPool,
    requesting_access_key_id: &str,
    model: &str,
    explicit_session_key: Option<&str>,
) -> Result<Option<GatewayAccessStickyAffinityView>, GatewayError> {
    let key = sticky_affinity_key(requesting_access_key_id, model, explicit_session_key);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let raw: Option<String> = conn
        .get(&key)
        .await
        .map_err(|error| GatewayError::server_error(format!("read sticky affinity: {error}")))?;
    raw.map(|value| {
        serde_json::from_str::<GatewayAccessStickyAffinityView>(&value).map_err(|error| {
            GatewayError::server_error(format!("deserialize sticky affinity: {error}"))
        })
    })
    .transpose()
}

pub async fn reset_access_sticky_affinity(
    redis_pool: &RedisPool,
    requesting_access_key_id: &str,
    model: &str,
    explicit_session_key: Option<&str>,
) -> Result<(), GatewayError> {
    let key = sticky_affinity_key(requesting_access_key_id, model, explicit_session_key);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = conn
        .del(key)
        .await
        .map_err(|error| GatewayError::server_error(format!("delete sticky affinity: {error}")))?;
    Ok(())
}

pub async fn record_access_sticky_affinity(
    redis_pool: &RedisPool,
    requesting_access_key_id: &str,
    source_access_key_id: &str,
    platform_access_id: &str,
    provider_account_id: &str,
    real_credential_ref: Option<&str>,
    model: &str,
    explicit_session_key: Option<&str>,
) -> Result<(), GatewayError> {
    let view = GatewayAccessStickyAffinityView {
        scope: sticky_affinity_scope(explicit_session_key),
        model: model.to_string(),
        requesting_access_key_id: requesting_access_key_id.to_string(),
        source_access_key_id: source_access_key_id.to_string(),
        platform_access_id: platform_access_id.to_string(),
        provider_account_id: provider_account_id.to_string(),
        real_credential_ref: real_credential_ref.map(str::to_string),
        expires_at: Some(format_timestamp(now_utc() + time::Duration::hours(1))),
    };
    let payload = serde_json::to_string(&view).map_err(|error| {
        GatewayError::server_error(format!("serialize sticky affinity: {error}"))
    })?;
    let key = sticky_affinity_key(requesting_access_key_id, model, explicit_session_key);
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|error| GatewayError::server_error(format!("get redis connection: {error}")))?;
    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg(payload)
        .arg("EX")
        .arg(ACCESS_AFFINITY_TTL_SECS)
        .query_async(&mut conn)
        .await
        .map_err(|error| GatewayError::server_error(format!("write sticky affinity: {error}")))?;
    Ok(())
}

fn route_rows_for_endpoint(
    projection: &CachedAccessProjection,
    model: &str,
    endpoint_kind: EndpointKind,
) -> Vec<ProjectedPlatformAccessRow> {
    let endpoint_kind = endpoint_kind_name(endpoint_kind);
    let rows = projection
        .rows_by_model
        .get(model)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    let exact_rows = rows
        .iter()
        .filter(|row| row.endpoint_kind == endpoint_kind)
        .cloned()
        .collect::<Vec<_>>();
    if !exact_rows.is_empty() {
        return exact_rows;
    }

    for compatible_endpoint_kind in openai_text_endpoint_family(endpoint_kind) {
        let compatible_rows = rows
            .iter()
            .filter(|row| row.endpoint_kind == *compatible_endpoint_kind)
            .cloned()
            .collect::<Vec<_>>();
        if !compatible_rows.is_empty() {
            return compatible_rows;
        }
    }
    Vec::new()
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

fn credential_payload_supports_route_row(
    credential_payload: &Value,
    row: &ProjectedPlatformAccessRow,
) -> bool {
    let model_candidates = [Some(row.model_code.as_str()), row.upstream_model.as_deref()]
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

pub async fn preview_access_candidates(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    requested_model: &str,
    endpoint_kind: EndpointKind,
    estimated_tokens: u64,
    explicit_session_key: Option<&str>,
) -> Result<Vec<GatewayAccessCandidatePreviewView>, GatewayError> {
    let access_key = find_access_key_auth_by_id(pool, access_key_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("access key 不存在"))?;
    validate_access_key_auth(&access_key)?;
    let projection = get_or_rebuild_access_projection(pool, redis_pool, &access_key).await?;
    let sticky = inspect_access_sticky_affinity(
        redis_pool,
        access_key_id,
        requested_model,
        explicit_session_key,
    )
    .await?;
    let rows = route_rows_for_endpoint(&projection, requested_model, endpoint_kind);
    let provider_account_ids = rows
        .iter()
        .map(|row| row.provider_account_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let credential_map = load_provider_credential_map_for_rows(pool, &provider_account_ids).await?;
    let accounts_with_credentials =
        load_provider_accounts_with_any_credentials(pool, &provider_account_ids).await?;
    let mut previews = Vec::with_capacity(rows.len());
    for row in rows {
        let balance = evaluate_access_key_balance(
            pool,
            redis_pool,
            &row.source_access_key_id,
            estimated_tokens,
        )
        .await?;
        if let Some(credentials) = credential_map.get(row.provider_account_id.as_str()) {
            for credential in credentials {
                if !credential_payload_supports_route_row(&credential.payload, &row) {
                    continue;
                }
                previews.push(GatewayAccessCandidatePreviewView {
                    requesting_access_key_id: row.requesting_access_key_id.clone(),
                    source_access_key_id: row.source_access_key_id.clone(),
                    platform_access_id: row.platform_access_id.clone(),
                    provider_account_id: row.provider_account_id.clone(),
                    provider_credential_id: Some(credential.id.clone()),
                    model_code: row.model_code.clone(),
                    endpoint_kind: row.endpoint_kind.clone(),
                    upstream_model: row.upstream_model.clone(),
                    platform_tier: row.platform_tier.clone(),
                    operator_weight: row.operator_weight,
                    routing_priority: row.routing_priority,
                    sticky_matched: sticky.as_ref().is_some_and(|value| {
                        value.platform_access_id == row.platform_access_id
                            && value.real_credential_ref.as_deref() == Some(credential.id.as_str())
                    }),
                    available_by_balance: balance.allowed,
                    balance_mode: balance.balance_mode.clone(),
                    remaining_tokens: balance.remaining_tokens,
                    remaining_messages: balance.remaining_messages,
                });
            }
        } else {
            if accounts_with_credentials.contains(row.provider_account_id.as_str()) {
                continue;
            }
            previews.push(GatewayAccessCandidatePreviewView {
                requesting_access_key_id: row.requesting_access_key_id.clone(),
                source_access_key_id: row.source_access_key_id.clone(),
                platform_access_id: row.platform_access_id.clone(),
                provider_account_id: row.provider_account_id.clone(),
                provider_credential_id: None,
                model_code: row.model_code.clone(),
                endpoint_kind: row.endpoint_kind.clone(),
                upstream_model: row.upstream_model.clone(),
                platform_tier: row.platform_tier.clone(),
                operator_weight: row.operator_weight,
                routing_priority: row.routing_priority,
                sticky_matched: sticky.as_ref().is_some_and(|value| {
                    value.platform_access_id == row.platform_access_id
                        && value.real_credential_ref.is_none()
                }),
                available_by_balance: balance.allowed,
                balance_mode: balance.balance_mode,
                remaining_tokens: balance.remaining_tokens,
                remaining_messages: balance.remaining_messages,
            });
        }
    }
    previews.sort_by(|left, right| {
        right
            .sticky_matched
            .cmp(&left.sticky_matched)
            .then(right.routing_priority.cmp(&left.routing_priority))
            .then(right.operator_weight.cmp(&left.operator_weight))
            .then(
                platform_tier_rank(&right.platform_tier)
                    .cmp(&platform_tier_rank(&left.platform_tier)),
            )
            .then(left.provider_account_id.cmp(&right.provider_account_id))
            .then(
                left.provider_credential_id
                    .as_deref()
                    .unwrap_or_default()
                    .cmp(right.provider_credential_id.as_deref().unwrap_or_default()),
            )
    });
    Ok(previews)
}

pub async fn resolve_access_key_route_context(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    requested_model: &str,
    endpoint_kind: EndpointKind,
    estimated_tokens: u64,
    explicit_session_key: Option<&str>,
) -> Result<ResolvedAccessKeyRouteContext, GatewayError> {
    let access_key = find_access_key_auth_by_id(pool, access_key_id)
        .await?
        .ok_or_else(|| GatewayError::not_found("access key 不存在"))?;
    validate_access_key_auth(&access_key)?;
    let projection = get_or_rebuild_access_projection(pool, redis_pool, &access_key).await?;
    let sticky = inspect_access_sticky_affinity(
        redis_pool,
        access_key_id,
        requested_model,
        explicit_session_key,
    )
    .await?;
    let rows = route_rows_for_endpoint(&projection, requested_model, endpoint_kind);
    let provider_account_ids = rows
        .iter()
        .map(|row| row.provider_account_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let provider_meta = load_provider_candidate_meta_map(pool, &provider_account_ids).await?;
    let credential_map = load_provider_credential_map_for_rows(pool, &provider_account_ids).await?;
    let accounts_with_credentials =
        load_provider_accounts_with_any_credentials(pool, &provider_account_ids).await?;

    let mut selected = None;
    let mut route_candidates = Vec::new();
    for row in rows {
        let balance = evaluate_access_key_balance(
            pool,
            redis_pool,
            &row.source_access_key_id,
            estimated_tokens,
        )
        .await?;
        if !balance.allowed {
            continue;
        }
        let Some(provider_row) = provider_meta.get(&row.provider_account_id) else {
            continue;
        };
        if provider_row
            .cooldown_until
            .is_some_and(|value| value > now_utc())
        {
            continue;
        }
        let sticky_matched = sticky
            .as_ref()
            .is_some_and(|value| value.platform_access_id == row.platform_access_id);
        let account_payload = get_provider_payload(pool, &row.provider_account_id)
            .await?
            .map(|view| view.payload);
        if let Some(credentials) = credential_map.get(row.provider_account_id.as_str()) {
            let Some(account_payload) = account_payload.clone() else {
                continue;
            };
            for credential in credentials {
                if !credential_payload_supports_route_row(&credential.payload, &row) {
                    continue;
                }
                let merged_payload = merge_provider_account_and_credential_payloads(
                    &account_payload,
                    &credential.payload,
                );
                let supported_protocol_families =
                    resolve_supported_wire_protocol_families_for_model(
                        Some(&merged_payload),
                        Some(requested_model),
                        row.upstream_model.as_deref(),
                        &provider_row.adapter,
                        &provider_row.protocol_family,
                    );
                let mut payload = crate::routing::candidate::deserialize_provider_payload(
                    &merged_payload,
                    Some(&provider_row.adapter),
                )
                .map_err(|error| {
                    GatewayError::server_error(format!(
                        "deserialize provider credential payload for {}: {error}",
                        credential.id
                    ))
                })?;
                if !payload.supports_search_endpoint(endpoint_kind) {
                    continue;
                }
                if payload.default_model.is_none() {
                    payload.default_model = Some(row.model_code.clone());
                }
                payload.credential_id = Some(credential.id.clone());
                let credential_sticky = sticky_matched
                    && sticky
                        .as_ref()
                        .and_then(|value| value.real_credential_ref.as_deref())
                        == Some(credential.id.as_str());
                if selected.is_none() && credential_sticky {
                    selected = Some(row.clone());
                }
                route_candidates.push((
                    credential_sticky,
                    row.clone(),
                    RouteCandidate {
                        provider_account_id: provider_row.id.clone(),
                        provider_credential_id: Some(credential.id.clone()),
                        label: format!("{} / {}", provider_row.label, credential.label),
                        payload,
                        protocol_family: canonicalize_protocol_family_name(
                            &provider_row.protocol_family,
                        ),
                        protocol_profile: provider_row.protocol_profile.clone(),
                        supported_protocol_families,
                        adapter: canonicalize_adapter_name(&provider_row.adapter),
                        model_alias: Some(requested_model.to_string()),
                        upstream_model: row.upstream_model.clone(),
                        resolved_execution_mode: match provider_row.execution_mode.as_str() {
                            "browser_backed" => ProviderExecutionMode::BrowserBacked,
                            _ => ProviderExecutionMode::DirectHttp,
                        },
                        priority: 0,
                        weight: 1,
                        failure_count: credential.failure_count.max(0) as u32,
                        cooldown_until: credential.cooldown_until.clone(),
                        routing_score: None,
                        routing_health_weight: None,
                        routing_capacity_weight: None,
                        routing_degraded: Some(credential.failure_count > 0),
                        routing_breaker_open: Some(false),
                        routing_degradation_reasons: if credential.failure_count > 0 {
                            vec!["provider_credential_failures".to_string()]
                        } else {
                            Vec::new()
                        },
                    },
                ));
            }
        } else {
            if accounts_with_credentials.contains(row.provider_account_id.as_str()) {
                continue;
            }
            let Some(payload_view) = get_provider_payload(pool, &row.provider_account_id).await?
            else {
                continue;
            };
            let supported_protocol_families = resolve_supported_wire_protocol_families_for_model(
                Some(&payload_view.payload),
                Some(requested_model),
                row.upstream_model.as_deref(),
                &provider_row.adapter,
                &provider_row.protocol_family,
            );
            let mut payload = crate::routing::candidate::deserialize_provider_payload(
                &payload_view.payload,
                Some(&provider_row.adapter),
            )
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "deserialize provider payload for {}: {error}",
                    row.provider_account_id
                ))
            })?;
            if !payload.supports_search_endpoint(endpoint_kind) {
                continue;
            }
            if selected.is_none() && sticky_matched {
                selected = Some(row.clone());
            }
            if payload.default_model.is_none() {
                payload.default_model = Some(row.model_code.clone());
            }
            let upstream_model = row.upstream_model.clone();
            route_candidates.push((
                sticky_matched,
                row,
                RouteCandidate {
                    provider_account_id: provider_row.id.clone(),
                    provider_credential_id: None,
                    label: provider_row.label.clone(),
                    payload,
                    protocol_family: canonicalize_protocol_family_name(
                        &provider_row.protocol_family,
                    ),
                    protocol_profile: provider_row.protocol_profile.clone(),
                    supported_protocol_families,
                    adapter: canonicalize_adapter_name(&provider_row.adapter),
                    model_alias: Some(requested_model.to_string()),
                    upstream_model,
                    resolved_execution_mode: match provider_row.execution_mode.as_str() {
                        "browser_backed" => ProviderExecutionMode::BrowserBacked,
                        _ => ProviderExecutionMode::DirectHttp,
                    },
                    priority: 0,
                    weight: 1,
                    failure_count: provider_row.failure_count.max(0) as u32,
                    cooldown_until: provider_row.cooldown_until.map(format_timestamp),
                    routing_score: None,
                    routing_health_weight: None,
                    routing_capacity_weight: None,
                    routing_degraded: Some(provider_row.failure_count > 0),
                    routing_breaker_open: Some(false),
                    routing_degradation_reasons: if provider_row.failure_count > 0 {
                        vec!["provider_failures".to_string()]
                    } else {
                        Vec::new()
                    },
                },
            ));
        }
    }

    route_candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then(right.1.routing_priority.cmp(&left.1.routing_priority))
            .then(right.1.operator_weight.cmp(&left.1.operator_weight))
            .then(
                platform_tier_rank(&right.1.platform_tier)
                    .cmp(&platform_tier_rank(&left.1.platform_tier)),
            )
            .then(left.1.provider_account_id.cmp(&right.1.provider_account_id))
    });
    if selected.is_none() {
        selected = route_candidates.first().map(|entry| entry.1.clone());
    }

    Ok(ResolvedAccessKeyRouteContext {
        requesting_access_key_id: access_key.id.clone(),
        key_kind: access_key.key_kind.clone(),
        model: requested_model.to_string(),
        sticky,
        selected,
        candidates: route_candidates
            .iter()
            .map(|entry| entry.1.clone())
            .collect(),
        route_candidates: route_candidates.into_iter().map(|entry| entry.2).collect(),
    })
}

pub async fn preview_route_decision(
    pool: &PgPool,
    redis_pool: &RedisPool,
    access_key_id: &str,
    requested_model: &str,
    endpoint_kind: EndpointKind,
    estimated_tokens: u64,
    explicit_session_key: Option<&str>,
) -> Result<GatewayAccessRouteDecisionPreviewView, GatewayError> {
    let previews = preview_access_candidates(
        pool,
        redis_pool,
        access_key_id,
        requested_model,
        endpoint_kind,
        estimated_tokens,
        explicit_session_key,
    )
    .await?;
    let selected = previews
        .iter()
        .find(|candidate| candidate.sticky_matched && candidate.available_by_balance)
        .cloned()
        .or_else(|| {
            previews
                .iter()
                .find(|candidate| candidate.available_by_balance)
                .cloned()
        });
    Ok(GatewayAccessRouteDecisionPreviewView {
        requesting_access_key_id: access_key_id.to_string(),
        requested_model: requested_model.to_string(),
        endpoint_kind: endpoint_kind_name(endpoint_kind).to_string(),
        sticky_matched: selected
            .as_ref()
            .is_some_and(|candidate| candidate.sticky_matched),
        selected,
        candidates: previews,
    })
}

pub async fn ensure_default_access_bundle_for_project(
    pool: &PgPool,
    redis_pool: &RedisPool,
    project_id: &str,
    display_name: &str,
) -> Result<GatewayAccessBundleView, GatewayError> {
    let slug = format!("project-{}-default", project_id.trim());
    let existing = sqlx::query_as::<_, AccessBundleRow>(
        r#"
        select id, project_id, slug, display_name, billing_mode, status, description, metadata, created_at, updated_at
        from gateway_access_bundles
        where project_id = $1 and slug = $2
        limit 1
        "#,
    )
    .bind(project_id)
    .bind(&slug)
    .fetch_optional(pool)
    .await
    .map_err(map_db_error)?;
    if let Some(row) = existing {
        return Ok(to_access_bundle_view(row));
    }
    save_access_bundle(
        pool,
        redis_pool,
        None,
        UpsertAccessBundleInput {
            project_id: Some(project_id.to_string()),
            slug,
            display_name: display_name.to_string(),
            billing_mode: "time_pass".to_string(),
            status: "active".to_string(),
            description: Some("默认访问包".to_string()),
            metadata: None,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadpool_redis::{Config as RedisConfig, Runtime as RedisRuntime};
    use sqlx::postgres::PgPoolOptions;

    fn make_projection_row(
        model_code: &str,
        upstream_model: Option<&str>,
    ) -> ProjectedPlatformAccessRow {
        ProjectedPlatformAccessRow {
            requesting_access_key_id: "ak-1".to_string(),
            source_access_key_id: "ak-1".to_string(),
            platform_access_id: "pa-1".to_string(),
            provider_account_id: "provider-1".to_string(),
            model_code: model_code.to_string(),
            endpoint_kind: "responses".to_string(),
            upstream_model: upstream_model.map(str::to_string),
            platform_tier: "high".to_string(),
            operator_weight: 100,
            routing_priority: 10,
        }
    }

    fn make_access_key_auth() -> AccessKeyAuthRecord {
        AccessKeyAuthRecord {
            id: "ak-1".to_string(),
            owner_type: "project".to_string(),
            owner_id: "project-a".to_string(),
            resolved_project_id: "project-a".to_string(),
            resolved_tenant_id: "tenant-a".to_string(),
            key_kind: "normal".to_string(),
            status: "active".to_string(),
            public_key_prefix: "gw-project".to_string(),
            display_name: "test".to_string(),
            external_key: None,
            expires_at: None,
            metadata: None,
            legacy_gateway_api_key_id: None,
            legacy_user_credential_id: None,
            projection_version: "2026-07-19T00:00:00Z".to_string(),
        }
    }

    async fn access_integration_pool() -> PgPool {
        let database_url = std::env::var("GATEWAY_ACCESS_TEST_DATABASE_URL")
            .expect("set GATEWAY_ACCESS_TEST_DATABASE_URL for ignored access integration tests");
        PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("connect access integration PostgreSQL")
    }

    fn access_integration_redis_pool() -> RedisPool {
        let redis_url = std::env::var("GATEWAY_ACCESS_TEST_REDIS_URL")
            .expect("set GATEWAY_ACCESS_TEST_REDIS_URL for ignored access integration tests");
        RedisConfig::from_url(redis_url)
            .create_pool(Some(RedisRuntime::Tokio1))
            .expect("create access integration Redis pool")
    }

    async fn reset_projection_integration_schema(pool: &PgPool) {
        sqlx::raw_sql(
            r#"
            drop table if exists gateway_access_key_bundle_bindings cascade;
            drop table if exists gateway_access_bundle_items cascade;
            drop table if exists gateway_platform_access_catalog cascade;
            drop table if exists gateway_provider_capability_catalog cascade;
            drop table if exists gateway_access_bundles cascade;
            drop table if exists gateway_access_keys cascade;

            create table gateway_access_keys (
              id text primary key,
              resolved_project_id text not null
            );

            create table gateway_access_bundles (
              id text primary key,
              project_id text,
              status text not null
            );

            create table gateway_provider_capability_catalog (
              id text primary key,
              provider_account_id text not null,
              enabled boolean not null
            );

            create table gateway_platform_access_catalog (
              id text primary key,
              provider_capability_id text not null,
              model_code text not null,
              endpoint_kind text not null,
              upstream_model text,
              platform_tier text not null,
              status text not null,
              operator_weight integer not null,
              routing_priority integer not null,
              enabled_for_sale boolean not null,
              notes text,
              created_at timestamptz not null default now(),
              updated_at timestamptz not null default now()
            );

            create table gateway_access_bundle_items (
              bundle_id text not null,
              platform_access_id text not null,
              primary key (bundle_id, platform_access_id)
            );

            create table gateway_access_key_bundle_bindings (
              access_key_id text not null,
              bundle_id text not null,
              primary key (access_key_id, bundle_id)
            );
            "#,
        )
        .execute(pool)
        .await
        .expect("reset projection integration schema");
    }

    async fn reset_rotation_integration_schema(pool: &PgPool) {
        sqlx::raw_sql(
            r#"
            drop table if exists gateway_access_key_aggregate_memberships cascade;
            drop table if exists gateway_access_key_balances cascade;
            drop table if exists gateway_access_key_bundle_bindings cascade;
            drop table if exists gateway_access_bundles cascade;
            drop table if exists gateway_access_keys cascade;

            create table gateway_access_keys (
              id text primary key,
              owner_type text not null,
              owner_id text not null,
              resolved_project_id text not null,
              resolved_tenant_id text not null,
              key_kind text not null,
              status text not null,
              public_key_prefix text not null,
              display_name text not null,
              external_key text,
              rotated_from_access_key_id text references gateway_access_keys(id) on delete set null,
              legacy_gateway_api_key_id text unique,
              legacy_user_credential_id text unique,
              expires_at timestamptz,
              last_used_at timestamptz,
              metadata jsonb,
              revoked_at timestamptz,
              revoke_reason text,
              created_at timestamptz not null,
              updated_at timestamptz not null
            );

            create table gateway_access_bundles (
              id text primary key,
              project_id text,
              slug text not null default 'bundle',
              display_name text not null default 'bundle',
              billing_mode text not null default 'time_pass',
              status text not null,
              description text,
              metadata jsonb,
              created_at timestamptz not null default now(),
              updated_at timestamptz not null default now()
            );

            create table gateway_access_key_bundle_bindings (
              access_key_id text not null references gateway_access_keys(id) on delete cascade,
              bundle_id text not null references gateway_access_bundles(id) on delete cascade,
              created_at timestamptz not null,
              primary key (access_key_id, bundle_id)
            );

            create table gateway_access_key_balances (
              access_key_id text primary key references gateway_access_keys(id) on delete cascade,
              balance_mode text not null,
              status text not null,
              unlimited_until timestamptz,
              period_starts_at timestamptz,
              period_ends_at timestamptz,
              total_tokens bigint,
              remaining_tokens bigint,
              total_messages bigint,
              remaining_messages bigint,
              updated_at timestamptz not null
            );

            create table gateway_access_key_aggregate_memberships (
              aggregate_access_key_id text not null references gateway_access_keys(id) on delete cascade,
              member_access_key_id text not null references gateway_access_keys(id) on delete cascade,
              priority integer not null,
              created_at timestamptz not null,
              primary key (aggregate_access_key_id, member_access_key_id)
            );
            "#,
        )
        .execute(pool)
        .await
        .expect("reset rotation integration schema");
    }

    async fn seed_rotation_access_key(
        pool: &PgPool,
        id: &str,
        project_id: &str,
        tenant_id: &str,
        key_kind: &str,
    ) {
        sqlx::query(
            r#"
            insert into gateway_access_keys (
              id, owner_type, owner_id, resolved_project_id, resolved_tenant_id, key_kind, status,
              public_key_prefix, display_name, created_at, updated_at
            ) values ($1, 'project', $2, $2, $3, $4, 'active', 'gw-project', $1, '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z')
            "#,
        )
        .bind(id)
        .bind(project_id)
        .bind(tenant_id)
        .bind(key_kind)
        .execute(pool)
        .await
        .expect("seed rotation access key");
    }

    #[tokio::test]
    #[ignore = "requires isolated GATEWAY_ACCESS_TEST_DATABASE_URL PostgreSQL fixture"]
    async fn normal_projection_excludes_cross_project_bundle_bindings() {
        let pool = access_integration_pool().await;
        reset_projection_integration_schema(&pool).await;
        sqlx::raw_sql(
            r#"
            insert into gateway_access_keys (id, resolved_project_id)
            values ('key-a', 'project-a');

            insert into gateway_provider_capability_catalog (id, provider_account_id, enabled)
            values ('cap-a', 'provider-a', true);

            insert into gateway_platform_access_catalog (
              id, provider_capability_id, model_code, endpoint_kind, upstream_model, platform_tier,
              status, operator_weight, routing_priority, enabled_for_sale
            ) values
              ('access-project-a', 'cap-a', 'model-a', 'responses', 'model-a', 'high', 'active', 10, 100, true),
              ('access-global', 'cap-a', 'model-global', 'responses', 'model-global', 'high', 'active', 10, 100, true),
              ('access-project-b', 'cap-a', 'model-b', 'responses', 'model-b', 'high', 'active', 10, 100, true);

            insert into gateway_access_bundles (id, project_id, status) values
              ('bundle-a', 'project-a', 'active'),
              ('bundle-global', null, 'active'),
              ('bundle-b', 'project-b', 'active');

            insert into gateway_access_bundle_items (bundle_id, platform_access_id) values
              ('bundle-a', 'access-project-a'),
              ('bundle-global', 'access-global'),
              ('bundle-b', 'access-project-b');

            insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id) values
              ('key-a', 'bundle-a'),
              ('key-a', 'bundle-global'),
              ('key-a', 'bundle-b');
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed projection boundary fixture");

        let platform_access_ids = build_projection_rows_for_normal_key(&pool, "key-a")
            .await
            .expect("build normal access projection")
            .into_iter()
            .map(|row| row.platform_access_id)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            platform_access_ids,
            BTreeSet::from(["access-global".to_string(), "access-project-a".to_string(),])
        );
    }

    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL and Redis access fixtures"]
    async fn rotation_preserves_balances_memberships_and_projection_versions() {
        let pool = access_integration_pool().await;
        let redis_pool = access_integration_redis_pool();
        reset_rotation_integration_schema(&pool).await;
        seed_rotation_access_key(&pool, "old-key", "project-a", "tenant-a", "auto_route").await;
        seed_rotation_access_key(&pool, "member-key", "project-a", "tenant-a", "normal").await;
        seed_rotation_access_key(&pool, "parent-key", "project-a", "tenant-a", "auto_route").await;
        sqlx::raw_sql(
            r#"
            update gateway_access_keys
            set legacy_gateway_api_key_id = 'legacy-api-key',
                legacy_user_credential_id = 'legacy-user-credential'
            where id = 'old-key';
            insert into gateway_access_bundles (id, project_id, status)
            values ('bundle-a', 'project-a', 'active');
            insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id, created_at)
            values ('old-key', 'bundle-a', now());
            insert into gateway_access_key_balances (
              access_key_id, balance_mode, status, total_tokens, remaining_tokens,
              total_messages, remaining_messages, updated_at
            ) values ('old-key', 'token_prepaid', 'active', 1000, 725, 40, 29, now());
            insert into gateway_access_key_aggregate_memberships (
              aggregate_access_key_id, member_access_key_id, priority, created_at
            ) values
              ('old-key', 'member-key', 7, now()),
              ('parent-key', 'old-key', 9, now());
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed rotation relationship fixture");

        let rotated = rotate_access_key(&pool, &redis_pool, "old-key", None)
            .await
            .expect("rotate access key");

        let old_status: String =
            sqlx::query_scalar("select status from gateway_access_keys where id = 'old-key'")
                .fetch_one(&pool)
                .await
                .expect("read old key status");
        assert_eq!(old_status, "revoked");

        let legacy_ids = sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "select legacy_gateway_api_key_id, legacy_user_credential_id from gateway_access_keys where id = $1",
        )
        .bind(&rotated.id)
        .fetch_one(&pool)
        .await
        .expect("rotated key legacy identifiers must be preserved");
        assert_eq!(
            legacy_ids,
            (
                Some("legacy-api-key".to_string()),
                Some("legacy-user-credential".to_string()),
            )
        );
        let old_legacy_ids = sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "select legacy_gateway_api_key_id, legacy_user_credential_id from gateway_access_keys where id = 'old-key'",
        )
        .fetch_one(&pool)
        .await
        .expect("read old key legacy identifiers");
        assert_eq!(old_legacy_ids, (None, None));

        let balance = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<i64>,
                Option<i64>,
                Option<i64>,
                Option<i64>,
            ),
        >(
            r#"
            select balance_mode, status, total_tokens, remaining_tokens, total_messages, remaining_messages
            from gateway_access_key_balances where access_key_id = $1
            "#,
        )
        .bind(&rotated.id)
        .fetch_one(&pool)
        .await
        .expect("rotated key balance must be preserved");
        assert_eq!(
            balance,
            (
                "token_prepaid".to_string(),
                "active".to_string(),
                Some(1000),
                Some(725),
                Some(40),
                Some(29),
            )
        );

        let bundle_count: i64 = sqlx::query_scalar(
            "select count(*) from gateway_access_key_bundle_bindings where access_key_id = $1 and bundle_id = 'bundle-a'",
        )
        .bind(&rotated.id)
        .fetch_one(&pool)
        .await
        .expect("read rotated bundle binding");
        assert_eq!(bundle_count, 1);

        let outgoing_priority: i32 = sqlx::query_scalar(
            "select priority from gateway_access_key_aggregate_memberships where aggregate_access_key_id = $1 and member_access_key_id = 'member-key'",
        )
        .bind(&rotated.id)
        .fetch_one(&pool)
        .await
        .expect("rotated aggregate membership must be preserved");
        assert_eq!(outgoing_priority, 7);

        let incoming_priority: i32 = sqlx::query_scalar(
            "select priority from gateway_access_key_aggregate_memberships where aggregate_access_key_id = 'parent-key' and member_access_key_id = $1",
        )
        .bind(&rotated.id)
        .fetch_one(&pool)
        .await
        .expect("rotated member relationship must be preserved");
        assert_eq!(incoming_priority, 9);

        let stale_projection_versions: i64 = sqlx::query_scalar(
            "select count(*) from gateway_access_keys where updated_at <= '1970-01-01T00:00:00Z'::timestamptz",
        )
        .fetch_one(&pool)
        .await
        .expect("read projection versions");
        assert_eq!(stale_projection_versions, 0);
    }

    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL and Redis access fixtures"]
    async fn rotation_rolls_back_revoke_when_relationship_copy_fails() {
        let pool = access_integration_pool().await;
        let redis_pool = access_integration_redis_pool();
        reset_rotation_integration_schema(&pool).await;
        seed_rotation_access_key(&pool, "old-key", "project-a", "tenant-a", "auto_route").await;
        seed_rotation_access_key(&pool, "member-key", "project-a", "tenant-a", "normal").await;
        sqlx::query(
            r#"
            insert into gateway_access_key_aggregate_memberships (
              aggregate_access_key_id, member_access_key_id, priority, created_at
            ) values ('old-key', 'member-key', 7, now())
            "#,
        )
        .execute(&pool)
        .await
        .expect("seed rollback membership");
        sqlx::raw_sql(
            r#"
            create or replace function reject_rotated_membership_copy() returns trigger as $$
            begin
              if new.aggregate_access_key_id <> 'old-key' then
                raise exception 'forced rotated membership copy failure';
              end if;
              return new;
            end;
            $$ language plpgsql;
            create trigger reject_rotated_membership_copy_trigger
            before insert on gateway_access_key_aggregate_memberships
            for each row execute function reject_rotated_membership_copy();
            "#,
        )
        .execute(&pool)
        .await
        .expect("install forced relationship-copy failure");

        rotate_access_key(&pool, &redis_pool, "old-key", None)
            .await
            .expect_err("relationship-copy failure must abort the entire rotation");

        let old_status: String =
            sqlx::query_scalar("select status from gateway_access_keys where id = 'old-key'")
                .fetch_one(&pool)
                .await
                .expect("read old key status after rollback");
        assert_eq!(old_status, "active");
        let replacement_count: i64 = sqlx::query_scalar(
            "select count(*) from gateway_access_keys where rotated_from_access_key_id = 'old-key'",
        )
        .fetch_one(&pool)
        .await
        .expect("count rolled-back replacement keys");
        assert_eq!(replacement_count, 0);
    }

    #[test]
    fn cache_projection_rejects_scope_or_version_mismatch() {
        let access_key = make_access_key_auth();
        let projection = group_projection_rows_by_model(&access_key, Vec::new());
        assert!(cache_projection_is_compatible(&projection, &access_key));

        let mut wrong_scope = projection.clone();
        wrong_scope.resolved_project_id = "project-b".to_string();
        assert!(!cache_projection_is_compatible(&wrong_scope, &access_key));

        let mut stale_version = projection;
        stale_version.projection_version = "2026-07-18T00:00:00Z".to_string();
        assert!(!cache_projection_is_compatible(&stale_version, &access_key));
    }

    #[test]
    fn expand_projection_rows_with_aliases_adds_alias_entry() {
        let rows = vec![make_projection_row("gpt-5.4", Some("gpt-5.4"))];
        let aliases = vec![ModelAliasProjectionRow {
            alias: "gpt-5".to_string(),
            provider_account_id: "provider-1".to_string(),
            upstream_model: Some("gpt-5.4".to_string()),
        }];

        let expanded = expand_projection_rows_with_aliases(rows, &aliases);
        assert!(expanded.iter().any(|row| row.model_code == "gpt-5.4"));
        assert!(expanded.iter().any(|row| row.model_code == "gpt-5"));
    }

    #[test]
    fn expand_projection_rows_with_aliases_ignores_provider_mismatch() {
        let rows = vec![make_projection_row("gpt-5.4", Some("gpt-5.4"))];
        let aliases = vec![ModelAliasProjectionRow {
            alias: "gpt-5".to_string(),
            provider_account_id: "provider-2".to_string(),
            upstream_model: Some("gpt-5.4".to_string()),
        }];

        let expanded = expand_projection_rows_with_aliases(rows, &aliases);
        assert!(!expanded.iter().any(|row| row.model_code == "gpt-5"));
    }

    #[test]
    fn credential_route_row_filter_honors_supported_models() {
        let row = make_projection_row("qwen3.5-35b-a3b", Some("astron-code-latest"));
        let payload = serde_json::json!({
            "supportedModels": ["qwen3.5-35b-a3b"]
        });
        assert!(credential_payload_supports_route_row(&payload, &row));

        let mismatched = serde_json::json!({
            "supportedModels": ["qwen3.5-2b"]
        });
        assert!(!credential_payload_supports_route_row(&mismatched, &row));
    }

    #[test]
    fn collect_model_catalog_ids_from_access_projection_returns_sorted_alias_only_models() {
        let mut rows = vec![
            make_projection_row("gpt-5.4", Some("gpt-5.4")),
            make_projection_row("gpt-5.4", Some("gpt-5.4")),
            make_projection_row("claude-sonnet-4-6", Some("claude-sonnet-4-6")),
        ];
        rows.push(ProjectedPlatformAccessRow {
            model_code: "gpt-5".to_string(),
            ..make_projection_row("gpt-5.4", Some("gpt-5.4"))
        });

        let projection = group_projection_rows_by_model(&make_access_key_auth(), rows);
        let model_ids = collect_model_catalog_ids_from_access_projection(&projection);

        assert_eq!(
            model_ids,
            vec!["claude-sonnet-4-6".to_string(), "gpt-5".to_string()]
        );
    }

    #[test]
    fn collect_model_catalog_ids_from_access_projection_hides_upstream_model_when_alias_exists() {
        let rows = vec![
            make_projection_row(
                "nvidia/llama-3.3-nemotron-super-49b-v1.5",
                Some("nvidia/llama-3.3-nemotron-super-49b-v1.5"),
            ),
            ProjectedPlatformAccessRow {
                model_code: "nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string(),
                ..make_projection_row(
                    "nvidia/llama-3.3-nemotron-super-49b-v1.5",
                    Some("nvidia/llama-3.3-nemotron-super-49b-v1.5"),
                )
            },
        ];

        let projection = group_projection_rows_by_model(&make_access_key_auth(), rows);
        let model_ids = collect_model_catalog_ids_from_access_projection(&projection);

        assert_eq!(
            model_ids,
            vec!["nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string()]
        );
    }
}
