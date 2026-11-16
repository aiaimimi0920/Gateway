//! Freeze an actual caller's native intent and wait without holding a provider permit or DB lock.
use super::*;
pub(super) async fn prepare(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    candidate: &RouteCandidate,
    model: &str,
    target: BusinessTarget,
) -> Result<Ready, GatewayError> {
    let body = protocol::request(&ctx.canonical_req).map_err(|_| input_error())?;
    let local = state.local_runtime.clone().ok_or_else(|| {
        GatewayError::conflict("FreeBuff catalog relay requires the local durable claim owner.")
            .with_code("freebuff_catalog_local_required")
    })?;
    let credential = candidate
        .payload
        .credential_id
        .as_deref()
        .ok_or_else(input_error)?;
    let mut session = CatalogSession::new(target.origin, &candidate.payload.api_key, target.client)
        .map_err(|error| business(error.into()))?;
    // Never register or repair a device from a public request; only restore the exact saved receipt.
    let device = local
        .freebuff_registration(&candidate.provider_account_id, credential, &session)
        .await?
        .ok_or_else(|| {
            GatewayError::conflict(
                "Explicitly register the saved FreeBuff account device before calling it.",
            )
            .with_code("freebuff_catalog_device_required")
        })?;
    session
        .refresh(Some(&device), remaining(ctx)?.min(Duration::from_secs(10)))
        .await
        .map_err(|error| business(error.into()))?;
    let requests = session
        .business_requests(&device, model)
        .map_err(business)?;
    let step = requests
        .prepare_relay_step(
            body,
            state
                .config
                .max_body_chat_completions_bytes
                .min(50 * 1024 * 1024),
        )
        .map_err(|error| match error {
            BusinessError::ContextBudgetExceeded => GatewayError::bad_request(
                "FreeBuff native request exceeds the current runtime context budget; shorten it before requesting a purchase quote.",
            ).with_code(error.to_string()),
            _ => input_error().with_code(error.to_string()),
        })?;
    let snapshot = state.route_config.snapshot();
    let binding = RequestBinding::capture(ctx, &snapshot, candidate, model, step.intent())?;
    // Public callers cannot execute another request's old/unknown purchase. Management reconciles it.
    if local
        .recover_freebuff_admission(
            &candidate.provider_account_id,
            credential,
            &requests.recovery_requests(),
        )
        .await?
        .is_some()
    {
        return Err(GatewayError::conflict("Reconcile and explicitly end the original FreeBuff claim before a new public purchase; no existing session was executed or replaced.").with_code("freebuff_public_existing_claim"));
    }
    let quote = requests
        .purchase_quote(
            binding.context(),
            remaining(ctx)?.min(Duration::from_secs(10)),
        )
        .await
        .map_err(|error| {
            GatewayError::conflict(
                "FreeBuff cannot quote this request; no purchase was authorized.",
            )
            .with_code(error.to_string())
        })?;
    let waiting = state
        .lifecycle
        .freebuff_purchases
        .begin(binding, quote, remaining(ctx)?)?;
    let approved = waiting.wait().await?;
    Ok(Ready {
        local,
        session,
        device,
        step,
        approved,
    })
}
