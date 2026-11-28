//! Request-owned catalog relay: consent before concurrency, one purchase/run, never legacy retry.
use super::*;
use crate::{
    console::freebuff_purchase::{ApprovedRequest, RequestBinding},
    local_runtime::LocalRuntime,
    protocol::freebuff::{
        business_target::BusinessTarget,
        catalog::{BusinessError, CatalogSession, DeviceRegistration, RelayStep},
    },
    routing::candidate::RouteCandidate,
};
use std::time::Duration;
mod live;
mod paid;
mod prepare;
mod protocol;
struct Ready {
    local: LocalRuntime,
    session: CatalogSession,
    device: DeviceRegistration,
    step: RelayStep,
    approved: ApprovedRequest,
}
fn remaining(ctx: &PipelineContext) -> Result<Duration, GatewayError> {
    ctx.request_budget.check()?;
    Ok(ctx
        .request_budget
        .admission_deadline()
        .map(|deadline| deadline.saturating_duration_since(tokio::time::Instant::now()))
        .unwrap_or(Duration::from_secs(300))
        .min(Duration::from_secs(300)))
}
fn business(error: BusinessError) -> GatewayError {
    if error == BusinessError::RunJournalUnavailable {
        return GatewayError::service_unavailable("FreeBuff original run journal is unavailable or changed. The original claim was retained; no uncertain operation will be replayed.").with_code(error.to_string());
    }
    let mut error = GatewayError::server_error("FreeBuff catalog execution or its accounting is unconfirmed. Any existing local claim was retained; inspect it before another purchase.")
        .with_code(error.to_string()).with_provider("freebuff_compatible");
    error.http_status = Some(502);
    error
}
fn input_error() -> GatewayError {
    GatewayError::bad_request("FreeBuff catalog relay accepts supported Chat/Responses/Messages text/function JSON or SSE; unsupported fields are rejected before quoting.")
        .with_code("freebuff_catalog_relay_unsupported")
}
pub(super) async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
    candidate: &RouteCandidate,
    candidate_index: usize,
    model: &str,
    target: BusinessTarget,
    gate: stage_rate_limit::ProviderAttemptGate,
) -> Result<PipelineOutput, GatewayError> {
    let ready = recovery_wait::cancellable(
        ctx.caller_cancel.clone(),
        ctx.request_budget.clone(),
        state,
        Box::pin(prepare::prepare(ctx, state, candidate, model, target)),
    )
    .await??;
    let controller = state
        .concurrency_registry
        .get_or_create(&candidate.provider_account_id);
    let account_permit = recovery_wait::cancellable(
        ctx.caller_cancel.clone(),
        ctx.request_budget.clone(),
        state,
        state
            .lifecycle
            .account_concurrency
            .acquire(state, candidate),
    )
    .await??;
    let permit = recovery_wait::cancellable(
        ctx.caller_cancel.clone(),
        ctx.request_budget.clone(),
        state,
        controller.acquire(),
    )
    .await?;
    let permit = match account_permit {
        Some(account_permit) => permit.with_guard(account_permit),
        None => permit,
    };
    gate.admit().await?;
    let started = Instant::now();
    if ctx.canonical_req.stream {
        let live = Box::pin(live::prepare(
            ctx, state, candidate, model, &gate, ready, permit,
        ))
        .await;
        return match live {
            Ok((stream, usage, semantics, terminal)) => {
                let projected = ctx
                    .projected_access_candidates
                    .get(candidate_index)
                    .cloned();
                set_selected_candidate(ctx, candidate, projected.as_ref(), model, false);
                let tracked = streaming::terminal::track(
                    ctx,
                    state,
                    candidate,
                    streaming::terminal::Owner {
                        stream,
                        permit: None, // The cooperative paid future already retains its permit.
                        usage: Some(usage),
                        native_semantics: Some(semantics),
                        model: model.into(),
                        route_policy_config: ctx.route_policy_config.clone(),
                        controller,
                        gate,
                        started_at: started,
                        legacy_lease: None,
                        terminal: Some(terminal),
                    },
                );
                Ok(PipelineOutput::Sse(tracked))
            }
            Err(error) => {
                let result: Result<(), _> = Err(error.clone());
                observe_provider_result_metric(
                    global_gateway_metrics().as_ref(),
                    &candidate.provider_account_id,
                    model,
                    &result,
                    started.elapsed().as_millis() as u64,
                );
                failed(ctx, state, candidate, &controller, &gate, &error);
                Err(error)
            }
        };
    }
    // The paid owner observes cancellation itself and awaits known-cost FINISH. Actual caller Drop
    // still retains its journal, with no detached finalization, retry, end or purchase replacement.
    let result = Box::pin(paid::execute(ctx, state, candidate, model, &gate, ready)).await;
    observe_provider_result_metric(
        global_gateway_metrics().as_ref(),
        &candidate.provider_account_id,
        model,
        &result,
        started.elapsed().as_millis() as u64,
    );
    drop(permit);
    match result {
        Ok(completed) => {
            gate.finish_success();
            controller.on_success();
            spawn_record_provider_success(
                state.clone(),
                candidate.provider_account_id.clone(),
                candidate.provider_credential_id.clone(),
            );
            let projected = ctx
                .projected_access_candidates
                .get(candidate_index)
                .cloned();
            set_selected_candidate(ctx, candidate, projected.as_ref(), model, false);
            ctx.observed_usage = Some(completed.usage);
            ctx.canonical_completion_semantics = completed.semantics;
            Ok(PipelineOutput::Json(completed.body))
        }
        Err(error) => {
            failed(ctx, state, candidate, &controller, &gate, &error);
            Err(error)
        }
    }
}
fn failed(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    candidate: &RouteCandidate,
    controller: &crate::concurrency::aimd::AimdController,
    gate: &stage_rate_limit::ProviderAttemptGate,
    error: &GatewayError,
) {
    let journal_unavailable =
        error.code.as_deref() == Some("freebuff_business_run_journal_unavailable");
    if journal_unavailable {
        gate.finish_neutral();
    } else {
        gate.finish_error(&error);
    }
    if !journal_unavailable && should_record_provider_failure(&error) {
        controller.on_failure(classify_failure_kind(&error));
        spawn_record_provider_failure(
            ctx.started_at,
            state.clone(),
            candidate.provider_account_id.clone(),
            candidate.provider_credential_id.clone(),
            ctx.route_policy_config.clone(),
            error.message.clone(),
        );
    }
}
#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
mod tests;
