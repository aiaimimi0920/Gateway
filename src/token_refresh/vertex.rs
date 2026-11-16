//! Full Vertex refresh pins its own explicit client and mutable context before durable Sending.
use super::transport::{RefreshError, TokenResponse};
use crate::{
    protocol::vertex_oauth::{
        business, AuthorizationContext, OAuthError, RefreshGrant, TOKEN_ENDPOINT,
    },
    routing::{candidate::ProviderAccountPayload, config::RefreshAttempt},
};
pub(super) struct Prepared {
    grant: RefreshGrant,
    context: AuthorizationContext,
}
pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    attempt.vertex_oauth.is_some() || business::owns_payload(payload)
}
pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let invalid = || RefreshError {
        code: "vertex_oauth_refresh_context_required",
        terminal: false,
    };
    let context = attempt.vertex_oauth.as_ref().ok_or_else(invalid)?;
    if attempt.endpoint != TOKEN_ENDPOINT
        || attempt.client_id != context.client_id()
        || attempt.context.is_some()
        || attempt.kiro_oauth.is_some()
        || attempt.accio_sgcookie.is_some()
    {
        return Err(invalid());
    }
    business::validate_context(payload, context).map_err(|_| invalid())?;
    let grant = context
        .refresh_grant(&attempt.refresh_token)
        .map_err(classify)?;
    Ok(Prepared {
        grant,
        context: context.clone(),
    })
}
impl Prepared {
    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<TokenResponse, RefreshError> {
        let material = self.grant.execute(http).await.map_err(classify)?;
        let context = self.context.rotated(&material).map_err(classify)?;
        Ok(TokenResponse {
            access_token: material.access_token().into(),
            refresh_token: material.refresh_token().map(str::to_owned),
            expires_in: Some(material.expires_in()),
            absolute_expiry: Some(
                material
                    .expires_at()
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(|_| classify(OAuthError::InvalidResponse))?,
            ),
            vertex_oauth: Some(context),
            dim_oauth: None,
            nous_portal: None,
            lobsterai: None,
            grok_oauth: None,
            accio_sgcookie: None,
        })
    }
}
fn classify(error: OAuthError) -> RefreshError {
    let terminal = matches!(
        error,
        OAuthError::Expired
            | OAuthError::ReauthorizationRequired
            | OAuthError::AccessDenied
            | OAuthError::InsufficientScope
    );
    RefreshError {
        code: if terminal {
            "vertex_oauth_reauthorization_required"
        } else {
            "vertex_oauth_refresh_failed"
        },
        terminal,
    }
}
