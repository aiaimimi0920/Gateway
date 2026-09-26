use super::*;

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

#[derive(Debug, Clone)]
pub struct AccessBalanceDecision {
    pub allowed: bool,
    pub balance_mode: Option<String>,
    pub pre_deduct_amount: u64,
    pub remaining_tokens: Option<i64>,
    pub remaining_messages: Option<i64>,
    pub reason: Option<String>,
}
