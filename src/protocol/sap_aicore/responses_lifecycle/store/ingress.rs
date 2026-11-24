//! Typed bridge from authenticated pipeline dispatch to the durable lifecycle.
use super::{
    authorization::read_authorized,
    create::{submit, Target},
    dispatch::{access_existing, not_found},
    RoutePin, Store,
};
use crate::routing::model_circuit::CircuitAttempt;
use crate::{
    auth::session::AuthenticatedSession, error::GatewayError,
    protocol::canonical::CanonicalRelayRequest, state::AppState,
};
use serde_json::Value;
use std::sync::Arc;

use super::super::{failure, transport::DispatchError, Endpoint, Operation};

pub(crate) struct SendHooks<'a> {
    pub check: &'a (dyn Fn() -> Result<(), GatewayError> + Sync),
    pub begin: &'a (dyn Fn(Option<Arc<CircuitAttempt>>) -> Result<(), GatewayError> + Sync),
    pub record: &'a (dyn Fn(&str) + Sync),
    pub observe: &'a (dyn Fn(super::super::evidence::Evidence) + Sync),
    pub deadline: Option<tokio::time::Instant>,
}

#[cfg(test)]
impl Default for SendHooks<'_> {
    fn default() -> Self {
        Self {
            check: &|| Ok(()),
            begin: &|circuit| circuit.map_or(Ok(()), |attempt| attempt.begin()),
            record: &|_| {},
            observe: &|_| {},
            deadline: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PinnedTarget {
    pub public_model: String,
    pub provider: String,
    pub credential: String,
    pub source_key: String,
    pub platform_access: Option<String>,
    pub upstream_model: String,
}

impl PinnedTarget {
    fn pin(&self) -> RoutePin {
        RoutePin {
            public_model: self.public_model.clone(),
            source_key_ref: self.source_key.clone(),
            platform_access_ref: self.platform_access.clone(),
        }
    }
}

pub(crate) async fn lookup_pinned(
    state: &AppState,
    session: &AuthenticatedSession,
    id: &str,
    group: Option<&str>,
) -> Result<PinnedTarget, GatewayError> {
    let store = Store::from_state(state).ok_or_else(not_found)?;
    let (receipt, payload) = read_authorized(state, session, id, group)
        .await?
        .ok_or_else(not_found)?;
    if receipt.phase == "deleted" {
        return Err(not_found());
    }
    let pin = store.route_pin(&receipt).await?.ok_or_else(not_found)?;
    let endpoint = Endpoint::parse(&payload.base_url).ok_or_else(not_found)?;
    Ok(PinnedTarget {
        public_model: pin.public_model,
        provider: receipt.provider_ref,
        credential: receipt.credential_ref,
        source_key: pin.source_key_ref,
        platform_access: pin.platform_access_ref,
        upstream_model: endpoint.deployment,
    })
}

#[cfg(test)]
pub(crate) async fn run_submit(
    state: &std::sync::Arc<AppState>,
    session: &AuthenticatedSession,
    selected: &PinnedTarget,
    group: Option<&str>,
    request: &CanonicalRelayRequest,
    estimate: u64,
    hooks: SendHooks<'_>,
) -> Result<Value, GatewayError> {
    run_submit_with_stream(
        state, session, selected, group, request, estimate, hooks, None,
    )
    .await
}

pub(crate) async fn run_submit_with_stream(
    state: &std::sync::Arc<AppState>,
    session: &AuthenticatedSession,
    selected: &PinnedTarget,
    group: Option<&str>,
    request: &CanonicalRelayRequest,
    estimate: u64,
    hooks: SendHooks<'_>,
    publish: Option<&(dyn Fn(bytes::Bytes, bool) + Sync)>,
) -> Result<Value, GatewayError> {
    if request.stream != publish.is_some() {
        return Err(super::super::invalid());
    }
    let target = Target {
        pin: selected.pin(),
        provider: &selected.provider,
        credential: &selected.credential,
        upstream_model: &selected.upstream_model,
        account_group: group,
    };
    let observation = submit(state, session, &target, request, estimate, hooks, publish)
        .await
        .map_err(public_error)?;
    observation
        .public_json(&target.pin)
        .map_err(|error| recoverable(error, &observation.receipt.id))
}

pub(crate) async fn run_existing_with_stream(
    state: &AppState,
    session: &AuthenticatedSession,
    selected: &PinnedTarget,
    id: &str,
    group: Option<&str>,
    operation: Operation,
    hooks: SendHooks<'_>,
    publish: Option<&(dyn Fn(bytes::Bytes, bool) + Sync)>,
) -> Result<Value, GatewayError> {
    let pin = selected.pin();
    access_existing(
        state,
        session,
        id,
        group,
        operation,
        hooks,
        publish.map(|publish| (&pin, publish)),
    )
    .await
    .map_err(public_error)?
    .public_json(&pin)
}

fn public_error(error: DispatchError) -> GatewayError {
    match error {
        DispatchError::NotSent(error) => error,
        DispatchError::DeliveryUnknown => failure("sap_aicore_response_delivery_unknown"),
        DispatchError::Recorded(id) => {
            recoverable(failure("sap_aicore_response_delivery_unknown"), &id)
        }
        DispatchError::SettlementPending(id) => {
            recoverable(failure("sap_aicore_response_settlement_pending"), &id)
        }
        DispatchError::UnsentRefund(id) => {
            let mut error = failure("sap_aicore_response_refund_pending");
            error.message = format!(
                "Stored SAP response {id} was not sent; its refund requires reconciliation. Do not repeat POST; contact the operator."
            );
            error.retryable = false;
            error.fallback_hint = crate::error::FallbackHint::Abort {
                reason: error.message.clone(),
            };
            error
        }
    }
}

fn recoverable(mut error: GatewayError, id: &str) -> GatewayError {
    error.message = format!(
        "Stored SAP response {id} requires reconciliation. Do not repeat POST; retrieve /v1/responses/{id} or contact the operator."
    );
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: error.message.clone(),
    };
    error
}
