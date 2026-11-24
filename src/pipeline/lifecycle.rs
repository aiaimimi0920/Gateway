//! Admission and terminal storage retain ownership on caller Drop; generation stays in the caller task.
use super::{
    cpa_scope, stage_auth, stage_filter, stage_finalize, stage_rate_limit, stage_route,
    PipelineContext, PipelineOutput,
};
use crate::{error::GatewayError, local_runtime::FinalizerGuard, state::AppState};
use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const LIFECYCLE_SLOTS: usize = 512;
static ADMISSION: OnceLock<Arc<Semaphore>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Audit,
    Filter,
    Route,
    RateLimit,
    Cash,
    RouteUnreserved,
    AutoReserve,
}
mod stored_create;
pub(crate) use stored_create::run as run_stored_create;
struct Lifetime {
    _permit: OwnedSemaphorePermit,
    _drain: Option<FinalizerGuard>,
}
struct Owner {
    ctx: Option<Box<PipelineContext>>,
    state: Arc<AppState>,
    lifetime: Option<Lifetime>,
    #[cfg(test)]
    hooks: test_support::Hooks,
}

pub(super) async fn run(
    mut ctx: PipelineContext,
    state: &Arc<AppState>,
) -> Result<PipelineOutput, GatewayError> {
    if let Err(error) = stage_auth::run(&mut ctx, state).await {
        stage_finalize::run_failure(&error, &ctx, state).await;
        return Err(error);
    }
    let limit = ADMISSION
        .get_or_init(|| Arc::new(Semaphore::new(LIFECYCLE_SLOTS)))
        .clone();
    let mut owner = Owner::begin(ctx, state, limit).await?;
    for stage in [
        Stage::Audit,
        Stage::Filter,
        Stage::Route,
        Stage::RateLimit,
        Stage::Cash,
    ] {
        if let Err(error) = owner.check() {
            return owner.finish(Err(error)).await;
        }
        let (next, result) = owner.step(stage).await?;
        owner = next;
        if let Err(error) = result {
            return owner.finish(Err(error)).await;
        }
    }
    if let Err(error) = owner.check() {
        return owner.finish(Err(error)).await;
    }
    let state = owner.state.clone();
    // No spawn here: dropping the caller must drop the actual HTTP/body/generation owner first.
    let output = Box::pin(cpa_scope::send(owner.ctx.as_mut().unwrap(), &state)).await;
    // No await may separate an SSE output from handoff, or stream and Owner could both refund.
    owner.finish(output).await
}

