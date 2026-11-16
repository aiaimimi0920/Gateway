//! One-shot public-client registration. A lost response is not permission to register again.
use super::{atom, Authorization, OAuthError};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

pub const REGISTRATION_ENDPOINT: &str = "https://auth.lumalabs.ai/oauth2/register";
const CLIENT_NAME: &str = "Neuro Gateway";
const LIMIT: usize = 65_536;
const BUDGET: Duration = Duration::from_secs(15);

/// Not Clone: the caller must retain a registration receipt and explicitly decide recovery.
pub struct RegistrationRequest {
    redirect: String,
    offline_access: bool,
}

/// Public registration only. Never contains a token, client secret or registration access token.
#[derive(Debug, PartialEq, Eq)]
pub struct RegisteredClient {
    pub client_id: String,
    pub redirect_uri: String,
}

impl RegistrationRequest {
    pub fn new(redirect: &str) -> Result<Self, OAuthError> {
        Authorization::callback_port(redirect)?;
        Ok(Self {
            redirect: redirect.into(),
            offline_access: false,
        })
    }
    pub fn with_offline_access(mut self) -> Self {
        self.offline_access = true;
        self
    }

    /// Only call after explicit approval and while owning the exact callback listener.
    /// Dedicated client must have no cookies or custom retries. This method never retries or spawns.
    pub async fn register(self, http: &rquest::Client) -> Result<RegisteredClient, OAuthError> {
        #[cfg(test)]
        if let Ok(endpoint) = super::test_support::REGISTRATION_ENDPOINT.try_with(Clone::clone) {
            return self.register_at(http, &endpoint, BUDGET).await;
        }
        self.register_at(http, REGISTRATION_ENDPOINT, BUDGET).await
    }

    async fn register_at(
        self,
        http: &rquest::Client,
        endpoint: &str,
        budget: Duration,
    ) -> Result<RegisteredClient, OAuthError> {
        tokio::time::timeout(budget.min(BUDGET), async {
            let response = http
                .post(endpoint)
                .default_headers(false)
                .redirect(rquest::redirect::Policy::none())
                .timeout(budget.min(BUDGET))
                .header("accept", "application/json")
                .json(
                    &json!({"client_name":CLIENT_NAME,"redirect_uris":[self.redirect],
                    "grant_types":if self.offline_access { vec!["authorization_code", "refresh_token"] } else { vec!["authorization_code"] },"response_types":["code"],
                    "token_endpoint_auth_method":"none"}),
                )
                .send()
                .await
                .map_err(|_| OAuthError::Transport)?;
            if response.status().is_redirection() {
                return Err(OAuthError::RedirectRejected);
            }
            if response.status().as_u16() != 201 {
                return Err(OAuthError::Rejected);
            }
            if response
                .content_length()
                .is_some_and(|length| length > LIMIT as u64)
            {
                return Err(OAuthError::ResponseTooLarge);
            }
            let mime = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(';').next())
                .unwrap_or("")
                .trim();
            if !mime.eq_ignore_ascii_case("application/json") {
                return Err(OAuthError::InvalidResponse);
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| OAuthError::Transport)?;
                if bytes.len().saturating_add(chunk.len()) > LIMIT {
                    return Err(OAuthError::ResponseTooLarge);
                }
                bytes
                    .try_reserve(chunk.len())
                    .map_err(|_| OAuthError::ResponseTooLarge)?;
                bytes.extend_from_slice(&chunk);
            }
            parse_with_offline(&bytes, &self.redirect, self.offline_access)
        })
        .await
        .map_err(|_| OAuthError::Timeout)?
    }
}

// Typed deserialization rejects duplicate identity/contract fields; unknown response data is discarded.
#[derive(Deserialize)]
struct Reply {
    client_id: String,
    redirect_uris: Vec<String>,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    token_endpoint_auth_method: String,
    client_secret: Option<String>,
}
#[cfg(test)]
fn parse(bytes: &[u8], redirect: &str) -> Result<RegisteredClient, OAuthError> {
    parse_with_offline(bytes, redirect, false)
}
fn parse_with_offline(
    bytes: &[u8],
    redirect: &str,
    offline_access: bool,
) -> Result<RegisteredClient, OAuthError> {
    let reply: Reply = serde_json::from_slice(bytes).map_err(|_| OAuthError::InvalidResponse)?;
    if !atom(&reply.client_id, 512)
        || reply.redirect_uris != [redirect]
        || if offline_access {
            reply.grant_types.len() != 2
                || !reply.grant_types.iter().any(|g| g == "authorization_code")
                || !reply.grant_types.iter().any(|g| g == "refresh_token")
        } else {
            reply.grant_types != ["authorization_code"]
        }
        || reply.response_types != ["code"]
        || reply.token_endpoint_auth_method != "none"
        || reply.client_secret.is_some()
    {
        return Err(OAuthError::InvalidResponse);
    }
    Ok(RegisteredClient {
        client_id: reply.client_id,
        redirect_uri: redirect.into(),
    })
}

#[cfg(test)]
#[path = "registration_tests.rs"]
mod tests;
