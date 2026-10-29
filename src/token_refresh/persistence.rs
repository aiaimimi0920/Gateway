//! Refresh commits use the normal revision/secret owner and optimistic writer CAS.
use super::transport::TokenResponse;
use crate::console::RouteConfigRuntime;
use crate::routing::config::{
    effective_credential_id, ProviderCredentialYaml, RouteConfigStore, RouteConfigYaml,
};

pub(super) struct PendingRefresh {
    pub provider: String,
    pub credential: String,
    pub original: ProviderCredentialYaml,
    pub token: TokenResponse,
    pub expires_at: String,
    pub previous_access: String,
    pub previous_refresh: String,
}

pub(super) fn credential<'a>(
    document: &'a RouteConfigYaml,
    provider: &str,
    id: &str,
) -> Option<&'a ProviderCredentialYaml> {
    let provider = document.providers.iter().find(|p| p.id == provider)?;
    provider
        .credentials
        .iter()
        .enumerate()
        .find(|(index, c)| effective_credential_id(&provider.id, *index, c) == id)
        .map(|(_, c)| c)
}

fn same_authorization(a: &ProviderCredentialYaml, b: &ProviderCredentialYaml) -> bool {
    a.api_key == b.api_key
        && a.auth_token == b.auth_token
        && a.refresh_token == b.refresh_token
        && a.refresh_endpoint == b.refresh_endpoint
        && a.refresh_client_id == b.refresh_client_id
        && a.expires_at == b.expires_at
        && a.headers == b.headers
}

impl PendingRefresh {
    pub async fn write_to(
        &self,
        local: Option<&RouteConfigRuntime>,
        redis: Option<&deadpool_redis::Pool>,
    ) -> bool {
        if let Some(runtime) = local {
            return self.persist(runtime).await;
        }
        if let Some(pool) = redis {
            if super::server_cache::persist(
                pool,
                &self.credential,
                &self.previous_access,
                &self.previous_refresh,
                &self.token,
                &self.expires_at,
            )
            .await
            .is_ok()
            {
                return true;
            }
            tracing::warn!(cred_id = %self.credential, "OAuth refresh cache writeback pending");
        }
        false
    }

    fn already_saved(&self, current: &ProviderCredentialYaml) -> bool {
        current.api_key.as_deref() == Some(self.token.access_token.as_str())
            && current.refresh_token.as_deref()
                == self
                    .token
                    .refresh_token
                    .as_deref()
                    .or(self.original.refresh_token.as_deref())
            && current.expires_at.as_deref() == Some(self.expires_at.as_str())
    }

    /// true means saved, already visible after an uncertain commit, or superseded.
    /// false retains rotated material in memory for a later disk-only retry.
    pub async fn persist(&self, runtime: &RouteConfigRuntime) -> bool {
        for _ in 0..3 {
            let snapshot = runtime.route_config().snapshot();
            let Some(current) = credential(snapshot.document(), &self.provider, &self.credential)
            else {
                return true;
            };
            if self.already_saved(current) || !same_authorization(current, &self.original) {
                return true;
            }
            let mut document = snapshot.document().clone();
            let provider = document
                .providers
                .iter_mut()
                .find(|p| p.id == self.provider)
                .unwrap();
            let index = provider
                .credentials
                .iter()
                .enumerate()
                .position(|(index, c)| {
                    effective_credential_id(&provider.id, index, c) == self.credential
                })
                .unwrap();
            let target = &mut provider.credentials[index];
            target.api_key = Some(self.token.access_token.clone());
            if let Some(rt) = &self.token.refresh_token {
                target.refresh_token = Some(rt.clone());
            }
            target.token_expires_in_secs = self.token.expires_in;
            target.expires_at = Some(self.expires_at.clone());
            if let Some(session) = &mut target.session_auth {
                session.expires_at = Some(self.expires_at.clone());
            }
            match runtime
                .commit_automation_document(
                    snapshot.revision().id(),
                    document,
                    Some("Refresh OAuth credential".into()),
                )
                .await
            {
                Ok(_) => return true,
                Err(error) if error.code() == "console_revision_conflict" => continue,
                Err(error) => {
                    tracing::warn!(provider = %self.provider, cred_id = %self.credential,
                        code = error.code(), "OAuth refresh persistence pending; retaining rotated token for retry");
                    // Publication may have succeeded before the journal's last write failed.
                    let latest = runtime.route_config().snapshot();
                    return credential(latest.document(), &self.provider, &self.credential)
                        .is_none_or(|c| {
                            self.already_saved(c) || !same_authorization(c, &self.original)
                        });
                }
            }
        }
        false
    }

    pub fn is_current(&self, store: &RouteConfigStore) -> bool {
        credential(
            store.snapshot().document(),
            &self.provider,
            &self.credential,
        )
        .is_some_and(|c| same_authorization(c, &self.original))
    }
}
