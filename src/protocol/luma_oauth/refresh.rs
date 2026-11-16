//! One-shot public-client refresh. The caller owns durable consumption and result publication.
use super::{
    AuthorizationContext, OAuthError, ReceivedRefresh, TokenMaterial, RESOURCE, TOKEN_ENDPOINT,
};
use serde::Deserialize;
use std::time::Duration;
use time::OffsetDateTime;

const DEADLINE: Duration = Duration::from_secs(15);
pub struct RefreshGrant {
    context: AuthorizationContext,
    refresh_token: String,
}
pub struct RefreshOutcome {
    pub(super) material: TokenMaterial,
    pub(super) refresh_token: String,
}
impl RefreshOutcome {
    pub fn into_parts(self) -> (TokenMaterial, String) {
        (self.material, self.refresh_token)
    }
}
impl std::fmt::Debug for RefreshOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaRefreshOutcome { material: <redacted> }")
    }
}
impl RefreshGrant {
    pub fn new(context: AuthorizationContext, refresh_token: String) -> Result<Self, OAuthError> {
        if !context.offline_access() || !valid_refresh(&refresh_token) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            context,
            refresh_token,
        })
    }
    /// Dedicated no-cookie/no-custom-retry client only. Caller must durably consume the old grant first.
    /// A failed or cancelled exchange may already have rotated the token; never automatically replay it.
    pub async fn exchange(self, http: &rquest::Client) -> Result<RefreshOutcome, OAuthError> {
        self.exchange_at(http, TOKEN_ENDPOINT, DEADLINE).await
    }
    /// Persist the returned evidence before interpreting or publishing it. Reserve the old grant before sending.
    pub async fn receive(self, http: &rquest::Client) -> Result<ReceivedRefresh, OAuthError> {
        #[cfg(test)]
        if let Ok(endpoint) = super::test_support::ENDPOINT.try_with(Clone::clone) {
            return self.receive_at(http, &endpoint, DEADLINE).await;
        }
        self.receive_at(http, TOKEN_ENDPOINT, DEADLINE).await
    }
    async fn exchange_at(
        self,
        http: &rquest::Client,
        endpoint: &str,
        budget: Duration,
    ) -> Result<RefreshOutcome, OAuthError> {
        let received = self.receive_at(http, endpoint, budget).await?;
        received.interpret_successor(&self.context, &self.refresh_token)
    }
    async fn receive_at(
        &self,
        http: &rquest::Client,
        endpoint: &str,
        budget: Duration,
    ) -> Result<ReceivedRefresh, OAuthError> {
        let budget = budget.min(DEADLINE);
        if budget.is_zero() {
            return Err(OAuthError::Timeout);
        }
        let started_at = OffsetDateTime::now_utc();
        tokio::time::timeout(budget, async {
            let response = http
                .post(endpoint)
                .default_headers(false)
                .redirect(rquest::redirect::Policy::none())
                .timeout(budget)
                .header("accept", "application/json")
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("refresh_token", self.refresh_token.as_str()),
                    ("client_id", self.context.client_id()),
                    ("resource", RESOURCE),
                ])
                .send()
                .await
                .map_err(|_| OAuthError::Transport)?;
            let bytes = super::token_response::read_response(response).await?;
            ReceivedRefresh::capture(&bytes, started_at, &self.context, &self.refresh_token)
        })
        .await
        .map_err(|_| OAuthError::Timeout)?
    }
}

#[derive(Deserialize)]
struct RefreshReply {
    // Omission preserves the original token under RFC 6749 section 6; null is not omission.
    #[serde(default, deserialize_with = "refresh_field")]
    refresh_token: Option<String>,
}
fn refresh_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
pub(crate) fn valid_refresh(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32_768 && value.bytes().all(|b| (0x20..=0x7e).contains(&b))
}

pub(super) fn response_refresh(bytes: &[u8]) -> Result<Option<String>, OAuthError> {
    let reply: RefreshReply =
        serde_json::from_slice(bytes).map_err(|_| OAuthError::InvalidResponse)?;
    if reply
        .refresh_token
        .as_deref()
        .is_some_and(|value| !valid_refresh(value))
    {
        return Err(OAuthError::InvalidResponse);
    }
    Ok(reply.refresh_token)
}

#[cfg(test)]
#[path = "received_tests.rs"]
mod received_tests;
#[cfg(test)]
#[path = "refresh_tests.rs"]
mod tests;
