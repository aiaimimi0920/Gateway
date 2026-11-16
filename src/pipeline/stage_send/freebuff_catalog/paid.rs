//! One paid public owner: cooperative cancellation and reported-cost closeout of the original run.
use super::*;
use crate::pipeline::request_budget::CLOSEOUT_TIMEOUT;
mod control;
use control::Control;
pub(super) struct Input {
    ready: Ready,
    current: RequestBinding,
    model: String,
    gate: stage_rate_limit::ProviderAttemptGate,
    control: Control,
}
impl Input {
    pub(super) fn capture(
        ctx: &PipelineContext,
        state: &Arc<AppState>,
        candidate: &RouteCandidate,
        model: &str,
        gate: &stage_rate_limit::ProviderAttemptGate,
        ready: Ready,
    ) -> Result<Self, GatewayError> {
        let current = RequestBinding::capture(
            ctx,
            &state.route_config.snapshot(),
            candidate,
            model,
            ready.step.intent(),
        )?;
        Ok(Self {
            ready,
            current,
            model: model.into(),
            gate: gate.clone(),
            control: Control::new(ctx, state),
        })
    }
}

pub(super) async fn execute(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    candidate: &RouteCandidate,
    model: &str,
    gate: &stage_rate_limit::ProviderAttemptGate,
    ready: Ready,
) -> Result<protocol::Completed, GatewayError> {
    let input = Input::capture(ctx, state, candidate, model, gate, ready)?;
    execute_observed(
        input,
        |_| std::future::ready(Ok(())),
        |body| protocol::response(body, &ctx.canonical_req, &ctx.req_id.to_string()),
    )
    .await
}
/// Both JSON and live bodies move through the same paid owner. The fallible final projection is
/// completed before the original FINISH; observing frames never grants run/purchase authority.
pub(super) async fn execute_observed<F, O, P>(
    input: Input,
    observe: O,
    project: P,
) -> Result<protocol::Completed, GatewayError>
where
    O: FnMut(serde_json::Value) -> F,
    F: std::future::Future<Output = Result<(), BusinessError>>,
    P: FnOnce(serde_json::Value) -> Result<serde_json::Value, BusinessError>,
{
    let Input {
        ready,
        current,
        model,
        gate,
        control,
    } = input;
    let Ready {
        local,
        session,
        device,
        step,
        approved,
    } = ready;
    let requests = session
        .business_requests(&device, &model)
        .map_err(business)?;
    approved.check(&current)?;
    let (mut active, fence) = approved
        .admit_new(
            &current,
            &local,
            &requests,
            control.remaining()?,
            || async {
                control.check()?;
                gate.recheck_send_eligibility().await?;
                control.check()
            },
            || {
                control.check()?;
                gate.begin_attempt()
            },
        )
        .await?;
    let control = control.admitted()?;
    // An in-flight purchase is awaited, never replaced. Cancellation afterwards does not START.
    control.check()?;
    fence()?;
    let run = active
        .start_catalog_run(&requests, control.remaining()?)
        .await
        .map_err(|e| control.business(e))?;
    // Only a confirmed run may retain the original dispatch at the outer deadline. Unknown START
    // still drops/retains its claim; no guessed run ID, unbounded grace or detached recovery exists.
    let _closeout = control.budget.retain_deadline_closeout();
    let eligible = gate.recheck_inflight_eligibility().await;
    let eligible = control
        .check()
        .and(eligible)
        .and_then(|_| fence())
        .and_then(|_| control.remaining());
    let budget = match eligible {
        Ok(budget) => budget,
        Err(error) => {
            run.finish_cancelled(CLOSEOUT_TIMEOUT)
                .await
                .map_err(|e| control.business(e))?;
            return Err(error);
        }
    };
    let decoded = run
        .execute_relay_step_observed(step, budget, control.cancelled(), observe)
        .await;
    let (usage, semantics) = protocol::observe(decoded.completion().ok());
    // Projection is fallible BEFORE FINISH: an unrepresentable native completion must not be
    // reported as a completed public run. Native accounting remains owned by the same decoded run.
    let decoded = decoded.map_completion(project);
    // FINISH is explicitly awaited even after cancellation/deadline. Unknown cost rejects it, never
    // invents zero. A FINISH already on the wire is not cancelled, retried or claimed as settlement.
    let finished = decoded
        .finish_run(CLOSEOUT_TIMEOUT)
        .await
        .map_err(|e| control.business(e))?;
    control.check()?;
    let body = finished.completion.map_err(business)?;
    fence()?;
    Ok(protocol::Completed {
        body,
        usage,
        semantics,
    })
}
