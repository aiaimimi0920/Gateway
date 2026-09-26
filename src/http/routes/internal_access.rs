//! Management catalog, bundle, key and route-diagnostic entry paths.

mod access_keys;
mod bundles;
mod catalog;
mod routing_diagnostics;

#[cfg(test)]
mod tests;

pub use catalog::{
    create_platform_access, create_provider_capability, get_access_catalog, update_platform_access,
    update_provider_capability, AccessPath, CatalogQuery, PlatformAccessBody,
    ProviderCapabilityBody,
};

pub use bundles::{
    create_access_bundle, delete_access_bundle, ensure_bundle_user_key, replace_bundle_items,
    update_access_bundle, AccessBundleBody, BundlePath, EnsureBundleUserKeyBody,
    ReplaceBundleItemsBody,
};

pub use access_keys::{
    adjust_access_key_balance, create_access_key, delete_access_key, get_access_key_balance,
    replace_aggregate_memberships, revoke_access_key, rotate_access_key, update_access_key,
    AccessKeyBody, AccessKeyPath, AggregateMembershipBody, BalanceAdjustBody,
    ReplaceAggregateMembershipsBody, RevokeAccessKeyBody,
};

pub use routing_diagnostics::{
    inspect_affinity, preview_candidates, preview_route_decision, reset_affinity, AffinityQuery,
    CandidatePreviewQuery,
};

use crate::error::GatewayError;
use crate::state::AppState;

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}
