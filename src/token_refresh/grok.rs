//! Build refresh is fixed-audience and keeps the existing scheduler/generation/rotation owners.
use super::transport::{RefreshError, TokenResponse};
use crate::{
    protocol::grok_oauth::{business, OAuthError, TOKEN_ENDPOINT},
    routing::{candidate::ProviderAccountPayload, config::RefreshAttempt},
};

pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    attempt.context.is_some()
        || business::owns_payload(payload)
        || url::Url::parse(&attempt.endpoint).is_ok_and(|url| url.host_str() == Some("auth.x.ai"))
}

pub(super) struct Prepared {
    client: crate::protocol::grok_oauth::OAuthClient,
    context: crate::protocol::grok_oauth::AuthorizationContext,
    refresh: String,
}

pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let invalid = || RefreshError {
        code: "grok_oauth_refresh_invalid_context",
        terminal: false,
    };
    let context = attempt.context.as_ref().ok_or_else(invalid)?;
    if attempt.endpoint != TOKEN_ENDPOINT
        || attempt.client_id != context.client_id()
        || !business::owns_payload(payload)
    {
        return Err(invalid());
    }
    business::validate(payload, payload.canonical_adapter()).map_err(|_| invalid())?;
    let client = context.client().map_err(|_| invalid())?;
    crate::protocol::grok_oauth::OAuthClient::validate_refresh_token(&attempt.refresh_token)
        .map_err(classify)?;
    // This scope is absent from release code and cannot change a stored destination.
    #[cfg(test)]
    let client = TEST_ISSUER
        .try_with(|issuer| client.clone().test_issuer(issuer))
        .unwrap_or(client);
    Ok(Prepared {
        client,
        context: context.clone(),
        refresh: attempt.refresh_token.clone(),
    })
}

impl Prepared {
    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<TokenResponse, RefreshError> {
        let tokens = self
            .client
            .refresh(http, &self.refresh)
            .await
            .map_err(classify)?;
        let context = self.context.refreshed(&tokens).map_err(classify)?;
        // Unknown expiry must not become a made-up six-hour authorization on every restart.
        if tokens.expires_in.is_none() {
            return Err(classify(OAuthError::InvalidResponse));
        }
        Ok(TokenResponse {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
            grok_oauth: Some(context),
            absolute_expiry: None,
            accio_sgcookie: None,
            vertex_oauth: None,
            dim_oauth: None,
            nous_portal: None,
            lobsterai: None,
        })
    }
}

fn classify(error: OAuthError) -> RefreshError {
    let terminal = matches!(
        error,
        OAuthError::ReauthorizationRequired
            | OAuthError::PrincipalMismatch
            | OAuthError::InvalidIdentity
            | OAuthError::AccessDenied
            | OAuthError::Expired
    );
    RefreshError {
        code: if terminal {
            "grok_oauth_reauthorization_required"
        } else {
            "grok_oauth_refresh_failed"
        },
        terminal,
    }
}

#[cfg(test)]
tokio::task_local! {
    pub(super) static TEST_ISSUER: String;
}
