//! Fixed Google origin, bounded decoded response and one send+read budget; no detached work.
use super::{OAuthError, TokenMaterial, TOKEN_ENDPOINT};
use futures::StreamExt;
use serde::Deserialize;
use std::time::Duration;

const MAX_RESPONSE: usize = 65_536;
const DEADLINE: Duration = Duration::from_secs(15);

pub(super) async fn post(
    http: &rquest::Client,
    form: &[(&str, &str)],
    require_refresh: bool,
    remaining: Duration,
) -> Result<TokenMaterial, OAuthError> {
    let endpoint = TOKEN_ENDPOINT.to_owned();
    #[cfg(test)]
    let endpoint = super::transport_test_support::ENDPOINT
        .try_with(Clone::clone)
        .unwrap_or(endpoint);
    let budget = DEADLINE.min(remaining);
    #[cfg(test)]
    let budget = super::transport_test_support::BUDGET
        .try_with(|test| budget.min(*test))
        .unwrap_or(budget);
    let started = time::OffsetDateTime::now_utc();
    tokio::time::timeout(budget, async {
        let response = http
            .post(endpoint)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header("accept", "application/json")
            .form(form)
            .send()
            .await
            .map_err(transport_error)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(OAuthError::RedirectRejected);
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE as u64)
        {
            return Err(OAuthError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        #[cfg(test)]
        let _ = super::transport_test_support::BODY_STARTED.try_with(|notify| notify.notify_one());
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(transport_error)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
                return Err(OAuthError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| OAuthError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            return Err(rejection(status, &bytes));
        }
        TokenMaterial::parse(&bytes, require_refresh, started)
    })
    .await
    .map_err(|_| OAuthError::Timeout)?
}
fn transport_error(error: rquest::Error) -> OAuthError {
    if error.is_timeout() {
        OAuthError::Timeout
    } else {
        OAuthError::Transport
    }
}
fn rejection(status: rquest::StatusCode, bytes: &[u8]) -> OAuthError {
    if !matches!(status.as_u16(), 400 | 401) {
        return OAuthError::Rejected;
    }
    #[derive(Deserialize)]
    struct Rejection {
        error: String,
    }
    match serde_json::from_slice::<Rejection>(bytes)
        .ok()
        .map(|reply| reply.error)
        .as_deref()
    {
        Some("invalid_grant" | "invalid_client" | "unauthorized_client") => {
            OAuthError::ReauthorizationRequired
        }
        Some("access_denied") => OAuthError::AccessDenied,
        _ => OAuthError::Rejected,
    }
}
