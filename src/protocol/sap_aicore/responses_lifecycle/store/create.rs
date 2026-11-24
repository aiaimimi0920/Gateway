//! Internal one-shot submission on an authenticated, fixed route.
use super::super::{
    create_plan,
    evidence::Evidence,
    observe,
    transport::{DispatchError, Prepared},
    UpstreamId,
};
use super::{
    authorization::{key, owner_tag, read_authorized, read_candidate},
    dispatch::{ensure_running, live_id, not_found, Observation},
    RoutePin, SendHooks, Store,
};
use crate::{
    auth::session::AuthenticatedSession, error::GatewayError,
    protocol::canonical::CanonicalRelayRequest, routing::candidate::RouteCandidate,
    state::AppState,
};
use std::sync::Arc;

/// Values come from an internally selected route, never directly from HTTP.
pub(super) struct Target<'a> {
    pub(super) pin: RoutePin,
    pub(super) provider: &'a str,
    pub(super) credential: &'a str,
    pub(super) upstream_model: &'a str,
    pub(super) account_group: Option<&'a str>,
}

pub(super) async fn submit(
    state: &Arc<AppState>,
    session: &AuthenticatedSession,
    target: &Target<'_>,
    request: &CanonicalRelayRequest,
    estimate: u64,
    hooks: SendHooks<'_>,
    publish: Option<super::stream::Publish<'_>>,
) -> Result<Observation, DispatchError> {
    let store = Store::from_state(state).ok_or_else(not_found)?;
    let secret = key(state).ok_or_else(not_found)?;
    let owner = owner_tag(&secret, session).ok_or_else(not_found)?;
    if request.requested_model.as_deref() != Some(target.pin.public_model.as_str()) {
        return Err(not_found().into());
    }
    ensure_running(state)?;
    (hooks.check)()?;
    let (candidate, model) = current_target(state, session, target).await?;
    if model != target.upstream_model {
        return Err(not_found().into());
    }
    let payload = &candidate.payload;
    let binding = target
        .pin
        .capture_binding(&secret, target.provider, target.credential, payload)
        .ok_or_else(not_found)?;
    let previous = request.previous_response_id.as_deref();
    let predecessor = async {
        if let Some(id) = previous {
            predecessor_id(&store, state, session, target, id, &binding)
                .await
                .map(Some)
        } else {
            Ok(None)
        }
    };
    let upstream_previous = predecessor.await?;
    let plan = create_plan(
        payload,
        request,
        &model,
        previous.zip(upstream_previous.as_ref()),
    )?;
    let prepared = Prepared::prepare(&state.upstream_client, payload, plan).await?;
    let recheck = || async {
        (hooks.check)()?;
        ensure_running(state)?;
        let (current, current_model) = current_target(state, session, target).await?;
        if current_model != model
            || target
                .pin
                .capture_binding(
                    &secret,
                    target.provider,
                    target.credential,
                    &current.payload,
                )
                .as_deref()
                != Some(binding.as_str())
        {
            return Err(not_found());
        }
        if let Some(id) = previous {
            let current_id = predecessor_id(&store, state, session, target, id, &binding).await?;
            if Some(&current_id) != upstream_previous.as_ref() {
                return Err(not_found());
            }
        }
        // Reapply request admission in case deployment capabilities changed.
        create_plan(
            &current.payload,
            request,
            &model,
            previous.zip(upstream_previous.as_ref()),
        )?;
        ensure_running(state)?;
        (hooks.check)()?;
        Ok(current)
    };
    recheck().await?;
    // No journal row survives a denied debit, and no second debit is used
    // for polling or settlement. A crash after this commit is indeterminate.
    let receipt = store
        .begin_reserved(
            &owner,
            &binding,
            target.provider,
            target.credential,
            &target.pin,
            estimate,
        )
        .await?;
    (hooks.record)(&receipt.id);
    let admitted = async {
        let current = recheck().await?;
        let circuit = crate::routing::admission::attempt(state, &current, &model);
        // Claim the exact half-open/capacity lease inside the same synchronous
        // send gate as the request budget and real-send audit identity.
        (hooks.begin)(Some(circuit.clone()))?;
        Ok::<_, GatewayError>(circuit)
    }
    .await;
    let circuit = match admitted {
        Ok(circuit) => circuit,
        Err(error) => {
            store
                .refund_unsent(&receipt)
                .await
                .map_err(|_| DispatchError::UnsentRefund(receipt.id.clone()))?;
            return Err(error.into());
        }
    };
    if let Some(publish) = publish {
        return super::stream::consume(
            prepared,
            &store,
            receipt,
            &target.pin,
            hooks,
            super::stream::Source::Create {
                circuit: &circuit,
                background: request.raw_body["background"] == true,
            },
            publish,
        )
        .await;
    }
    let received = prepared
        .execute_until(hooks.deadline)
        .await
        .map_err(|evidence| {
            evidence.finish(&circuit);
            (hooks.observe)(evidence);
            DispatchError::Recorded(receipt.id.clone())
        })?;
    let body = received.body;
    let receipt = async {
        let (upstream, outcome) = observe(&body, None).map_err(|error| {
            let evidence = Evidence::failed(error.clone(), Some(received.status));
            evidence.finish(&circuit);
            (hooks.observe)(evidence);
            error
        })?;
        // Only validated upstream evidence affects the trial. Local database or
        // public projection failures must not be attributed to the provider.
        let evidence = Evidence::terminal(&body, received.status);
        evidence.finish(&circuit);
        (hooks.observe)(evidence);
        let written = store
            .observe(&receipt, &upstream, outcome)
            .await?
            .ok_or_else(not_found)?;
        Ok::<_, GatewayError>(written)
    }
    .await
    .map_err(|_| DispatchError::Recorded(receipt.id.clone()))?;
    let receipt = store
        .settle_balance(&receipt)
        .await
        .map_err(|_| DispatchError::SettlementPending(receipt.id.clone()))?
        .unwrap_or(receipt);
    Ok(Observation { receipt, body })
}

async fn current_target(
    state: &Arc<AppState>,
    session: &AuthenticatedSession,
    target: &Target<'_>,
) -> Result<(RouteCandidate, String), GatewayError> {
    let (candidate, model) = read_candidate(
        state,
        session,
        &target.pin,
        target.provider,
        target.credential,
        target.account_group,
    )
    .await?
    .ok_or_else(not_found)?;
    // A continuation is also a new generation. Poll/cancel/delete deliberately
    // retain only live authorization and do not inherit inference cooldowns.
    crate::routing::call_flow::validate_prepared_candidate(&candidate)?;
    crate::pipeline::stage_rate_limit::eligibility::EligibilityCheck::new(
        state, &candidate, &model,
    )
    .check()
    .await?;
    Ok((candidate, model))
}

async fn predecessor_id(
    store: &Store<'_>,
    state: &AppState,
    session: &AuthenticatedSession,
    target: &Target<'_>,
    id: &str,
    binding: &str,
) -> Result<UpstreamId, GatewayError> {
    let (row, _) = read_authorized(state, session, id, target.account_group)
        .await?
        .ok_or_else(not_found)?;
    if row.binding_tag != binding
        || row.phase != "completed"
        || store.route_pin(&row).await?.as_ref() != Some(&target.pin)
    {
        return Err(not_found());
    }
    live_id(&row).ok_or_else(not_found)
}
