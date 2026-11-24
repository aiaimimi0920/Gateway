//! Refresh commits use the normal revision/secret owner and optimistic writer CAS.
use super::transport::TokenResponse;
use crate::console::refresh_grants::{GrantRecord, GrantRequest};
use crate::console::RouteConfigRuntime;
use crate::routing::config::{
    effective_credential_id, same_authorization, ProviderCredentialYaml, RouteConfigSnapshot,
    RouteConfigStore, RouteConfigYaml, TokenRefreshState,
};

pub(crate) struct PendingRefresh {
    pub(super) owner: std::sync::Arc<TokenRefreshState>,
    pub(super) generation: u64,
    // Retain single-flight across storage outages, including policy wrapper replacement.
    pub(super) attempt: crate::routing::config::RefreshAttempt,
    pub(super) applied: bool,
    pub(super) request: GrantRequest,
    pub(super) record: GrantRecord,
    pub(super) provider: String,
    pub(super) credential: String,
    pub(super) original: ProviderCredentialYaml,
    pub(super) token: TokenResponse,
    pub(super) expires_at: Option<String>,
    pub(super) previous_access: String,
    pub(super) previous_refresh: String,
    pub(super) previous_context: Option<crate::protocol::grok_oauth::AuthorizationContext>,
    pub(super) previous_sgcookie: Option<String>,
}

pub(super) fn authority_budget() -> std::time::Duration {
    let budget = std::time::Duration::from_secs(20);
    #[cfg(test)]
    let budget = super::tests::SERVER_AUTHORITY_BUDGET
        .try_with(|budget| *budget)
        .unwrap_or(budget);
    budget
}

