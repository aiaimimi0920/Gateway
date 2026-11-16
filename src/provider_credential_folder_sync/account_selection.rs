//! Per-scan account selection owns the metadata index and existing bounded payload cache.
use super::account_cache::HydratedAccountCache;
use super::accounts::{
    build_provider_maps, select_provider_account_for_path, FolderSyncProviderMaps,
};
use super::canonicalization::canonicalize_folder_service_provider_slug;
use super::classification::ProviderAccountDescriptor;
use super::layout::FolderImportPathDescriptor;
use super::limits::PROVIDER_ACCOUNT_INLINE_PAYLOAD_BYTES;
use crate::{db, error::GatewayError};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::time::{timeout_at, Instant};

// The index retains only public bases, not another copy of hydrated material.
const DISCOVERED_BASE_BYTES: usize = 32 * 1024 * 1024;

#[cfg(test)]
mod tests;

pub(super) struct ImportAccountSelection {
    accounts: Vec<db::ProviderAccountFolderSyncMetadata>,
    maps: FolderSyncProviderMaps,
    cache: HydratedAccountCache,
    kiro_discovered: bool,
    discovery_budget: Duration,
}

impl ImportAccountSelection {
    pub(super) fn new(accounts: Vec<db::ProviderAccountFolderSyncMetadata>) -> Self {
        Self {
            maps: build_provider_maps(&accounts),
            accounts,
            cache: HydratedAccountCache::new(),
            kiro_discovered: false,
            discovery_budget: Duration::from_secs(30),
        }
    }

    pub(super) async fn select(
        &mut self,
        pool: &sqlx::PgPool,
        descriptor: &FolderImportPathDescriptor,
    ) -> Result<Option<String>, GatewayError> {
        self.select_with(descriptor, |id| load_account(pool, id))
            .await
    }

    async fn select_with<L, F>(
        &mut self,
        descriptor: &FolderImportPathDescriptor,
        mut load: L,
    ) -> Result<Option<String>, GatewayError>
    where
        L: FnMut(String) -> F,
        F: Future<Output = Result<db::GatewayProviderAccountView, GatewayError>>,
    {
        if !self.kiro_discovered
            && matches!(
                descriptor.provider_surface_slug.as_str(),
                "kiro-api-key" | "kiro-compatible" | "kiro"
            )
        {
            // One deadline for the entire discovery, not 4,096 per-object timeouts.
            // Async I/O is cancelled; synchronous JSON parsing cannot be preempted.
            let deadline = Instant::now() + self.discovery_budget;
            let mut remaining_base_bytes = DISCOVERED_BASE_BYTES;
            let mut changed = false;
            for index in 0..self.accounts.len() {
                let metadata = &self.accounts[index];
                if !needs_kiro_discovery(metadata) {
                    continue;
                }
                check_deadline(deadline)?;
                let account = timeout_at(
                    deadline,
                    hydrate_with(&mut self.cache, &metadata.id, &mut load),
                )
                .await
                .map_err(|_| discovery_timeout())??;
                // Tokio may accept an immediately-ready future after its deadline.
                check_deadline(deadline)?;
                self.accounts[index].payload_base_url =
                    discovered_base(&account, &mut remaining_base_bytes)?;
                changed = true;
            }
            if changed {
                // Rebuild only after every ambiguous candidate is resolved; otherwise
                // a stale OAuth map or an inline winner could hide a failed object read.
                self.maps = build_provider_maps(&self.accounts);
                check_deadline(deadline)?;
            }
            self.kiro_discovered = true;
        }
        Ok(select_provider_account_for_path(&self.maps, descriptor))
    }

    pub(super) async fn hydrate(
        &mut self,
        pool: &sqlx::PgPool,
        id: &str,
    ) -> Result<Arc<db::GatewayProviderAccountView>, GatewayError> {
        hydrate_with(&mut self.cache, id, &mut |id| load_account(pool, id)).await
    }
}

fn needs_kiro_discovery(account: &db::ProviderAccountFolderSyncMetadata) -> bool {
    account.payload_base_url.is_none()
        && (account.adapter.eq_ignore_ascii_case("kiro_compatible")
            || account.protocol_profile.eq_ignore_ascii_case("kiro")
            || account.protocol_family.eq_ignore_ascii_case("kiro")
            || canonicalize_folder_service_provider_slug(&account.service_provider_key)
                == "kiro-platform")
}

fn discovered_base(
    account: &db::GatewayProviderAccountView,
    remaining_bytes: &mut usize,
) -> Result<Option<String>, GatewayError> {
    let Some(base) = account.payload_base_url() else {
        return Ok(None);
    };
    *remaining_bytes = remaining_bytes.checked_sub(base.len()).ok_or_else(|| {
        GatewayError::bad_request(
            "provider credential folder discovered base metadata exceeds byte limit",
        )
        .with_code("provider_credential_folder_sync_account_discovery_metadata_limit")
    })?;
    Ok(Some(base.to_string()))
}

fn check_deadline(deadline: Instant) -> Result<(), GatewayError> {
    if Instant::now() >= deadline {
        Err(discovery_timeout())
    } else {
        Ok(())
    }
}

fn discovery_timeout() -> GatewayError {
    GatewayError::service_unavailable("provider credential folder account discovery timed out")
        .with_code("provider_credential_folder_sync_account_discovery_timeout")
}

async fn hydrate_with<L, F>(
    cache: &mut HydratedAccountCache,
    id: &str,
    load: &mut L,
) -> Result<Arc<db::GatewayProviderAccountView>, GatewayError>
where
    L: FnMut(String) -> F,
    F: Future<Output = Result<db::GatewayProviderAccountView, GatewayError>>,
{
    if let Some(account) = cache.get(id) {
        return Ok(account);
    }
    Ok(cache.insert(load(id.to_string()).await?))
}

async fn load_account(
    pool: &sqlx::PgPool,
    id: String,
) -> Result<db::GatewayProviderAccountView, GatewayError> {
    db::get_provider_account_for_folder_sync(pool, &id, PROVIDER_ACCOUNT_INLINE_PAYLOAD_BYTES)
        .await?
        .ok_or_else(|| GatewayError::not_found("Provider account 不存在"))
}
