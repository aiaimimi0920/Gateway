//! Buffered, streaming, cancellation and restart all refer to the same durable reservation.
use super::CashOutcome;
use crate::{
    access_balance::AccessBalanceStore,
    pipeline::{stage_finalize::RequestAuditFinalizeSnapshot, PipelineContext},
    protocol::canonical::TokenUsage,
    state::AppState,
};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc, Weak,
};

pub struct CashGuard {
    id: String,
    state: Weak<AppState>,
    attempts: Arc<AtomicU32>,
}

impl std::fmt::Debug for CashGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CashGuard")
            .field("id", &self.id)
            .finish()
    }
}

impl CashGuard {
    pub(crate) fn new(id: String, state: &Arc<AppState>, attempts: Arc<AtomicU32>) -> Self {
        Self {
            id,
            state: Arc::downgrade(state),
            attempts,
        }
    }
}

impl Drop for CashGuard {
    fn drop(&mut self) {
        let (Some(state), Ok(runtime)) =
            (self.state.upgrade(), tokio::runtime::Handle::try_current())
        else {
            return;
        };
        let id = self.id.clone();
        let outcome = if self.attempts.load(Ordering::Relaxed) == 0 {
            CashOutcome::NotSent
        } else {
            CashOutcome::Unknown
        };
        let guard = state
            .local_runtime
            .as_ref()
            .map(|local| local.track_finalizer());
        runtime.spawn(async move {
            let _guard = guard;
            finish(&id, outcome, &state).await;
        });
    }
}

async fn finish(id: &str, outcome: CashOutcome, state: &AppState) {
    let result = match AccessBalanceStore::from_state(state) {
        Ok(store) => store.finish_cash(id, outcome).await,
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        tracing::error!(request_id = id, code = ?error.code, "Cash settlement failed; reservation retained");
    }
}

fn credential_id(provider: &str, credential: Option<&str>) -> String {
    credential
        .map(str::to_owned)
        .unwrap_or_else(|| crate::routing::config::provider_default_account_id(provider))
}

pub async fn success(ctx: &PipelineContext, usage: Option<TokenUsage>, state: &AppState) {
    let Some(charge) = &ctx.cash_charge else {
        return;
    };
    let provider = ctx.selected_provider_id.clone().unwrap_or_default();
    let credential = credential_id(
        &provider,
        ctx.selected_provider_credential_id
            .as_deref()
            .or(ctx.selected_real_credential_ref.as_deref()),
    );
    finish(
        &charge.id,
        CashOutcome::Success {
            provider,
            credential,
            model: ctx.resolved_model.clone().unwrap_or_default(),
            usage,
        },
        state,
    )
    .await;
}

pub async fn stream_success(
    snapshot: &RequestAuditFinalizeSnapshot,
    usage: Option<TokenUsage>,
    state: &AppState,
) {
    let Some(charge) = &snapshot.cash_charge else {
        return;
    };
    let provider = snapshot.provider_account_id.clone().unwrap_or_default();
    let credential = credential_id(&provider, snapshot.cash_credential_id.as_deref());
    finish(
        &charge.id,
        CashOutcome::Success {
            provider,
            credential,
            model: snapshot.resolved_model.clone().unwrap_or_default(),
            usage,
        },
        state,
    )
    .await;
}

pub async fn failure(charge: Option<&Arc<CashGuard>>, attempts: u32, state: &AppState) {
    if let Some(charge) = charge {
        finish(
            &charge.id,
            if attempts == 0 {
                CashOutcome::NotSent
            } else {
                CashOutcome::Unknown
            },
            state,
        )
        .await;
    }
}
