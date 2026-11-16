//! Consume authorization codes once; a lost exchange is not retried and does not yield a receipt.
use super::{
    grant::{unix_now, Tokens},
    reply, transport, CodeGrant, GrantReceipt, OAuthError,
};
use futures::StreamExt;
use std::time::Duration;

impl CodeGrant {
    pub async fn exchange(self, http: &rquest::Client) -> Result<GrantReceipt, OAuthError> {
        let auth = self.authorization;
        let budget = Duration::from_secs(15).min(auth.remaining());
        if budget.is_zero() {
            return Err(OAuthError::Expired);
        }
        // One deadline includes token delivery, key verification and userinfo, not three budgets.
        tokio::time::timeout(budget, async {
            let tokens = post(
                http,
                auth.metadata.token_endpoint(),
                &[
                    ("grant_type", "authorization_code"),
                    ("code", &self.code),
                    ("client_id", auth.client().client_id()),
                    ("redirect_uri", &auth.redirect),
                    ("code_verifier", &auth.verifier),
                ],
                true,
            )
            .await?;
            let identity = if let Some(token) = &tokens.id_token {
                let keys =
                    transport::get(http, auth.client(), auth.metadata.jwks_uri(), None, 65_536)
                        .await?;
                Some(super::identity::verify(
                    auth.client(),
                    token,
                    auth.nonce(),
                    &tokens.access_token,
                    &keys,
                    unix_now()?,
                )?)
            } else {
                None
            };
            let user = transport::get(
                http,
                auth.client(),
                auth.metadata.userinfo_endpoint(),
                Some(&tokens.access_token),
                65_536,
            )
            .await?;
            if auth.remaining().is_zero() {
                return Err(OAuthError::Expired);
            }
            GrantReceipt::accept(auth.metadata, tokens, &user, identity, auth.nonce)
        })
        .await
        .map_err(|_| OAuthError::Timeout)?
    }
}

pub(super) async fn post(
    http: &rquest::Client,
    endpoint: &str,
    fields: &[(&str, &str)],
    require_refresh: bool,
) -> Result<Tokens, OAuthError> {
    let endpoint = endpoint.to_owned();
    #[cfg(test)]
    let endpoint = transport::test_target(&endpoint);
    let budget = Duration::from_secs(15);
    #[cfg(test)]
    let budget = transport::TEST_BUDGET
        .try_with(|value| budget.min(*value))
        .unwrap_or(budget);
    let started = unix_now()?;
    tokio::time::timeout(budget, async {
        let response = http
            .post(endpoint)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header("accept", "application/json")
            .header("user-agent", "NeuroGateway/0.1 dim-oauth")
            .header("x-title", "Neuro Gateway")
            .form(fields)
            .send()
            .await
            .map_err(transport::transport_error)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(OAuthError::RedirectRejected);
        }
        if status.as_u16() == 401 {
            return Err(OAuthError::ReauthorizationRequired);
        }
        if response
            .content_length()
            .is_some_and(|length| length > 65_536)
        {
            return Err(OAuthError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(transport::transport_error)?;
            if bytes.len().saturating_add(chunk.len()) > 65_536 {
                return Err(OAuthError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| OAuthError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            let terminal = reply::object(&bytes, 65_536).ok().is_some_and(|v| {
                matches!(v["error"].as_str(), Some("invalid_grant" | "invalid_token"))
            });
            return Err(if terminal {
                OAuthError::ReauthorizationRequired
            } else {
                OAuthError::Rejected
            });
        }
        Tokens::parse(&bytes, require_refresh, started)
    })
    .await
    .map_err(|_| OAuthError::Timeout)?
}
