//! End only a confirmed local attempt; a foreign observed session is never adopted or deleted.
use super::*;
use crate::protocol::freebuff::catalog::SessionEndReceipt;
use std::time::Duration;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EndRequest {
    provider_id: String,
    credential_id: String,
    expected_revision: String,
    attempt_id: String,
}
fn changed() -> GatewayError {
    GatewayError::conflict("FreeBuff original attempt changed or is absent. Inspect the saved account before confirming end.")
        .with_code("freebuff_session_claim_changed")
}
fn result(
    input: &CatalogRequest,
    attempt: &str,
    model: &str,
    receipt: SessionEndReceipt,
    from_history: bool,
) -> Response {
    let released = receipt.is_final();
    reply(
        input,
        serde_json::json!({"attemptId":attempt,"modelKey":model,"receipt":receipt,"localReleased":released,"fromHistory":from_history}),
    )
}
pub async fn end_freebuff_claim(
    State(state): State<Arc<AppState>>,
    connect: Option<ConnectInfo<SocketAddr>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(body): Json<EndRequest>,
) -> Result<Response, GatewayError> {
    let (mut account, fence) = prepare(
        &state,
        connect,
        token.as_deref(),
        &headers,
        &body.provider_id,
        &body.credential_id,
        &body.expected_revision,
    )?;
    let id = attempt(&body.attempt_id)?;
    tokio::time::timeout(Duration::from_secs(20), async {
        fence()?;
        let scope = account.session.stored_claim_scope();
        let journal = account
            .local
            .freebuff_claim_journal(&body.provider_id, &body.credential_id, &scope, None)
            .await?;
        if journal
            .claim
            .as_ref()
            .is_none_or(|claim| claim.attempt_id != body.attempt_id)
        {
            // Reconcile a committed closeout even when its original HTTP waiter lost the result.
            // Never apply an old attempt's confirmation to a newer active claim.
            let closed = account
                .local
                .freebuff_closeout_for_scope(&body.provider_id, &body.credential_id, &scope, id)
                .await?
                .ok_or_else(changed)?;
            fence()?;
            return Ok(result(
                &account.input,
                &body.attempt_id,
                &closed.model_key,
                closed.receipt,
                true,
            ));
        }
        let device = account
            .local
            .freebuff_registration(&body.provider_id, &body.credential_id, &account.session)
            .await?
            .ok_or_else(|| {
                GatewayError::conflict(
                    "Register the original saved account explicitly before ending its claim.",
                )
                .with_code("freebuff_device_registration_required")
            })?;
        let _slot = crate::provider_discovery::background::SLOTS
            .try_acquire()
            .map_err(|_| GatewayError::conflict("Discovery is busy; try again shortly."))?;
        fence()?;
        account
            .session
            .refresh(Some(&device), Duration::from_secs(10))
            .await
            .map_err(failure)?;
        fence()?;
        let requests = account
            .session
            .recovery_requests(&device)
            .map_err(end_error)?;
        let original = account
            .local
            .recover_freebuff_admission(&body.provider_id, &body.credential_id, &requests)
            .await?
            .ok_or_else(changed)?;
        if original.claim().attempt_id() != body.attempt_id {
            return Err(changed());
        }
        let model = original.model_key().to_owned();
        let receipt = original
            .end(&requests, &fence, Duration::from_secs(20))
            .await?;
        fence()?;
        Ok(result(
            &account.input,
            &body.attempt_id,
            &model,
            receipt,
            false,
        ))
    })
    .await
    .map_err(|_| end_error(BusinessError::Timeout))?
}
