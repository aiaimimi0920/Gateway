//! Local Lobster refresh uses the shared grant journal, with no inferred expiry or automatic send on restart.
use super::transport::{RefreshError, TokenResponse};
use crate::{
    console::{refresh_grants::*, RouteConfigRedisRevision, RouteConfigRuntime},
    protocol::lobsterai::*,
    routing::{candidate::ProviderAccountPayload, config::*},
};
pub(super) struct Prepared(RefreshGrant);
pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    attempt.lobsterai.is_some() || payload.auth_mode.as_deref() == Some(AUTH_MODE)
}
pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let context = attempt
        .lobsterai
        .as_ref()
        .ok_or_else(|| classify(LoginError::InvalidInput))?;
    if attempt.endpoint != TOKEN_ENDPOINT
        || attempt.client_id != CLIENT_ATTRIBUTION
        || attempt.expires_at.is_some()
        || payload.auth_mode.as_deref() != Some(AUTH_MODE)
        || payload.base_url != crate::routing::config::lobsterai::BASE
        || attempt.context.is_some()
        || attempt.kiro_oauth.is_some()
        || attempt.vertex_oauth.is_some()
        || attempt.dim_oauth.is_some()
        || attempt.nous_portal.is_some()
        || attempt.accio_sgcookie.is_some()
    {
        return Err(classify(LoginError::InvalidInput));
    }
    let builder = crate::http_client::builder();
    #[cfg(test)]
    let builder = builder.no_proxy().proxy(
        rquest::Proxy::all(
            crate::protocol::lobsterai::transport::TEST_ORIGIN
                .try_with(Clone::clone)
                .expect("Lobster refresh tests require loopback scope"),
        )
        .unwrap(),
    );
    let client = Client::new(context.installation_id(), builder).map_err(classify)?;
    let session = SessionTokens::restore(
        client,
        SessionMaterial {
            access_token: attempt.access_token.clone(),
            refresh_token: attempt.refresh_token.clone(),
            context: context.clone(),
        },
    )
    .map_err(classify)?;
    Ok(Prepared(
        session
            .into_refresh_grant(&attempt.access_token, 401)
            .map_err(classify)?,
    ))
}
impl Prepared {
    pub(super) async fn execute(self) -> Result<ReceivedTokens, RefreshError> {
        self.0
            .execute()
            .await
            .and_then(ReceivedSession::into_receipt)
            .map_err(classify)
    }
}
pub(super) fn admit(
    request: &GrantRequest,
    receipt: &ReceivedTokens,
) -> Result<TokenResponse, RefreshError> {
    let old = &request.material;
    let tokens = receipt
        .interpret_successor(
            old.lobsterai
                .as_ref()
                .ok_or_else(|| classify(LoginError::InvalidInput))?,
            &old.access,
            &old.refresh,
        )
        .map_err(classify)?;
    Ok(TokenResponse {
        access_token: tokens.access_token,
        refresh_token: Some(tokens.refresh_token),
        expires_in: None,
        absolute_expiry: None,
        lobsterai: Some(tokens.context),
        grok_oauth: None,
        vertex_oauth: None,
        dim_oauth: None,
        nous_portal: None,
        accio_sgcookie: None,
    })
}
pub(super) fn replay(record: &GrantRecord, request: &GrantRequest) -> Option<ReceivedTokens> {
    let receipt = if record.replay_for(request).is_some() {
        record.receipt()
    } else {
        record.receipt_for(request)
    }?;
    match receipt {
        GrantReceipt::LobsterAi { tokens } => Some(tokens.clone()),
        _ => None,
    }
}
/// This read can mark existing evidence due for disk-only replay; absence never grants a refresh send.
pub(super) async fn recover(
    runtime: &RouteConfigRuntime,
    snapshot: &RouteConfigSnapshot,
    provider: &str,
    credential: &str,
    owner: &TokenRefreshState,
) -> bool {
    if !owner.lobster_recovery_needed() {
        return true;
    }
    let Some(generation) = owner.lobster_dispatch_generation() else {
        return false;
    };
    let Ok(revision) =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
    else {
        return false;
    };
    let Ok(request) = GrantRequest::from_revision(&revision, provider, credential) else {
        return false;
    };
    match tokio::time::timeout(
        super::persistence::authority_budget(),
        runtime.find_refresh_grant(&request),
    )
    .await
    {
        Ok(Ok(record)) => {
            owner.confirm_lobster_recovery(generation, record.is_some());
            true
        }
        _ => false,
    }
}
fn classify(error: LoginError) -> RefreshError {
    RefreshError {
        code: match error {
            LoginError::ReauthorizationRequired => "lobsterai_reauthorization_required",
            LoginError::Timeout => "lobsterai_timeout",
            LoginError::Transport => "lobsterai_transport_failed",
            _ => "lobsterai_refresh_rejected",
        },
        terminal: error == LoginError::ReauthorizationRequired,
    }
}
