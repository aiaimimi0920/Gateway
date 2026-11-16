//! One owned HTTP attempt per grant. No automatic retry of ambiguously consumed material.
use super::{CodeGrant, OAuthError, RefreshGrant, TokenMaterial};
#[cfg(feature = "line-google-agent-platform-official")]
use std::time::Duration;

impl CodeGrant {
    pub async fn exchange(self, http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        #[cfg(not(feature = "line-google-agent-platform-official"))]
        {
            let _ = http;
            Err(OAuthError::FeatureDisabled)
        }
        #[cfg(feature = "line-google-agent-platform-official")]
        {
            let remaining = self.remaining();
            if remaining.is_zero() {
                return Err(OAuthError::Expired);
            }
            let mut form = vec![
                ("grant_type", "authorization_code"),
                ("client_id", self.client.client_id()),
                ("redirect_uri", &self.redirect),
                ("code", &self.code),
                ("code_verifier", &self.verifier),
            ];
            if let Some(secret) = &self.client.client_secret {
                form.push(("client_secret", secret));
            }
            let material = super::transport_http::post(http, &form, true, remaining).await?;
            if self.remaining().is_zero() {
                return Err(OAuthError::Expired);
            }
            Ok(material)
        }
    }
}
impl RefreshGrant {
    pub async fn execute(self, http: &rquest::Client) -> Result<TokenMaterial, OAuthError> {
        #[cfg(not(feature = "line-google-agent-platform-official"))]
        {
            let _ = http;
            Err(OAuthError::FeatureDisabled)
        }
        #[cfg(feature = "line-google-agent-platform-official")]
        {
            let remaining = match self.refresh_expires_at {
                Some(expiry) => (expiry - time::OffsetDateTime::now_utc())
                    .try_into()
                    .map_err(|_| OAuthError::Expired)?,
                None => Duration::from_secs(15),
            };
            if remaining.is_zero() {
                return Err(OAuthError::Expired);
            }
            let mut form = vec![
                ("grant_type", "refresh_token"),
                ("client_id", self.client.client_id()),
                ("refresh_token", &self.refresh_token),
            ];
            if let Some(secret) = &self.client.client_secret {
                form.push(("client_secret", secret));
            }
            let mut material = super::transport_http::post(http, &form, false, remaining).await?;
            // Omission is not rotation. Never extend a previously known time-based access limit.
            if material.refresh_token.is_none() {
                material.refresh_token = Some(self.refresh_token);
            }
            material.refresh_expires_at =
                match (self.refresh_expires_at, material.refresh_expires_at) {
                    (Some(old), Some(new)) => Some(old.min(new)),
                    (old, new) => old.or(new),
                };
            if material
                .refresh_expires_at
                .is_some_and(|expiry| expiry <= time::OffsetDateTime::now_utc())
            {
                return Err(OAuthError::Expired);
            }
            Ok(material)
        }
    }
}
