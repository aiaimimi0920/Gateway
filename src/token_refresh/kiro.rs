//! Registered AWS JSON refresh, pinned before durable admission; never a generic form fallback.
use super::transport::{RefreshError, TokenResponse};
use crate::protocol::kiro_oauth::{business, OAuthError, RefreshGrant};
use crate::routing::{candidate::ProviderAccountPayload, config::RefreshAttempt};

pub(super) struct Prepared(RefreshGrant);

pub(super) fn owns(attempt: &RefreshAttempt, payload: &ProviderAccountPayload) -> bool {
    attempt.kiro_oauth.is_some()
        || payload.canonical_adapter() == "kiro_compatible"
        || url::Url::parse(&attempt.endpoint)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .is_some_and(|host| {
                (host.starts_with("oidc.") || host.starts_with("oidc-fips."))
                    && [".amazonaws.com", ".amazonaws.com.cn", ".api.aws"]
                        .iter()
                        .any(|suffix| host.ends_with(suffix))
            })
}

pub(super) fn prepare(
    attempt: &RefreshAttempt,
    payload: &ProviderAccountPayload,
) -> Result<Prepared, RefreshError> {
    let invalid = || RefreshError {
        code: "kiro_oauth_refresh_context_required",
        terminal: false,
    };
    if !cfg!(feature = "line-kiro-official-vendor-api") {
        return Err(RefreshError {
            code: "kiro_oauth_feature_disabled",
            terminal: false,
        });
    }
    let context = attempt.kiro_oauth.as_ref().ok_or_else(invalid)?;
    if attempt.endpoint != context.token_endpoint()
        || attempt.client_id != context.client_id()
        || attempt.context.is_some()
        || attempt.accio_sgcookie.is_some()
    {
        return Err(invalid());
    }
    business::validate_payload(payload, context).map_err(|_| invalid())?;
    context
        .registration()
        .map_err(classify)?
        .refresh_grant(&attempt.refresh_token)
        .map(Prepared)
        .map_err(classify)
}

impl Prepared {
    pub(super) async fn execute(
        self,
        http: &rquest::Client,
    ) -> Result<TokenResponse, RefreshError> {
        let material = self.0.execute(http).await.map_err(classify)?;
        let expiry = material
            .expires_at()
            .ok_or_else(|| classify(OAuthError::InvalidResponse))?;
        let absolute_expiry = expiry
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|_| classify(OAuthError::InvalidResponse))?;
        // Compatibility lifetime only; publication, replay and dispatch retain the original expiry.
        let lifetime = (expiry - time::OffsetDateTime::now_utc())
            .whole_seconds()
            .max(1) as u64;
        Ok(TokenResponse {
            access_token: material.access_token().into(),
            refresh_token: material.refresh_token().map(str::to_owned),
            expires_in: Some(lifetime),
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
        OAuthError::RegistrationExpired
            | OAuthError::ReauthorizationRequired
            | OAuthError::AccessDenied
            | OAuthError::Expired
    );
    RefreshError {
        code: if terminal {
            "kiro_oauth_reauthorization_required"
        } else {
            "kiro_oauth_refresh_failed"
        },
        terminal,
    }
}
