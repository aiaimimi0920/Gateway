//! Stored SAP requests do not execute ordinary retries, quota, cache or finalization.
pub(crate) use super::lifecycle::run_stored_create as create;
use super::{
    stage_auth, stage_filter, stage_finalize, stage_rate_limit, stage_route, PipelineContext,
};
use crate::{
    auth::adapter::AuthRequest,
    error::GatewayError,
    protocol::sap_aicore::responses_lifecycle::{
        enabled,
        store::{lookup_pinned, run_existing, run_submit, PinnedTarget, SendHooks},
        Operation,
    },
    state::{AppState, StoredResponseAdmission, StoredResponseWaiter},
};
use serde_json::Value;
use std::sync::Arc;
mod health;

pub(crate) enum Request {
    Create,
    Existing { id: String, operation: Operation },
}

impl Request {
    fn label(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Existing { operation, .. } => match operation {
                Operation::Retrieve => "retrieve",
                Operation::Cancel => "cancel",
                Operation::Delete => "delete",
            },
        }
    }
}

pub(crate) async fn run(
    mut ctx: PipelineContext,
    auth: AuthRequest,
    request: Request,
    state: Arc<AppState>,
) -> Result<Value, GatewayError> {
    stage_auth::run_request(&mut ctx, &state, &auth).await?;
    if ctx
        .session
        .as_ref()
        .and_then(|s| s.access_key_id.as_ref())
        .is_none()
    {
        return Err(GatewayError::unauthorized(
            "Stored Responses requires an access key.",
        ));
    }
    let admission = state.lifecycle.stored_response_tasks.try_admit()?;
    run_authenticated(ctx, request, state, false, admission).await
}

pub(super) async fn run_authenticated(
    ctx: PipelineContext,
    request: Request,
    state: Arc<AppState>,
    caller_admitted: bool,
    admission: StoredResponseAdmission,
) -> Result<Value, GatewayError> {
    let owned = state.clone();
    let work = move |waiter| Box::pin(run_owned(ctx, request, owned, waiter, caller_admitted));
    #[cfg(test)]
    let work = {
        // The mock's destination is task-local and must follow the durable task.
        let (tx, rx) = tokio::sync::oneshot::channel();
        let task = crate::protocol::sap_aicore::test_support::inherit(async move {
            work(
                rx.await
                    .map_err(|_| GatewayError::server_error("missing stored waiter"))?,
            )
            .await
        });
        move |waiter| {
            let _ = tx.send(waiter);
            task
        }
    };
    admission.run(work).await
}

async fn run_owned(
    mut ctx: PipelineContext,
    request: Request,
    state: Arc<AppState>,
    waiter: StoredResponseWaiter,
    caller_admitted: bool,
) -> Result<Value, GatewayError> {
    let receipt_id = parking_lot::Mutex::new(None);
    let observed = parking_lot::Mutex::new(None);
    ctx.observed_upstream_status = Some(None);
    let result = dispatch(
        &mut ctx,
        &request,
        &state,
        &waiter,
        &receipt_id,
        &observed,
        caller_admitted,
    )
    .await;
    // This owner survives caller disconnect. Health never gates required receipt persistence.
    let observed = observed.into_inner();
    if let Some(observed) = &observed {
        ctx.observed_upstream_status = Some(observed.evidence.status());
    }
    if tokio::time::timeout(
        std::time::Duration::from_secs(5),
        health::persist(&state, ctx.started_at, observed),
    )
    .await
    .is_err()
    {
        tracing::warn!("SAP response health persistence timed out");
    }
    let disconnected = waiter.check().is_err();
    if disconnected {
        ctx.request_budget.cancel_caller();
    }
    let receipt_id = receipt_id.into_inner();
    stage_finalize::finish_stored_response_audit(
        &ctx,
        &result,
        request.label(),
        disconnected,
        receipt_id.as_deref(),
        &state,
    )
    .await;
    result
}