pub(super) async fn prepare_worker_authority(runtime: &RouteConfigRuntime) -> bool {
    matches!(
        tokio::time::timeout(authority_budget(), async {
            let mut recovery = runtime.refresh_authority_recovery.lock().await;
            *recovery = true;
            match runtime.recover_refresh_authority().await {
                Ok(()) => {
                    *recovery = false;
                    true
                }
                Err(error) => {
                    tracing::warn!(
                        code = error.code(),
                        "Worker OAuth refresh authority is not ready"
                    );
                    false
                }
            }
        })
        .await,
        Ok(true)
    )
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

impl PendingRefresh {
    pub async fn write_to(
        &mut self,
        runtime: Option<&RouteConfigRuntime>,
        redis: Option<&deadpool_redis::Pool>,
    ) -> bool {
        let Some(runtime) = runtime else { return false };
        // One shared authority budget covers Result, configuration publication and Published.
        if !matches!(
            tokio::time::timeout(
                authority_budget(),
                self.save_authority(runtime, redis.is_some())
            )
            .await,
            Ok(true)
        ) {
            return false;
        }
        // Supersession can discard publication/cache, never the Result storage obligation above.
        if !self.is_current(runtime.route_config()) || redis.is_none() {
            return true;
        }
        if let Some(pool) = redis {
            let Some(expiry) = self.expires_at.as_deref() else {
                return false;
            };
            if super::server_cache::persist_with_quick(
                pool,
                &self.credential,
                &self.previous_access,
                &self.previous_refresh,
                self.previous_context.as_ref(),
                self.original.kiro_oauth.as_ref(),
                self.original.quick_oauth.as_ref(),
                self.original.vertex_oauth.as_ref(),
                self.original.dim_oauth.as_ref(),
                self.original.nous_portal.as_ref(),
                self.previous_sgcookie.as_deref(),
                &self.token,
                expiry,
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

    async fn save_authority(&mut self, runtime: &RouteConfigRuntime, server: bool) -> bool {
        let result = if let Some(context) = &self.token.lobsterai {
            self.request
                .result_with_lobster(&crate::protocol::lobsterai::SessionMaterial {
                    access_token: self.token.access_token.clone(),
                    refresh_token: self.token.refresh_token.clone().unwrap_or_default(),
                    context: context.clone(),
                })
        } else if let Some(context) = &self.token.nous_portal {
            self.request.result_with_nous(
                &self.token.access_token,
                self.token.refresh_token.as_deref().unwrap_or_default(),
                self.token.expires_in.unwrap_or(0),
                self.expires_at.as_deref().unwrap_or_default(),
                context.clone(),
            )
        } else {
            self.request.result_with_dim(
                &self.token.access_token,
                self.token.refresh_token.as_deref(),
                self.token.expires_in.unwrap_or(0),
                self.expires_at.as_deref().unwrap_or_default(),
                self.token.grok_oauth.clone(),
                self.token.accio_sgcookie.clone(),
                self.token.vertex_oauth.clone(),
                self.token.dim_oauth.clone(),
            )
        };
        let Ok(result) = result else {
            return false;
        };
        // Even a revoked ticket must record an already accepted successful response.
        match runtime
            .store_refresh_grant_result(&self.record, &result)
            .await
        {
            Ok(record) => self.record = record,
            Err(error) => {
                tracing::warn!(cred_id = %self.credential, code = error.code(),
                    "OAuth refresh result storage pending");
                return false;
            }
        }
        if !self.applied {
            let generation = if let Some(context) = &self.token.lobsterai {
                self.attempt
                    .apply_lobster(&crate::protocol::lobsterai::SessionMaterial {
                        access_token: self.token.access_token.clone(),
                        refresh_token: self.token.refresh_token.clone().unwrap_or_default(),
                        context: context.clone(),
                    })
            } else {
                self.attempt.apply_with_nous(
                    self.token.access_token.clone(),
                    self.token.refresh_token.clone(),
                    result.expires_in.expect("validated known lifetime"),
                    self.expires_at.clone().expect("validated known expiry"),
                    self.token.grok_oauth.clone(),
                    self.token.accio_sgcookie.clone(),
                    self.token.vertex_oauth.clone(),
                    self.token.dim_oauth.clone(),
                    self.token.nous_portal.clone(),
                )
            };
            let Some(generation) = generation else {
                return true;
            };
            self.generation = generation;
            self.applied = true;
        }
        if !self.is_current(runtime.route_config()) {
            return true;
        }
        let saved = if server {
            self.persist_server_authority(runtime).await
        } else {
            self.persist(runtime).await
        };
        if !saved {
            return false;
        }
        if !self.is_current(runtime.route_config()) {
            return true;
        }
        match runtime.mark_refresh_grant_published(&self.record).await {
            Ok(record) => {
                self.record = record;
                true
            }
            Err(error) => {
                tracing::warn!(cred_id = %self.credential, code = error.code(),
                    "OAuth refresh publication evidence pending");
                false
            }
        }
    }

    async fn persist_server_authority(&self, runtime: &RouteConfigRuntime) -> bool {
        let result = tokio::time::timeout(authority_budget(), async {
            // Include lock acquisition in the budget. Keep the obligation in the runtime,
            // not this ticket: superseding authorization can discard the entire ticket.
            let mut recovery = runtime.refresh_authority_recovery.lock().await;
            let recover = *recovery;
            *recovery = true;
            if recover {
                if let Err(error) = runtime.recover_refresh_authority().await {
                    tracing::warn!(cred_id = %self.credential, code = error.code(),
                        "OAuth refresh authority recovery pending");
                    return false;
                }
            }
            let saved = self.persist(runtime).await;
            if saved {
                *recovery = false;
            }
            saved
        })
        .await;
        match result {
            Ok(saved) => saved,
            Err(_) => {
                tracing::warn!(cred_id = %self.credential,
                    "OAuth refresh authority attempt timed out; retaining pending material");
                false
            }
        }
    }

    fn already_saved(&self, current: &ProviderCredentialYaml, document: &RouteConfigYaml) -> bool {
        let correct_base = self.token.nous_portal.as_ref().is_none_or(|context| {
            document
                .providers
                .iter()
                .find(|p| p.id == self.provider)
                .is_some_and(|p| {
                    current
                        .base_url
                        .as_deref()
                        .unwrap_or(&p.base_url)
                        .trim_end_matches('/')
                        == context.inference_target().base_url()
                })
        });
        correct_base
            && current.lobsterai.as_ref()
                == self
                    .token
                    .lobsterai
                    .as_ref()
                    .or(self.original.lobsterai.as_ref())
            && current.token_expires_in_secs == self.token.expires_in
            && current.kiro_oauth == self.original.kiro_oauth
            && current.quick_oauth == self.original.quick_oauth
            && current.nous_portal.as_ref()
                == self
                    .token
                    .nous_portal
                    .as_ref()
                    .or(self.original.nous_portal.as_ref())
            && current.dim_oauth.as_ref()
                == self
                    .token
                    .dim_oauth
                    .as_ref()
                    .or(self.original.dim_oauth.as_ref())
            && current.vertex_oauth.as_ref()
                == self
                    .token
                    .vertex_oauth
                    .as_ref()
                    .or(self.original.vertex_oauth.as_ref())
            && current.grok_oauth.as_ref()
                == self
                    .token
                    .grok_oauth
                    .as_ref()
                    .or(self.original.grok_oauth.as_ref())
            && current.api_key.as_deref() == Some(self.token.access_token.as_str())
            && current.accio_sgcookie.as_ref()
                == self
                    .token
                    .accio_sgcookie
                    .as_ref()
                    .or(self.original.accio_sgcookie.as_ref())
            && current.refresh_token.as_deref()
                == self
                    .token
                    .refresh_token
                    .as_deref()
                    .or(self.original.refresh_token.as_deref())
            && current.expires_at == self.expires_at
    }

    /// true means saved, already visible after an uncertain commit, or superseded.
    /// false retains rotated material in memory for a later disk-only retry.
    pub async fn persist(&self, runtime: &RouteConfigRuntime) -> bool {
        for _ in 0..3 {
            if !self.is_current(runtime.route_config()) {
                return true;
            }
            match runtime.commit_refresh_ticket(self).await {
                Ok(_) => return true,
                Err(error) if error.code() == "console_revision_conflict" => continue,
                Err(error) => {
                    tracing::warn!(provider = %self.provider, cred_id = %self.credential,
                        code = error.code(), "OAuth refresh persistence pending; retaining rotated token for retry");
                    // Publication may have succeeded before the journal's last write failed.
                    let latest = runtime.route_config().snapshot();
                    return credential(latest.document(), &self.provider, &self.credential)
                        .is_none_or(|c| {
                            self.already_saved(c, latest.document())
                                || !same_authorization(c, &self.original)
                        });
                }
            }
        }
        false
    }

    pub(crate) fn identity(&self) -> (&str, &str) {
        (&self.provider, &self.credential)
    }

    pub(crate) fn grant_locator(
        &self,
    ) -> Result<crate::console::refresh_grants::GrantLocator, crate::console::PersistenceError>
    {
        crate::console::refresh_grants::GrantLocator::from_record(&self.record)
    }

    pub(crate) fn project(
        &self,
        snapshot: &RouteConfigSnapshot,
    ) -> Result<Option<RouteConfigYaml>, ()> {
        if !self.applied || self.record.result().is_none() {
            return Err(()); // A received-but-unrecorded response is not an authority admission ticket.
        }
        let Some((_, active)) = snapshot.oauth_refresh_target(&self.provider, &self.credential)
        else {
            return Ok(None);
        };
        if !active
            .refresh_config
            .as_ref()
            .is_some_and(|config| config.authorization_is_current(&self.owner, self.generation))
        {
            return Ok(None);
        }
        let current = credential(snapshot.document(), &self.provider, &self.credential).ok_or(())?;
        if self.already_saved(current, snapshot.document())
            || !same_authorization(current, &self.original)
        {
            return Ok(None);
        }
        let mut material = current.clone();
        material.api_key = Some(self.token.access_token.clone());
        if let Some(refresh) = &self.token.refresh_token {
            material.refresh_token = Some(refresh.clone());
        }
        material.token_expires_in_secs = self.token.expires_in;
        material.expires_at = self.expires_at.clone();
        material.lobsterai = self.token.lobsterai.clone().or(material.lobsterai);
        if let Some(context) = &self.token.grok_oauth {
            material.grok_oauth = Some(context.clone());
        }
        if let Some(cookie) = &self.token.accio_sgcookie {
            material.accio_sgcookie = Some(cookie.clone());
        }
        if let Some(context) = &self.token.vertex_oauth {
            material.vertex_oauth = Some(context.clone());
        }
        if let Some(context) = &self.token.dim_oauth {
            material.dim_oauth = Some(context.clone());
        }
        if let Some(context) = &self.token.nous_portal {
            material.nous_portal = Some(context.clone());
        }
        super::projection::project(
            snapshot.document(),
            &self.provider,
            &self.credential,
            &material,
        )
        .map(Some)
        .ok_or(())
    }

    pub fn is_current(&self, store: &RouteConfigStore) -> bool {
        store
            .snapshot()
            .oauth_refresh_target(&self.provider, &self.credential)
            .and_then(|(_, credential)| credential.refresh_config.as_ref())
            .is_some_and(|current| current.authorization_is_current(&self.owner, self.generation))
    }
}
