//! Fixed-origin JSON only; one HTTP attempt, bounded decoded body and one send+read deadline.
use super::{CodeGrant, OAuthError, RefreshGrant, TokenMaterial, REFRESH_ENDPOINT, TOKEN_ENDPOINT};
use futures::StreamExt;
use serde_json::{json, Value};
use std::time::Duration;

const MAX_RESPONSE: usize = 65_536;
const DEADLINE: Duration = Duration::from_secs(15);

impl CodeGrant {
    /// Consuming this capability prevents retrying an ambiguously consumed authorization code.
    pub async fn exchange(self, http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        #[cfg(test)]
        if let Ok(endpoint) = super::TEST_TOKEN_ENDPOINT.try_with(Clone::clone) {
            loopback(&endpoint);
            return self.exchange_at(http, &endpoint).await;
        }
        self.exchange_at(http, TOKEN_ENDPOINT).await
    }
    async fn exchange_at(
        self,
        http: &rquest::Client,
        endpoint: &str,
    ) -> Result<TokenMaterial, OAuthError> {
        let remaining = self.remaining();
        if remaining.is_zero() {
            return Err(OAuthError::Expired);
        }
        let body = json!({"code":self.code, "codeVerifier":self.verifier,
            "clientId":self.client.client_id(), "redirectUri":self.redirect});
        post(http, endpoint, body, None, DEADLINE.min(remaining)).await
    }
}
impl RefreshGrant {
    /// No automatic retry: a transport failure does not prove that refresh was not rotated.
    pub async fn execute(self, http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        #[cfg(test)]
        if let Ok(endpoint) = super::TEST_REFRESH_ENDPOINT.try_with(Clone::clone) {
            loopback(&endpoint);
            return self.execute_at(http, &endpoint).await;
        }
        self.execute_at(http, REFRESH_ENDPOINT).await
    }
    async fn execute_at(
        self,
        http: &rquest::Client,
        endpoint: &str,
    ) -> Result<TokenMaterial, OAuthError> {
        let body = json!({"accessToken":self.access, "refreshToken":self.refresh});
        post(http, endpoint, body, self.previous_sgcookie, DEADLINE).await
    }
}

async fn post(
    http: &rquest::Client,
    endpoint: &str,
    body: Value,
    previous_cookie: Option<String>,
    budget: Duration,
) -> Result<TokenMaterial, OAuthError> {
    tokio::time::timeout(budget, async {
        let response = http
            .post(endpoint)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(budget)
            .header("accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| OAuthError::Transport)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(OAuthError::RedirectRejected);
        }
        // No public machine-readable terminal codes have been proved. Do not match error prose.
        if !status.is_success() {
            return Err(OAuthError::Rejected);
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE as u64)
        {
            return Err(OAuthError::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| OAuthError::Transport)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
                return Err(OAuthError::ResponseTooLarge);
            }
            bytes
                .try_reserve(chunk.len())
                .map_err(|_| OAuthError::ResponseTooLarge)?;
            bytes.extend_from_slice(&chunk);
        }
        TokenMaterial::parse(&bytes, previous_cookie)
    })
    .await
    .map_err(|_| OAuthError::Timeout)?
}

// Private, compile-time test seam only. Stored/user configuration cannot select an OAuth origin.
#[cfg(test)]
pub(super) async fn test_exchange(
    grant: CodeGrant,
    http: &rquest::Client,
    endpoint: &str,
) -> Result<TokenMaterial, OAuthError> {
    loopback(endpoint);
    grant.exchange_at(http, endpoint).await
}
#[cfg(test)]
pub(super) async fn test_refresh(
    grant: RefreshGrant,
    http: &rquest::Client,
    endpoint: &str,
) -> Result<TokenMaterial, OAuthError> {
    loopback(endpoint);
    grant.execute_at(http, endpoint).await
}
#[cfg(test)]
fn loopback(endpoint: &str) {
    let url = url::Url::parse(endpoint).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
}
#[cfg(test)]
pub(super) async fn test_post(
    http: &rquest::Client,
    endpoint: &str,
    budget: Duration,
) -> Result<TokenMaterial, OAuthError> {
    loopback(endpoint);
    post(
        http,
        endpoint,
        json!({"synthetic":"deadline"}),
        None,
        budget,
    )
    .await
}