impl Owner {
    async fn begin(
        ctx: PipelineContext,
        state: &Arc<AppState>,
        limit: Arc<Semaphore>,
    ) -> Result<Self, GatewayError> {
        ctx.request_budget.check()?;
        let mut cancelled = ctx.caller_cancel.clone();
        let deadline = ctx.request_budget.admission_deadline();
        // Limit admitted lifecycles, not only SQL tasks. Waiting callers have made no reservation.
        let permit = tokio::select! {
            biased;
            _ = state.shutdown.wait() => return Err(ctx.request_budget.cancel_shutdown()),
            _ = async {
                match cancelled.as_mut() {
                    Some(cancel) => while !*cancel.borrow_and_update() {
                        if cancel.changed().await.is_err() { break; }
                    },
                    None => std::future::pending().await,
                }
            } => return Err(ctx.request_budget.cancel_caller()),
            _ = async { match deadline { Some(at) => tokio::time::sleep_until(at).await, None => std::future::pending().await } } => {
                return Err(ctx.request_budget.check().err().unwrap_or_else(|| GatewayError::service_unavailable("Lifecycle admission deadline elapsed")));
            },
            permit = limit.acquire_owned() => permit.map_err(|_| GatewayError::service_unavailable("Lifecycle admission unavailable"))?,
        };
        let drain = state
            .local_runtime
            .as_ref()
            .map(|local| local.track_finalizer());
        Ok(Self {
            ctx: Some(Box::new(ctx)),
            state: state.clone(),
            lifetime: Some(Lifetime {
                _permit: permit,
                _drain: drain,
            }),
            #[cfg(test)]
            hooks: test_support::capture(),
        })
    }
    fn check(&self) -> Result<(), GatewayError> {
        let ctx = self.ctx.as_ref().unwrap();
        if self.state.shutdown.is_requested() {
            return Err(ctx.request_budget.cancel_shutdown());
        }
        if ctx
            .caller_cancel
            .as_ref()
            .is_some_and(|cancel| *cancel.borrow() || cancel.has_changed().is_err())
        {
            return Err(ctx.request_budget.cancel_caller());
        }
        ctx.request_budget.check()
    }
    async fn step(
        mut self,
        stage: Stage,
    ) -> Result<(Self, Result<(), GatewayError>), GatewayError> {
        // Return the armed guard itself. A completed but unclaimed output still owns cancellation.
        tokio::spawn(async move {
            #[cfg(test)]
            let hooks = self.hooks.clone();
            #[cfg(test)]
            hooks.before(stage).await;
            let state = self.state.clone();
            let ctx = self.ctx.as_mut().unwrap();
            let result = match stage {
                Stage::Audit => {
                    stage_finalize::begin_request_audit(ctx, &state).await;
                    Ok(())
                }
                Stage::Filter => Box::pin(stage_filter::run(ctx, &state)).await,
                Stage::Route => Box::pin(stage_route::run(ctx, &state)).await,
                Stage::RateLimit => Box::pin(stage_rate_limit::run(ctx, &state)).await,
                Stage::Cash => Box::pin(crate::cash_billing::admission::prepare(ctx, &state)).await,
                Stage::RouteUnreserved => {
                    Box::pin(stage_route::select_for_durable_reservation(ctx, &state)).await
                }
                Stage::AutoReserve => stage_route::reserve_preselected_auto(ctx, &state).await,
            };
            #[cfg(test)]
            hooks.after(stage).await;
            #[cfg(test)]
            hooks.returning();
            (self, result)
        })
        .await
        .map_err(|_| GatewayError::server_error("Request admission task failed"))
    }
    async fn finish(
        mut self,
        result: Result<PipelineOutput, GatewayError>,
    ) -> Result<PipelineOutput, GatewayError> {
        let ctx = self.ctx.take().unwrap();
        if matches!(&result, Ok(PipelineOutput::Sse(_))) {
            // TrackedStream already owns audit/quota/cash snapshots. Never create a second finalizer.
            drop(ctx);
            return result;
        }
        let state = self.state.clone();
        let lifetime = self.lifetime.take();
        #[cfg(test)]
        let hooks = self.hooks.clone();
        // Normal terminal submission consumes cancellation responsibility before the first await.
        tokio::spawn(async move {
            #[cfg(test)]
            hooks.terminal().await;
            let result = match result {
                Ok(PipelineOutput::Json(body)) => {
                    stage_finalize::run(Some(&body), &ctx, &state).await;
                    Ok(PipelineOutput::Json(body))
                }
                Ok(PipelineOutput::Binary(body)) => {
                    stage_finalize::run_non_json_success(&ctx, &state).await;
                    Ok(PipelineOutput::Binary(body))
                }
                Ok(PipelineOutput::Sse(_)) => unreachable!("SSE transfers ownership synchronously"),
                Err(error) => {
                    stage_finalize::run_failure(&error, &ctx, &state).await;
                    Err(error)
                }
            };
            // CashGuard Drop can register its existing idempotent fallback before drain is released.
            drop(ctx);
            drop(lifetime);
            result
        })
        .await
        .map_err(|_| GatewayError::server_error("Request finalization task failed"))?
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let Some(ctx) = self.ctx.take() else {
            return;
        };
        let lifetime = self.lifetime.take();
        let state = self.state.clone();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            tracing::warn!("Request cancellation storage needs an active runtime");
            return;
        };
        runtime.spawn(async move {
            stage_finalize::run_cancelled(&ctx, &state).await;
            drop(ctx);
            drop(lifetime);
        });
    }
}

#[cfg(test)]
#[path = "lifecycle_cash_tests.rs"]
mod cash_tests;
#[cfg(test)]
#[path = "lifecycle_handoff_tests.rs"]
mod handoff_tests;
#[cfg(test)]
#[path = "lifecycle_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
