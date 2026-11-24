//! Registered AWS IAM Identity Center JSON refresh for Amazon Quick (`quick-oauth`).
//! Evaluated before `kiro::owns` so `oidc.*.amazonaws.com` grants carrying `quick_oauth` context
//! stay bound to Quick's own client registration and tenant payload invariants.

use super::transport::{RefreshError, TokenResponse};
use crate::protocol::quick_oauth::{metadata, owns_payload, OAuthError, RefreshGrant};
use crate::routing::{candidate::ProviderAccountPayload, config::RefreshAttempt};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(super) struct Prepared(RefreshGrant);

pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    attempt.quick_oauth.is_some() || owns_payload(payload)
}

pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let invalid = || RefreshError {
        code: "quick_oauth_refresh_context_required",
        terminal: false,
    };
    if !cfg!(feature = "line-quick-oauth") {
        return Err(RefreshError {
            code: "quick_oauth_feature_disabled",
            terminal: false,
        });
    }
    let context = attempt.quick_oauth.as_ref().ok_or_else(invalid)?;
    if attempt.endpoint != context.token_endpoint()
        || attempt.client_id != context.client_id()
        || attempt.context.is_some()
        || attempt.kiro_oauth.is_some()
        || attempt.accio_sgcookie.is_some()
    {
        return Err(invalid());
    }
    metadata::validate_payload(payload, context).map_err(|_| invalid())?;
    context
        .refresh_grant(&attempt.refresh_token)
        .map(Prepared)
        .map_err(classify)
}

impl Prepared {
    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<TokenResponse, RefreshError> {
        let refreshed = self.0.execute(http).await.map_err(classify)?;
        let expiry =
            OffsetDateTime::now_utc() + time::Duration::seconds(refreshed.expires_in_secs as i64);
        let absolute_expiry = expiry
            .format(&Rfc3339)
            .map_err(|_| classify(OAuthError::InvalidResponse))?;
        Ok(TokenResponse {
            access_token: refreshed.bearer_token,
            refresh_token: refreshed.refresh_token,
            expires_in: Some(refreshed.expires_in_secs),
            absolute_expiry: Some(absolute_expiry),
            grok_oauth: None,
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
        OAuthError::Expired | OAuthError::ReauthorizationRequired | OAuthError::AccessDenied
    );
    RefreshError {
        code: if terminal {
            "quick_oauth_reauthorization_required"
        } else {
            "quick_oauth_refresh_failed"
        },
        terminal,
    }
}
