//! Request-local credential witnesses. Health never mutates the entitlement cache or selects a route.
use std::collections::HashMap;

use sqlx::PgPool;

use super::{
    list_active_provider_credentials_for_accounts, map_db_error, GatewayProviderCredentialView,
};
use crate::error::GatewayError;

#[derive(Default)]
pub(super) struct CatalogCredentials {
    pools: HashMap<String, Vec<GatewayProviderCredentialView>>,
}

impl CatalogCredentials {
    pub(super) async fn load(pool: &PgPool, providers: &[String]) -> Result<Self, GatewayError> {
        if providers.is_empty() {
            return Ok(Self::default());
        }
        // Even an archived/disabled pool must not silently become an account-only credential.
        let owners = sqlx::query_scalar::<_, String>(
            "select distinct provider_account_id from gateway_provider_credentials where provider_account_id = any($1)",
        )
        .bind(providers)
        .fetch_all(pool)
        .await
        .map_err(map_db_error)?;
        let mut pools = owners
            .into_iter()
            .map(|id| (id, Vec::new()))
            .collect::<HashMap<_, _>>();
        for credential in list_active_provider_credentials_for_accounts(pool, providers).await? {
            pools
                .entry(credential.provider_account_id.clone())
                .or_default()
                .push(credential);
        }
        Ok(Self { pools })
    }

    pub(super) fn allows(
        &self,
        provider: &str,
        upstream: &str,
        supports: impl Fn(&GatewayProviderCredentialView) -> bool,
        admit: &impl Fn(&str, Option<&str>, &str) -> bool,
    ) -> bool {
        match self.pools.get(provider) {
            Some(credentials) => credentials.iter().any(|credential| {
                supports(credential) && admit(provider, Some(&credential.id), upstream)
            }),
            None => admit(provider, None, upstream),
        }
    }
}

#[cfg(test)]
mod tests;
