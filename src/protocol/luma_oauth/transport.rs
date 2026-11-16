//! Fixed token destination, single attempt, one send/read deadline and bounded decoded body.
use super::{CodeGrant, OAuthError, TokenMaterial, RESOURCE, TOKEN_ENDPOINT};
use std::time::Duration;
use time::OffsetDateTime;

#[cfg(test)]
use super::token_response::LIMIT;
const DEADLINE: Duration = Duration::from_secs(15);

impl CodeGrant {
    /// Caller supplies a dedicated OAuth client without cookie storage or custom retry policy.
    /// Dropping the future releases HTTP ownership; this owner never spawns or retries code exchange.
    pub async fn exchange(self, http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        #[cfg(test)]
        if let Ok(endpoint) = super::test_support::ENDPOINT.try_with(Clone::clone) {
            return self.exchange_at(http, &endpoint, DEADLINE).await;
        }
        self.exchange_at(http, TOKEN_ENDPOINT, DEADLINE).await
    }

    async fn exchange_at(
        self,
        http: &rquest::Client,
        endpoint: &str,
        budget: Duration,
    ) -> Result<TokenMaterial, OAuthError> {
        let budget = budget.min(DEADLINE).min(self.authorization.remaining());
        if budget.is_zero() {
            return Err(OAuthError::Expired);
        }
        let started_at = OffsetDateTime::now_utc();
        tokio::time::timeout(budget, async {
            let auth = &self.authorization;
            let response = http
                .post(endpoint)
                .default_headers(false)
                .redirect(rquest::redirect::Policy::none())
                .timeout(budget)
                .header("accept", "application/json")
                .form(&[
                    ("grant_type", "authorization_code"),
                    ("code", self.code.as_str()),
                    ("client_id", auth.client_id.as_str()),
                    ("redirect_uri", auth.redirect.as_str()),
                    ("code_verifier", auth.verifier.as_str()),
                    ("resource", RESOURCE),
                ])
                .send()
                .await
                .map_err(|_| OAuthError::Transport)?;
            let bytes = super::token_response::read_response(response).await?;
            let parse = if auth.offline_access {
                TokenMaterial::parse_offline
            } else {
                TokenMaterial::parse
            };
            parse(
                &bytes,
                started_at,
                auth.client_id.clone(),
                auth.redirect.clone(),
            )
        })
        .await
        .map_err(|_| OAuthError::Timeout)?
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
