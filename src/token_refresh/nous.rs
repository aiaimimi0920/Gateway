//! Dedicated one-use Nous exchange. Raw receipts are interpreted only after durable acknowledgement.
use super::transport::{RefreshError, TokenResponse};
use crate::{
    console::refresh_grants::GrantRequest,
    protocol::nous_portal::{
        business, OAuthClient, OAuthError, ReceivedTokens, RefreshGrant, TOKEN_ENDPOINT,
    },
    routing::{candidate::ProviderAccountPayload, config::RefreshAttempt},
};

pub(super) struct Prepared {
    grant: RefreshGrant,
}
pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    attempt.nous_portal.is_some() || business::owns_payload(payload)
}
pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let invalid = || RefreshError {
        code: "nous_portal_refresh_context_required",
        terminal: false,
    };
    let context = attempt.nous_portal.as_ref().ok_or_else(invalid)?;
    if attempt.endpoint != TOKEN_ENDPOINT
        || attempt.client_id != context.client_id()
        || attempt.context.is_some()
        || attempt.kiro_oauth.is_some()
        || attempt.vertex_oauth.is_some()
        || attempt.dim_oauth.is_some()
        || attempt.accio_sgcookie.is_some()
        || !attempt
            .expires_at
            .as_deref()
            .is_some_and(|expiry| context.matches_expiry(expiry))
    {
        return Err(invalid());
    }
    business::validate_context(payload, context).map_err(|_| invalid())?;
    let client = OAuthClient::new(&attempt.client_id).map_err(classify)?;
    context
        .validate_binding(&attempt.access_token, &client)
        .map_err(classify)?;
    Ok(Prepared {
        grant: client
            .prepare_refresh(&attempt.refresh_token)
            .map_err(classify)?,
    })
}
impl Prepared {
    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<ReceivedTokens, RefreshError> {
        self.grant.execute(http).await.map_err(classify)
    }
}
pub(super) fn admit(
    request: &GrantRequest,
    receipt: &ReceivedTokens,
) -> Result<TokenResponse, RefreshError> {
    let material = &request.material;
    let client = OAuthClient::new(&material.client).map_err(classify)?;
    let admitted = receipt
        .interpret_successor(
            &client,
            &material.access,
            &material.refresh,
            material.nous_portal.as_ref(),
        )
        .map_err(classify)?;
    let expiry = time::OffsetDateTime::from_unix_timestamp(admitted.expires_at_unix as i64)
        .ok()
        .and_then(|at| {
            at.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .ok_or_else(|| classify(OAuthError::InvalidResponse))?;
    Ok(TokenResponse {
        access_token: admitted.access_token,
        refresh_token: Some(admitted.refresh_token),
        expires_in: Some(admitted.expires_in),
        absolute_expiry: Some(expiry),
        nous_portal: Some(admitted.context),
        lobsterai: None,
        grok_oauth: None,
        vertex_oauth: None,
        dim_oauth: None,
        accio_sgcookie: None,
    })
}
fn classify(error: OAuthError) -> RefreshError {
    use OAuthError::*;
    let code = match error {
        AccessDenied | ReauthorizationRequired => "nous_portal_reauthorization_required",
        UpstreamBlocked => "nous_portal_upstream_blocked",
        TemporarilyUnavailable => "nous_portal_temporarily_unavailable",
        Rejected => "nous_portal_rejected",
        Timeout => "nous_portal_timeout",
        Transport => "nous_portal_transport_failed",
        _ => "nous_portal_refresh_invalid_response",
    };
    RefreshError {
        code,
        terminal: matches!(error, AccessDenied | ReauthorizationRequired),
    }
}
