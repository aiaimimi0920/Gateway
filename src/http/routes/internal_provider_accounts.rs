//! Management provider-account routes with domain-owned implementation modules.

mod accio_catalog;
mod account_input;
mod accounts;
mod model_catalog;
mod model_discovery;
mod model_pricing;
mod model_tiering;
mod quota;
mod redaction;
mod source_profile;
mod source_profiles;
mod tiering_storage;

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod tests;

pub use account_input::ProviderAccountBody;

pub use accounts::{
    create_provider_account, delete_provider_account, get_provider_account, get_provider_inventory,
    list_provider_accounts, update_provider_account,
};

pub use source_profile::{
    ProviderSourceProfileBackfillBody, ProviderSourceProfileBody, ProviderSourceProfilePatchBody,
};

pub use source_profiles::{backfill_provider_source_profiles, patch_provider_source_profile};

pub use model_pricing::{
    patch_provider_model_pricing, ProviderModelPricingEntryBody, ProviderModelPricingPatchBody,
};

pub use model_tiering::{
    get_provider_model_tiering, save_provider_model_tiering, ProviderModelTieringSaveBody,
};

pub(crate) use model_discovery::make_provider_payload_serde_compatible;

pub use quota::{
    get_provider_quota, probe_provider_account, refresh_provider_quota,
    sweep_cooling_provider_accounts,
};

use crate::error::GatewayError;
use crate::state::AppState;

fn build_absolute_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    if path.starts_with('/') {
        format!("{base_url}{path}")
    } else {
        format!("{base_url}/{path}")
    }
}

fn required_pg_pool(state: &AppState) -> Result<&sqlx::PgPool, GatewayError> {
    state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("PostgreSQL 尚未配置"))
}
