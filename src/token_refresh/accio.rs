//! Fixed Accio audience; the shared durable owner receives the complete absolute rotation result.
use super::transport::{RefreshError, TokenResponse};
use crate::{
    protocol::accio_oauth::{OAuthClient, RefreshGrant, GATEWAY, REFRESH_ENDPOINT},
    routing::{candidate::ProviderAccountPayload, config::RefreshAttempt},
};

pub(super) struct Prepared(RefreshGrant);

pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    payload.adapter == "accio_compatible"
        || attempt.accio_sgcookie.is_some()
        || [&attempt.endpoint, &payload.base_url]
            .into_iter()
            .any(|value| {
                url::Url::parse(value)
                    .is_ok_and(|url| url.host_str() == Some("phoenix-gw.alibaba.com"))
            })
}

pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let invalid = || RefreshError {
        code: "accio_oauth_refresh_invalid_context",
        terminal: false,
    };
    if !cfg!(feature = "line-accio-web-reverse-api") {
        return Err(RefreshError {
            code: "accio_oauth_feature_disabled",
            terminal: false,
        });
    }
    if payload.adapter != "accio_compatible"
        || attempt.endpoint != REFRESH_ENDPOINT
        || attempt.context.is_some()
        || url::Url::parse(&payload.base_url).ok() != url::Url::parse(GATEWAY).ok()
    {
        return Err(invalid());
    }
    OAuthClient::new(&attempt.client_id).map_err(|_| invalid())?;
    RefreshGrant::new(
        &attempt.access_token,
        &attempt.refresh_token,
        attempt.accio_sgcookie.as_deref(),
    )
    .map(Prepared)
    .map_err(|_| invalid())
}

impl Prepared {
    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<TokenResponse, RefreshError> {
        let tokens = self.0.execute(http).await.map_err(|_| RefreshError {
            // The reference proves no machine-readable permanent rejection classification.
            code: "accio_oauth_refresh_failed",
            terminal: false,
        })?;
        let expiry = tokens
            .expires_at()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| RefreshError {
                code: "accio_oauth_refresh_invalid_response",
                terminal: false,
            })?;
        // Positive compatibility metadata only: admission/dispatch/replay/cache use the original expiry.
        let lifetime = (tokens.expires_at() - time::OffsetDateTime::now_utc())
            .whole_seconds()
            .max(1) as u64;
        Ok(TokenResponse {
            vertex_oauth: None,
            dim_oauth: None,
            nous_portal: None,
            lobsterai: None,
            access_token: tokens.access_token().into(),
            refresh_token: Some(tokens.refresh_token().into()),
            expires_in: Some(lifetime),
            grok_oauth: None,
            absolute_expiry: Some(expiry),
            accio_sgcookie: tokens.sgcookie().map(str::to_owned),
        })
    }
}
