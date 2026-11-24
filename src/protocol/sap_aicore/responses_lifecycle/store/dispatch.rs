//! Internal fixed-route retrieval and mutations. No public ingress or alternate routing.
use super::super::{
    observe, operation_plan,
    transport::{DispatchError, Prepared},
    Operation, UpstreamId,
};
use super::{authorization::read_authorized, mutation::MutationKind, Receipt, SendHooks, Store};
use crate::{auth::session::AuthenticatedSession, error::GatewayError, state::AppState};
use serde_json::Value;

/// This body is internal upstream evidence. Ingress must not return it verbatim.
pub(super) struct Observation {
    pub(super) receipt: Receipt,
    pub(super) body: Value,
}

pub(super) async fn access_existing(
    state: &AppState,
    session: &AuthenticatedSession,
    public_id: &str,
    account_group: Option<&str>,
    operation: Operation,
    hooks: SendHooks<'_>,
) -> Result<Observation, DispatchError> {
    let store = Store::from_state(state).ok_or_else(not_found)?;
    let (initial, payload) = read_authorized(state, session, public_id, account_group)
        .await?
        .ok_or_else(not_found)?;
    (hooks.record)(&initial.id);
    let upstream = live_id(&initial).ok_or_else(|| {
        GatewayError::conflict(format!(
            "Stored SAP response {} has no confirmed upstream ID; operator reconciliation is required. Do not repeat POST.",
            initial.id,
        )).with_code("sap_aicore_response_reconciliation_required")
    })?;
    let plan = operation_plan(&payload, &upstream, operation)?;
    ensure_running(state)?;
    (hooks.check)()?;
    let prepared = Prepared::prepare(&state.upstream_client, &payload, plan).await?;
    // The OAuth await must not turn stale authorization into send permission.
    let (current, _) = read_authorized(state, session, public_id, account_group)
        .await?
        .ok_or_else(not_found)?;
    if live_id(&current).as_ref() != Some(&upstream) || initial.binding_tag != current.binding_tag {
        return Err(not_found().into());
    }
    ensure_running(state)?;
    let claim = match operation {
        Operation::Retrieve => None,
        Operation::Cancel | Operation::Delete => Some(
            store
                .claim_mutation(
                    &current,
                    if operation == Operation::Cancel {
                        MutationKind::Cancel
                    } else {
                        MutationKind::Delete
                    },
                )
                .await?
                .ok_or_else(|| {
                    GatewayError::conflict("Stored SAP Responses operation is not admissible.")
                        .with_code("sap_aicore_response_mutation_unavailable")
                })?,
        ),
    };
    // This is the last explicit pre-send check; cancellation/transport failure
    // once execution starts must leave the durable claim in place.
    let ready = async {
        // A mutation claim can wait for a database lock. Recheck the original
        // authority after acquiring it, before invoking the prepared request.
        if claim.is_some() {
            let (fresh, _) = read_authorized(state, session, public_id, account_group)
                .await?
                .ok_or_else(not_found)?;
            if live_id(&fresh).as_ref() != Some(&upstream)
                || fresh.binding_tag != current.binding_tag
            {
                return Err(not_found());
            }
        }
        ensure_running(state)?;
        (hooks.check)()?;
        (hooks.begin)(None)
    };
    if let Err(error) = ready.await {
        if let Some(claim) = claim {
            store.release_unsent(claim).await?;
        }
        return Err(error.into());
    }
    let received = prepared
        .execute_until(hooks.deadline)
        .await
        .map_err(|evidence| {
            (hooks.observe)(evidence);
            DispatchError::DeliveryUnknown
        })?;
    let body = received.body;
    // Validate before touching the receipt; later database failures are local.
    let validated = if operation == Operation::Delete {
        super::super::validate_deleted(&body, &upstream)
    } else {
        observe(&body, Some(&upstream)).map(|_| ())
    };
    if let Err(error) = validated {
        (hooks.observe)(super::super::evidence::Evidence::failed(
            error,
            Some(received.status),
        ));
        return Err(DispatchError::DeliveryUnknown);
    }
    (hooks.observe)(super::super::evidence::Evidence::Neutral(received.status));
    let receipt = async {
        if let Some(claim) = claim {
            store.confirm_mutation(claim, &body).await?;
        } else {
            observe_retrieve(&store, &current, &upstream, &body).await?;
        }
        let latest = store
            .read(public_id, &current.owner_tag)
            .await?
            .ok_or_else(not_found)?;
        if operation != Operation::Delete && live_id(&latest).as_ref() != Some(&upstream) {
            return Err(not_found());
        }
        Ok::<_, GatewayError>(latest)
    }
    .await
    .map_err(|_| DispatchError::DeliveryUnknown)?;
    let receipt = store
        .settle_balance(&receipt)
        .await
        .map_err(|_| DispatchError::SettlementPending(receipt.id.clone()))?
        .unwrap_or(receipt);
    Ok(Observation { receipt, body })
}

async fn observe_retrieve(
    store: &Store<'_>,
    current: &Receipt,
    upstream: &UpstreamId,
    body: &Value,
) -> Result<(), GatewayError> {
    let (_, observation) = observe(body, Some(upstream))?;
    if store
        .observe(current, upstream, observation.clone())
        .await?
        .is_none()
    {
        if let Some(latest) = store.read(&current.id, &current.owner_tag).await? {
            // Allow a concurrent terminal publication to receive late authoritative usage.
            if latest.binding_tag == current.binding_tag && latest.revision != current.revision {
                store.observe(&latest, upstream, observation).await?;
            }
        }
    }
    Ok(())
}

pub(super) fn live_id(receipt: &Receipt) -> Option<UpstreamId> {
    if receipt.phase == "deleted" {
        return None;
    }
    UpstreamId::parse(receipt.upstream_id.as_deref()?)
}

pub(super) fn not_found() -> GatewayError {
    GatewayError::not_found("Stored SAP response is not available.")
        .with_code("sap_aicore_response_unavailable")
}

pub(super) fn ensure_running(state: &AppState) -> Result<(), GatewayError> {
    if state.shutdown.is_requested() || state.lifecycle.is_draining() {
        return Err(GatewayError::service_unavailable("Gateway is draining.")
            .with_code("sap_aicore_response_draining"));
    }
    Ok(())
}

#[cfg(all(test, feature = "line-sap-aicore-chat"))]
mod tests;
