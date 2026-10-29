//! Re-admit only a known refused candidate. Never rerun routing/reservations or concatenate streams.
use super::*;
use crate::request_wait::RequestWaitMode;
use std::io::Write;
mod identity;
mod policy;

pub(super) struct WaitingCandidate {
    index: usize,
}

pub(super) fn remember(
    slot: &mut Option<WaitingCandidate>,
    index: usize,
    error: &GatewayError,
    sent: bool,
    budget: &super::super::request_budget::RequestBudget,
) {
    if crate::request_wait::recoverable(error) {
        if sent {
            budget.allow_capacity_readmission(error);
        }
        *slot = Some(WaitingCandidate { index });
    }
}

pub(super) async fn run(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<PipelineOutput, GatewayError> {
    let aliases = identity::snapshot(state).await?;
    let snapshot = state.route_config.snapshot();
    let policies: Vec<_> = ctx
        .candidates
        .iter()
        .map(|c| policy::for_candidate(ctx, &snapshot, &aliases, c))
        .collect::<Result<_, _>>()?;
    let long = !policies.is_empty()
        && policies
            .iter()
            .all(|p| p.mode == RequestWaitMode::ClientConnected);
    let timeout = policies
        .iter()
        .filter_map(|p| p.total_timeout_seconds.map(|s| s as i32))
        .chain(
            ctx.route_policy_config
                .as_ref()
                .and_then(|p| p.total_request_timeout_seconds),
        )
        .min();
    let enabled = policies.iter().any(|p| p.mode != RequestWaitMode::Disabled)
        && matches!(
            ctx.canonical_req.endpoint_kind,
            EndpointKind::ChatCompletions | EndpointKind::Responses | EndpointKind::Messages
        );
    let budget = ctx.request_budget.clone();
    budget.configure_wait(&ctx.canonical_req, timeout, enabled && long, enabled)?;
    ctx.provider_identities = aliases;
    crate::request_wait::transport::within(
        enabled,
        budget.run(Box::pin(dispatch(ctx, state, enabled))),
    )
    .await
}

// Only wrap cancellation-safe waits/sends, never quota or rotating-credential transactions.
pub(super) async fn cancellable<T>(
    mut cancel: Option<tokio::sync::watch::Receiver<bool>>,
    budget: super::super::request_budget::RequestBudget,
    state: &AppState,
    future: impl std::future::Future<Output = T>,
) -> Result<T, GatewayError> {
    tokio::select! {
        biased;
        _ = state.shutdown.wait() => Err(budget.cancel()),
        _ = async {
            match cancel.as_mut() {
                Some(cancel) => while !*cancel.borrow_and_update() {
                    if cancel.changed().await.is_err() { break; }
                },
                None => std::future::pending().await,
            }
        } => Err(budget.cancel()),
        result = future => Ok(result),
    }
}

pub(super) async fn send_cancellable(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    index: usize,
    attempt: PreparedAttempt,
) -> Result<PipelineOutput, AttemptError> {
    let cancel = ctx.caller_cancel.clone();
    let budget = ctx.request_budget.clone();
    // Keep cancellation's generic frame pointer-sized on Windows debug stacks.
    let send = Box::pin(async {
        if ctx.canonical_req.stream {
            Box::pin(streaming::send(ctx, state, candidate, index, attempt)).await
        } else {
            Box::pin(buffered::send(ctx, state, candidate, index, attempt)).await
        }
    });
    cancellable(cancel, budget, state, send)
        .await
        .unwrap_or_else(|error| Err(AttemptError::Stop(error)))
}

async fn dispatch(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    enabled: bool,
) -> Result<PipelineOutput, GatewayError> {
    refresh_before_first_send(ctx, state).await?;
    let original_request = ctx.canonical_req.clone();
    let mut queue = None;
    let mut turn = None;
    let mut round = 0u32;
    loop {
        if state.lifecycle.is_draining() || state.shutdown.is_requested() {
            return Err(GatewayError::service_unavailable("Gateway is draining")
                .with_code("gateway_draining"));
        }
        let mut waiting = None;
        let result = Box::pin(super::run_candidates(ctx, state, &mut waiting)).await;
        let error = match result {
            Ok(output) => return Ok(output),
            Err(error) if enabled && crate::request_wait::recoverable(&error) => error,
            Err(error) => return Err(error),
        };
        let Some(waiting) = waiting else {
            return Err(error);
        };
        let candidate = ctx.candidates[waiting.index].clone();
        let policy = {
            let aliases = identity::snapshot(state).await?;
            let policy =
                policy::for_candidate(ctx, &state.route_config.snapshot(), &aliases, &candidate)?;
            state
                .lifecycle
                .request_waiters
                .reconcile_identities(&aliases)?;
            policy
        };
        if policy.mode == RequestWaitMode::Disabled {
            return Err(error);
        }
        // The permit/transport from run_candidates is gone before sleeping. No new detached task.
        drop(turn.take());
        if queue.is_none() {
            let session = ctx.session.as_ref();
            let key = format!(
                "{}:{}",
                session.map(|s| s.project_id.as_str()).unwrap_or("default"),
                session
                    .and_then(|s| s.access_key_id.as_deref().or(s.api_key_id.as_deref()))
                    .unwrap_or("anonymous")
            );
            queue = Some(state.lifecycle.request_waiters.enter(
                key,
                candidate.provider_account_id.clone(),
                retained_bytes(ctx, state.local_runtime.is_some())?,
            )?);
        }
        let delay = crate::request_wait::delay(&error, round);
        tracing::debug!(req_id=%ctx.req_id, round, wait_ms=delay.as_millis(), "waiting for upstream recovery");
        cancellable(ctx.caller_cancel.clone(), ctx.request_budget.clone(), state, async { tokio::select! {
            _ = state.shutdown.wait() => return Err(GatewayError::service_unavailable("Gateway is shutting down")),
            _ = crate::request_wait::sleep_delay(delay) => Ok(())
        }}).await??;
        turn = Some(
            cancellable(
                ctx.caller_cancel.clone(),
                ctx.request_budget.clone(),
                state,
                queue.as_ref().unwrap().turn(),
            )
            .await?,
        );
        ctx.canonical_req = original_request.clone();
        ctx.tools_were_injected = false;
        let refreshed = refresh_candidate(ctx, state, &candidate).await?;
        let row = ctx.projected_access_candidates.get(waiting.index).cloned();
        ctx.projected_access_candidates = row.into_iter().collect();
        ctx.candidates = vec![refreshed];
        round = round.saturating_add(1);
    }
}

async fn refresh_before_first_send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<(), GatewayError> {
    let snapshot = state.route_config.snapshot();
    for index in 0..ctx.candidates.len() {
        let candidate = &ctx.candidates[index];
        if ctx
            .provider_identities
            .counterpart(&candidate.provider_account_id)
            .is_none()
        {
            continue;
        }
        let renamed = identity::bind(ctx, &snapshot, &ctx.provider_identities, candidate)?
            .is_some_and(|binding| binding.target != candidate.provider_account_id);
        if renamed {
            let candidate = candidate.clone();
            // Only refresh the known account: never rerun routing or reservations.
            let fresh = refresh_candidate(ctx, state, &candidate).await?;
            ctx.candidates[index] = fresh;
        }
    }
    Ok(())
}

async fn refresh_candidate(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
) -> Result<crate::routing::candidate::RouteCandidate, GatewayError> {
    let snapshot = state.route_config.snapshot();
    let mut constraint = snapshot
        .account_group_constraint(ctx.account_group_id.as_deref())
        .map_err(|_| crate::access_key_groups::denied())?;
    let groups =
        crate::access_key_groups::session_constraint(state, ctx.session.as_ref(), &snapshot)
            .await?;
    if let Some(groups) = &groups {
        constraint.intersect(groups);
    }
    if !constraint.allows_candidate(candidate) {
        return Err(crate::access_key_groups::denied());
    }
    let aliases = identity::snapshot(state).await?;
    let mut fresh = identity::refresh(
        ctx,
        &snapshot,
        &constraint,
        groups.is_some(),
        &aliases,
        candidate,
    )?;
    let mut local_fallback = None;
    if let (Some(local), Some(id)) = (
        &state.local_runtime,
        crate::local_runtime::access_keys::local_key_id(state, ctx.session.as_ref()),
    ) {
        let mut candidates = vec![fresh];
        local_fallback = local
            .authorize_local_candidates_with_fallback_authorization(
                id,
                ctx.canonical_req.requested_model.as_deref(),
                &mut candidates,
            )
            .await?;
        fresh = candidates
            .pop()
            .ok_or_else(crate::access_key_groups::denied)?;
    }
    // Publish only after candidate and key re-admission both succeed. Never union
    // a prior key grant into the refreshed authority or replace route grants.
    // This snapshot can be newer than the one checked before sleeping. Reject
    // conflicting queue owners before publishing it or resuming any upstream IO.
    state
        .lifecycle
        .request_waiters
        .reconcile_identities(&aliases)?;
    ctx.local_key_fallback_provider_ids = local_fallback.unwrap_or_default();
    ctx.provider_identities = aliases;
    Ok(fresh)
}

fn retained_bytes(ctx: &PipelineContext, local: bool) -> Result<usize, GatewayError> {
    struct Counter(usize);
    impl Write for Counter {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(b.len());
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, &ctx.canonical_req)
        .map_err(|_| GatewayError::server_error("Cannot measure waiting request"))?;
    // Account conservatively for canonical snapshots, native wire and dispatch copies, without allocating JSON.
    // Reserve the full bounded registry even when currently empty: a waiting
    // request can observe new mappings without re-entering or changing its FIFO owner.
    let identity_bytes = if local {
        crate::provider_identity::ProviderAliases::WAIT_SNAPSHOT_BYTES
    } else {
        0
    };
    Ok(counter
        .0
        .saturating_mul(8)
        .saturating_add(64 * 1024)
        .saturating_add(identity_bytes))
}

#[cfg(test)]
mod memory_tests;
