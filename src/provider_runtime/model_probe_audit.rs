//! Cancellation-safe audit ownership; detached storage never detaches generation or health changes.
use crate::{
    db,
    error::GatewayError,
    local_runtime::{FinalizerGuard, LocalRuntime},
    state::AppState,
};
use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const AUDIT_SLOTS: usize = 32;
static ADMISSION: OnceLock<Arc<Semaphore>> = OnceLock::new();
#[derive(Clone)]
enum Store {
    Local(LocalRuntime),
    Server(sqlx::PgPool),
}
impl Store {
    fn from_state(state: &AppState) -> Option<Self> {
        state
            .local_runtime
            .clone()
            .map(Self::Local)
            .or_else(|| state.pg_pool.clone().map(Self::Server))
    }
    async fn create(&self, input: db::CreateRequestAuditInput) -> Result<String, GatewayError> {
        match self {
            Self::Local(local) => local.create_audit(input).await,
            Self::Server(pool) => db::create_request_audit(pool, input).await,
        }
    }
    async fn finalize(
        &self,
        id: &str,
        input: db::FinalizeRequestAuditInput,
    ) -> Result<(), GatewayError> {
        match self {
            Self::Local(local) => local.finalize_audit(id, input).await,
            Self::Server(pool) => db::finalize_request_audit(pool, id, input).await,
        }
    }
}
struct Pending {
    store: Store,
    id: String,
    provider: Option<String>,
    model: Option<String>,
    route_trace: Option<serde_json::Value>,
    started: Instant,
}
struct Lifetime {
    _permit: OwnedSemaphorePermit,
    _drain: Option<FinalizerGuard>,
}
pub(super) struct ProbeAudit {
    pending: Option<Pending>,
    lifetime: Option<Lifetime>,
}
impl ProbeAudit {
    pub(super) async fn begin(
        state: &AppState,
        input: db::CreateRequestAuditInput,
    ) -> Option<Self> {
        Self::begin_with_limit(
            state,
            input,
            ADMISSION
                .get_or_init(|| Arc::new(Semaphore::new(AUDIT_SLOTS)))
                .clone(),
        )
        .await
    }
    async fn begin_with_limit(
        state: &AppState,
        input: db::CreateRequestAuditInput,
        limit: Arc<Semaphore>,
    ) -> Option<Self> {
        let store = Store::from_state(state)?;
        // Acquire before spawning. An abandoned caller cannot queue an unbounded detached storage task.
        let permit = limit.acquire_owned().await.ok()?;
        let drain = match &store {
            Store::Local(local) => Some(local.track_finalizer()),
            Store::Server(_) => None,
        };
        let lifetime = Lifetime {
            _permit: permit,
            _drain: drain,
        };
        let started = Instant::now();
        #[cfg(test)]
        let hooks = test_support::capture();
        // Dropping this JoinHandle does not abort creation: its owned output is dropped by Tokio,
        // invoking ProbeAudit::drop even when SQL committed before the caller received its audit ID.
        let task = tokio::spawn(async move {
            #[cfg(test)]
            hooks.before_create().await;
            let id = match store.create(input.clone()).await {
                Ok(id) => id,
                Err(error) => {
                    tracing::warn!(code = ?error.code,"cannot persist model probe audit");
                    return None;
                }
            };
            let audit = Self {
                pending: Some(Pending {
                    store,
                    id,
                    provider: input.provider_account_id,
                    model: input.resolved_model,
                    route_trace: input.route_trace,
                    started,
                }),
                lifetime: Some(lifetime),
            };
            #[cfg(test)]
            hooks.after_create().await;
            #[cfg(test)]
            hooks.returning();
            Some(audit)
        });
        match task.await {
            Ok(audit) => audit,
            Err(_) => {
                tracing::warn!("model probe audit creation task failed");
                None
            }
        }
    }
    pub(super) async fn finish(
        mut self,
        input: db::FinalizeRequestAuditInput,
    ) -> Result<(), GatewayError> {
        #[cfg(test)]
        let hooks = test_support::capture();
        // This ownership transfer is the terminal commit point. Cancelling its waiter does not
        // replace an already calculated result with cancellation or interrupt the SQL write.
        tokio::spawn(async move {
            #[cfg(test)]
            hooks.before_finish().await;
            let Some(pending) = self.pending.take() else {
                return Ok(());
            };
            let result = pending.store.finalize(&pending.id, input).await;
            if let Err(error) = &result {
                tracing::warn!(code = ?error.code,"cannot finalize model probe audit");
            }
            drop(self);
            result
        })
        .await
        .unwrap_or_else(|_| {
            Err(GatewayError::server_error(
                "Model probe audit finalizer task failed.",
            ))
        })
    }
}
impl Drop for ProbeAudit {
    fn drop(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let lifetime = self.lifetime.take();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!("model probe audit cancellation needs an active runtime");
            return;
        };
        runtime.spawn(async move {
            // Continuous drain/admission ownership spans create -> recording -> terminal storage.
            let _lifetime = lifetime;
            let input = pending.cancelled();
            if let Err(error) = pending.store.finalize(&pending.id, input).await {
                tracing::warn!(code = ?error.code,"cannot cancel model probe audit");
            }
        });
    }
}
impl Pending {
    fn cancelled(&self) -> db::FinalizeRequestAuditInput {
        db::FinalizeRequestAuditInput {
            status: "cancelled".into(),
            client_disconnected: false,
            upstream_status: None,
            duration_ms: self.started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            client_has_cache_control: false,
            auto_cache_applied: false,
            error_summary: Some("Model probe cancelled before terminal result submission.".into()),
            access_key_id: None,
            source_access_key_id: None,
            session_id: None,
            route_policy_id: None,
            provider_account_id: self.provider.clone(),
            resolved_model: self.model.clone(),
            model_alias: None,
            route_attempt_count: 1,
            response_id: None,
            route_trace: self.route_trace.clone(),
        }
    }
}

#[cfg(test)]
#[path = "model_probe_audit_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "model_probe_audit_tests.rs"]
mod tests;
