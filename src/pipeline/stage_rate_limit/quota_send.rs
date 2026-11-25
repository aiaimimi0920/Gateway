//! Only quota-guarded inference defers its attempt to the final HTTP send.
//! A console probe has no pipeline scope; it still runs the transport's quota checks.
use super::ProviderAttemptGate;
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub(crate) fn guarded(payload: &ProviderAccountPayload) -> bool {
    crate::protocol::cloudflare_workers_ai::owns_payload(payload)
        || crate::protocol::vercel_free::owns_payload(payload)
}

pub(super) struct Scope {
    gate: ProviderAttemptGate,
    pub(super) started: AtomicBool,
}

impl Scope {
    pub(super) fn for_gate(gate: &ProviderAttemptGate) -> Option<Arc<Self>> {
        gate.defer_until_quota.then(|| {
            Arc::new(Self {
                gate: gate.clone(),
                started: AtomicBool::new(false),
            })
        })
    }
}

tokio::task_local! {
    static SEND: Arc<Scope>;
}

pub(super) async fn run<T>(
    scope: Option<&Arc<Scope>>,
    future: impl std::future::Future<Output = Result<T, GatewayError>>,
) -> Result<T, GatewayError> {
    let Some(scope) = scope else {
        return future.await;
    };
    let result = SEND.scope(Arc::clone(scope), future).await;
    if result.is_ok() && !scope.started.load(Ordering::Relaxed) {
        return Err(GatewayError::server_error(
            "Guarded inference completed without a send",
        ));
    }
    result
}

pub(crate) async fn after_quota() -> Result<(), GatewayError> {
    let Ok(scope) = SEND.try_with(Arc::clone) else {
        return Ok(());
    };
    // Analytics and balance probes can await. Never override revocation or a spent deadline.
    scope.gate.recheck_send_eligibility().await?;
    if scope.started.load(Ordering::Relaxed) {
        return Err(GatewayError::server_error(
            "Guarded inference attempted a duplicate send",
        ));
    }
    scope.gate.begin_attempt()?;
    scope.started.store(true, Ordering::Relaxed);
    Ok(())
}

#[cfg(all(test, feature = "family-openai-compatible-official-api"))]
#[path = "quota_send_tests.rs"]
mod tests;
