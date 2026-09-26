//! Internal Gateway management admission and explicit domain entry paths.

mod project_access;
mod project_records;
mod usage_reporting;
mod user_credential_cache;
mod user_credentials;

#[cfg(test)]
mod tests;

pub use project_access::{
    ensure_benefit_project, get_project_api_access, resolve_api_access, rotate_api_access,
    rotate_project_api_access, EnsureBenefitProjectBody, GatewayApiAccessResponse, ProjectPath,
    ResolveApiAccessBody, RotateApiAccessBody, RotateProjectApiAccessBody,
};

pub use user_credentials::{
    issue_user_credential, revoke_user_credential, verify_user_credential, IssueUserCredentialBody,
    RevokeUserCredentialBody, SimpleSuccessResponse, UserCredentialIssueResponse,
    VerifyUserCredentialBody,
};

pub use usage_reporting::{
    flush_usage_aggregates, get_project_prompt_cache_trend_report,
    list_provider_credential_model_states, list_usage_aggregates, summarize_project_prompt_cache,
    summarize_usage_aggregates, ProjectPromptCacheQuery, UsageAggregateFlushBody,
};

use crate::access_control::authorize_internal_request;
use crate::access_control::unauthenticated_internal_routes_allowed;
use crate::access_control::InternalAccessSurface;
use crate::error::GatewayError;
use crate::state::AppState;
use axum::http::HeaderMap;

pub(crate) fn assert_management_access(
    state: &AppState,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    assert_management_access_with_expected(
        state.config.gateway_management_token.as_deref(),
        unauthenticated_internal_routes_allowed(),
        bearer_token,
        headers,
    )
}

fn assert_management_access_with_expected(
    expected: Option<&str>,
    allow_unauthenticated: bool,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    authorize_internal_request(
        InternalAccessSurface::Management,
        expected,
        allow_unauthenticated,
        bearer_token,
        headers,
    )
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}