async fn dispatch(
    ctx: &mut PipelineContext,
    request: &Request,
    state: &Arc<AppState>,
    waiter: &StoredResponseWaiter,
    receipt_id: &parking_lot::Mutex<Option<String>>,
    observed: &parking_lot::Mutex<Option<health::Observed>>,
    caller_admitted: bool,
) -> Result<Value, GatewayError> {
    waiter.check()?;
    let session = ctx.session.clone().ok_or_else(unavailable)?;
    stage_finalize::begin_request_audit(ctx, state).await;
    if !caller_admitted {
        stage_rate_limit::run_stored(ctx, state, false).await?;
    }
    ctx.request_budget.configure(
        &ctx.canonical_req,
        ctx.route_policy_config
            .as_ref()
            .and_then(|p| p.total_request_timeout_seconds),
    )?;
    ctx.request_budget.check()?;
    let predecessor = match request {
        Request::Create => ctx.canonical_req.previous_response_id.as_deref(),
        Request::Existing { id, .. } => Some(id.as_str()),
    };
    let pinned = match predecessor {
        Some(id) => {
            Some(lookup_pinned(state, &session, id, ctx.account_group_id.as_deref()).await?)
        }
        None => None,
    };
    match request {
        Request::Existing { .. } => {
            ctx.canonical_req.requested_model = pinned.as_ref().map(|pin| pin.public_model.clone());
        }
        Request::Create => {
            if let Some(target) = &pinned {
                if ctx.canonical_req.requested_model.as_deref() != Some(&target.public_model) {
                    return Err(unavailable());
                }
            }
        }
    }
    if matches!(request, Request::Create) {
        stage_filter::enforce_content(ctx, state)?;
    }
    if !caller_admitted {
        stage_rate_limit::run_stored(ctx, state, true).await?;
    }
    let selected = match pinned {
        Some(target) => target,
        None => {
            if ctx.candidates.is_empty() {
                Box::pin(stage_route::select_for_durable_reservation(ctx, state)).await?;
            }
            select(ctx, state)?
        }
    };
    if !ctx.provider_allowed_for_send(&selected.provider) {
        let mut error =
            GatewayError::unauthorized("Stored response route is denied by current policy.")
                .with_code("sap_aicore_response_route_denied");
        error.http_status = Some(403);
        return Err(error);
    }
    ctx.source_access_key_id = Some(selected.source_key.clone());
    ctx.selected_platform_access_id = selected.platform_access.clone();
    let gate = stage_rate_limit::provider_attempt_gate(ctx, state, &selected.provider)?
        .with_pinned(
            &selected.provider,
            &selected.credential,
            &selected.upstream_model,
        );
    waiter.check()?;
    gate.admit().await?;
    let hooks = SendHooks {
        check: &|| {
            waiter.check()?;
            gate.check_remaining()
        },
        begin: &|circuit| {
            state
                .lifecycle
                .stored_response_tasks
                .begin_send(waiter, || {
                    if state.lifecycle.is_draining() || state.shutdown.is_requested() {
                        return Err(GatewayError::service_unavailable("Gateway is draining.")
                            .with_code("sap_aicore_response_not_sent"));
                    }
                    match circuit {
                        Some(circuit) => gate.clone().with_circuit(circuit).begin_attempt(),
                        None => gate.begin_attempt(),
                    }
                })
        },
        record: &|id| *receipt_id.lock() = Some(id.to_owned()),
        observe: &|evidence| {
            *observed.lock() = Some(health::Observed {
                target: selected.clone(),
                evidence,
            });
        },
        deadline: ctx.request_budget.admission_deadline(),
    };
    let group = ctx.account_group_id.as_deref();
    match request {
        Request::Create => {
            let estimate = crate::redis::usage_tracking::estimate_token_count(
                &ctx.canonical_req.messages_text(),
            )
            .max(1);
            let estimate = if session.access_key_kind.as_deref() == Some("auto_route") {
                estimate
            } else {
                (estimate as f64 * state.config.quota_pre_deduct_estimate_ratio).ceil() as u64
            };
            run_submit(
                state,
                &session,
                &selected,
                group,
                &ctx.canonical_req,
                estimate,
                hooks,
            )
            .await
        }
        Request::Existing { id, operation } => {
            run_existing(state, &session, &selected, id, group, *operation, hooks).await
        }
    }
}

fn select(ctx: &PipelineContext, state: &AppState) -> Result<PinnedTarget, GatewayError> {
    let model = ctx
        .canonical_req
        .requested_model
        .as_deref()
        .ok_or_else(unavailable)?;
    let caller = ctx
        .requesting_access_key_id
        .as_deref()
        .ok_or_else(unavailable)?;
    let hosted = state.pg_pool.is_some();
    if hosted && ctx.candidates.len() != ctx.projected_access_candidates.len() {
        return Err(GatewayError::server_error(
            "Stored Responses route projection misaligned.",
        ));
    }
    let (index, candidate) = ctx
        .candidates
        .iter()
        .enumerate()
        .find(|(_, candidate)| {
            enabled(&candidate.payload)
                && ctx.provider_allowed_for_send(&candidate.provider_account_id)
        })
        .ok_or_else(unavailable)?;
    let (credential, source, access) = if hosted {
        let row = &ctx.projected_access_candidates[index];
        if row.provider_account_id != candidate.provider_account_id
            || row.requesting_access_key_id != caller
            || row.model_code != model
            || row.upstream_model.as_deref().unwrap_or(model)
                != candidate.upstream_model.as_deref().unwrap_or(model)
        {
            return Err(GatewayError::server_error(
                "Stored Responses route identity misaligned.",
            ));
        }
        (
            candidate.provider_credential_id.as_deref(),
            row.source_access_key_id.as_str(),
            Some(row.platform_access_id.clone()),
        )
    } else {
        if state.local_runtime.is_none() || candidate.provider_credential_id.is_some() {
            return Err(unavailable());
        }
        (candidate.payload.credential_id.as_deref(), caller, None)
    };
    Ok(PinnedTarget {
        public_model: model.to_owned(),
        provider: candidate.provider_account_id.clone(),
        credential: credential.ok_or_else(unavailable)?.to_owned(),
        source_key: source.to_owned(),
        platform_access: access,
        upstream_model: candidate
            .upstream_model
            .as_deref()
            .unwrap_or(model)
            .to_owned(),
    })
}

fn unavailable() -> GatewayError {
    GatewayError::not_found("Stored SAP response is not available.")
        .with_code("sap_aicore_response_unavailable")
}
