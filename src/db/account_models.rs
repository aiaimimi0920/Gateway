//! Public tenant, project and credential database contracts.

use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayTenantView {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub status: String,
    pub owner_user_id: Option<String>,
    pub source_kind: String,
    pub source_key: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProjectView {
    pub id: String,
    pub tenant_id: String,
    pub slug: String,
    pub display_name: String,
    pub status: String,
    pub source_kind: String,
    pub source_key: String,
    pub default_route_policy_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayApiKeyView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub issued_at: String,
    pub revoked_at: Option<String>,
    pub rotated_from_api_key_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProjectApiAccessView {
    pub project: GatewayProjectView,
    pub tenant: GatewayTenantView,
    pub api_key: GatewayApiKeyView,
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBenefitProjectEnsureView {
    pub tenant: GatewayTenantView,
    pub project: GatewayProjectView,
    pub route_policy: GatewayRoutePolicyView,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayUserCredentialCacheEntry {
    pub id: String,
    pub user_id: String,
    pub project_id: String,
    pub scope: Vec<String>,
    pub expires_at: String,
    pub status: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GatewayProviderPayloadView {
    pub provider_account_id: String,
    pub payload: Value,
    pub storage_mode: String,
    pub status: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ProviderCredentialMutation {
    pub provider_account_id: String,
    pub payload: Value,
    pub ttl_seconds: Option<u64>,
    pub pre_warm: bool,
}

#[derive(Debug, Clone)]
pub struct UserCredentialIssueInput {
    pub user_id: String,
    pub project_id: String,
    pub credential_type: String,
    pub duration_days: i64,
    pub scope: Vec<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IssuedUserCredential {
    pub id: String,
    pub credential_key: String,
    pub expires_at: String,
    pub scope: Vec<String>,
    pub user_id: String,
    pub project_id: String,
    pub tenant_id: String,
    pub credential_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifiedUserCredential {
    pub valid: bool,
    pub credential: Option<GatewayUserCredentialCacheEntry>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchProviderCredentialOutcome {
    pub provider_account_id: String,
    pub action: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchProviderCredentialSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cache_invalidated: u64,
    pub pre_warmed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchProviderCredentialResult {
    pub results: Vec<BatchProviderCredentialOutcome>,
    pub summary: BatchProviderCredentialSummary,
}

#[derive(Debug, Clone)]
pub struct BatchProviderCredentialOperation {
    pub action: String,
    pub provider_account_id: String,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GatewayApiKeyAuthRow {
    pub api_key_id: String,
    pub project_id: String,
    pub tenant_id: String,
    pub api_key_status: String,
    pub project_status: String,
    pub tenant_status: String,
}
