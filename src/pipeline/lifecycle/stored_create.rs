//! Decide stored SAP vs ordinary native Responses after one authorized, uncharged
//! route selection. The existing owner protects cancellation on either branch.
use super::*;
use crate::auth::adapter::AuthRequest;

pub(crate) async fn run(
    mut ctx: PipelineContext,
    auth: AuthRequest,
    state: Arc<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<PipelineOutput, GatewayError> {
    stage_auth::run_request(&mut ctx, &state, &auth).await?;
    let limit = ADMISSION
        .get_or_init(|| Arc::new(Semaphore::new(LIFECYCLE_SLOTS)))
        .clone();
    let mut owner = Owner::begin(ctx, &state, limit).await?;
    for stage in [Stage::Audit, Stage::RateLimit] {
        let (next, result) = owner.step(stage).await?;
        owner = next;
        if let Err(error) = result {
            return owner.finish(Err(error)).await;
        }
    }
    let ctx = owner.ctx.as_mut().unwrap();
    if let Err(error) = ctx.request_budget.configure(
        &ctx.canonical_req,
        ctx.route_policy_config
            .as_ref()
            .and_then(|p| p.total_request_timeout_seconds),
    ) {
        return owner.finish(Err(error)).await;
    }
    let gateway_predecessor = ctx
        .canonical_req
        .previous_response_id
        .as_deref()
        .is_some_and(|id| id.starts_with("resp_gw_"));
    if !gateway_predecessor {
        let (next, result) = owner.step(Stage::RouteUnreserved).await?;
        owner = next;
        if let Err(error) = result {
            return owner.finish(Err(error)).await;
        }
    }
    if let Err(error) = owner.check() {
        return owner.finish(Err(error)).await;
    }
    let ctx = owner.ctx.as_ref().unwrap();
    let stored = gateway_predecessor
        || ctx
            .candidates
            .iter()
            .find(|candidate| ctx.provider_allowed_for_send(&candidate.provider_account_id))
            .is_some_and(|candidate| {
                crate::protocol::sap_aicore::responses_lifecycle::enabled(&candidate.payload)
            });
    if stored {
        if ctx
            .session
            .as_ref()
            .and_then(|s| s.access_key_id.as_ref())
            .is_none()
        {
            return owner
                .finish(Err(GatewayError::unauthorized(
                    "Stored Responses requires an access key.",
                )))
                .await;
        }
        let admission = match state.lifecycle.stored_response_tasks.try_admit() {
            Ok(admission) => admission,
            Err(error) => return owner.finish(Err(error)).await,
        };
        let ctx = *owner.ctx.take().unwrap();
        // This guard has no reservation; durable tasks take over before the first
        // write. A dropped waiter can never start the ordinary branch.
        let lifetime = owner.lifetime.take();
        use super::super::stored_responses as durable;
        let result = if ctx.canonical_req.stream {
            durable::run_live(ctx, state, admission).await
        } else {
            durable::run_authenticated(ctx, durable::Request::Create, state, true, admission)
                .await
                .map(PipelineOutput::Json)
        };
        drop(lifetime);
        return result;
    }
    // Experimental Codex session admission belongs only to the ordinary branch;
    // it must not reject a stored SAP request because another candidate is Codex.
    let ctx = owner.ctx.as_mut().unwrap();
    if let Some(wire) = ctx.native_wire.as_mut() {
        if let Err(error) = crate::http::attach_sdk_session(
            &state,
            ctx.bearer_token.as_deref(),
            &headers,
            &ctx.canonical_req.raw_body,
            Arc::make_mut(wire),
        )
        .await
        {
            return owner.finish(Err(error)).await;
        }
    }
    for stage in [Stage::Filter, Stage::AutoReserve, Stage::Cash] {
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
    let result = Box::pin(cpa_scope::send(owner.ctx.as_mut().unwrap(), &state)).await;
    owner.finish(result).await
}
