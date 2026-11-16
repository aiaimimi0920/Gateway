//! Caller-owned buffered SDK dispatch; compiled routing requires the SDK feature and explicit enable.
use super::{
    copilot_identity::CopilotIdentity, stage_rate_limit::ProviderAttemptGate, PipelineContext,
};
use crate::{
    error::{FallbackHint, GatewayError},
    protocol::canonical::CanonicalRelayResponse,
    routing::candidate::RouteCandidate,
    state::AppState,
};

pub(super) fn unavailable() -> GatewayError {
    abort(
        GatewayError::service_unavailable("Copilot text execution is unavailable")
            .with_code("copilot_execution_unavailable"),
    )
}

fn abort(mut error: GatewayError) -> GatewayError {
    error.retryable = false;
    error.native_transport = None;
    error.fallback_hint = FallbackHint::Abort {
        reason: "Copilot execution must not be replayed or substituted.".into(),
    };
    error
}

pub(super) async fn execute(
    state: &AppState,
    context: &PipelineContext,
    candidate: &RouteCandidate,
    model: &str,
    gate: &ProviderAttemptGate,
    observe: impl for<'a> FnMut(crate::retry::RetryAttemptObservation<'a>),
) -> Result<CanonicalRelayResponse, GatewayError> {
    let identity = CopilotIdentity::bind(state, context, candidate, model)?;
    identity.token_for_send().await?;
    #[cfg(feature = "copilot-sdk-runtime")]
    {
        let settings =
            crate::console::copilot_catalog::Settings::from_env().ok_or_else(unavailable)?;
        let policy = crate::retry::RetryPolicy {
            max_retries: 0,
            ..Default::default()
        };
        // Even a future retry-policy regression must not poll the owned generation twice.
        let mut operation = Some(Box::pin(runtime::run(
            &identity,
            gate,
            settings,
            &runtime::SLOT,
        )));
        gate.execute_observed(
            || {
                let operation = operation.take();
                async move {
                    match operation {
                        Some(operation) => operation.await,
                        None => Err(unavailable()),
                    }
                }
            },
            observe,
            &policy,
        )
        .await
        .map_err(abort)
    }
    #[cfg(not(feature = "copilot-sdk-runtime"))]
    {
        let _ = (gate, observe);
        Err(unavailable())
    }
}

#[cfg(feature = "copilot-sdk-runtime")]
mod runtime;
