//! DIM uses the shared durable grant owner; preparation itself sends no refresh material.
use super::transport::{RefreshError, TokenResponse};
use crate::{
    protocol::dim_oauth::{business, OAuthError, RefreshGrant},
    routing::{candidate::ProviderAccountPayload, config::RefreshAttempt},
};
pub(super) struct Prepared {
    grant: RefreshGrant,
}
pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    attempt.dim_oauth.is_some() || business::owns_payload(payload)
}
pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let invalid = || RefreshError {
        code: "dim_oauth_refresh_context_required",
        terminal: false,
    };
    let context = attempt.dim_oauth.as_ref().ok_or_else(invalid)?;
    if attempt.endpoint != context.token_endpoint()
        || attempt.client_id != context.client_id()
        || attempt.context.is_some()
        || attempt.kiro_oauth.is_some()
        || attempt.vertex_oauth.is_some()
        || attempt.accio_sgcookie.is_some()
    {
        return Err(invalid());
    }
    business::validate_context(payload, context).map_err(|_| invalid())?;
    Ok(Prepared {
        grant: context
            .refresh_grant(&attempt.refresh_token)
            .map_err(classify)?,
    })
}
impl Prepared {
    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<TokenResponse, RefreshError> {
        let material = self.grant.execute(http).await.map_err(classify)?;
        let expiry = i64::try_from(material.expires_at_unix)
            .ok()
            .and_then(|timestamp| time::OffsetDateTime::from_unix_timestamp(timestamp).ok())
            .and_then(|at| {
                at.format(&time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .ok_or_else(|| classify(OAuthError::InvalidResponse))?;
        Ok(TokenResponse {
            access_token: material.access_token,
            refresh_token: Some(material.refresh_token),
            expires_in: Some(material.expires_in),
            absolute_expiry: Some(expiry),
            dim_oauth: Some(material.context),
            nous_portal: None,
            lobsterai: None,
            grok_oauth: None,
            vertex_oauth: None,
            accio_sgcookie: None,
        })
    }
}
fn classify(error: OAuthError) -> RefreshError {
    let terminal = matches!(
        error,
        OAuthError::ReauthorizationRequired | OAuthError::AccessDenied
    );
    RefreshError {
        code: if terminal {
            "dim_oauth_reauthorization_required"
        } else {
            "dim_oauth_refresh_failed"
        },
        terminal,
    }
}
