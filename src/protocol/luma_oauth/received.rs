//! Bounded response evidence is stored before interpretation; base64 is not encryption or authorization.
use super::{
    refresh::response_refresh, token_response::LIMIT, AuthorizationContext, OAuthError,
    RefreshOutcome, TokenMaterial,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReceivedRefresh {
    body: String,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    binding: String,
}

impl ReceivedRefresh {
    pub(crate) fn capture(
        bytes: &[u8],
        started_at: OffsetDateTime,
        context: &AuthorizationContext,
        refresh: &str,
    ) -> Result<Self, OAuthError> {
        if bytes.len() > LIMIT {
            return Err(OAuthError::ResponseTooLarge);
        }
        if !context.offline_access() || !super::valid_refresh(refresh) {
            return Err(OAuthError::InvalidInput);
        }
        started_at
            .format(&Rfc3339)
            .map_err(|_| OAuthError::InvalidInput)?;
        // Preserve exact bytes, including invalid UTF-8 or duplicate JSON fields. Admission comes later.
        Ok(Self {
            body: STANDARD.encode(bytes),
            started_at,
            binding: binding(context, refresh),
        })
    }

    pub fn validate_storage(&self) -> Result<(), OAuthError> {
        self.decode().map(|_| ())
    }

    pub fn validate_binding(
        &self,
        context: &AuthorizationContext,
        refresh: &str,
    ) -> Result<(), OAuthError> {
        self.checked_body(context, refresh).map(|_| ())
    }

    /// Caller must retain the original received object when saving fails, never recapture at a new time.
    /// This hash binds the grant, not file authenticity; storage integrity belongs to its protected owner.
    pub fn interpret_successor(
        &self,
        context: &AuthorizationContext,
        refresh: &str,
    ) -> Result<RefreshOutcome, OAuthError> {
        let outcome = self.interpret_stored(context, refresh)?;
        AuthorizationContext::from_material(&outcome.material)?;
        Ok(outcome)
    }

    /// Preserve a rotated refresh token even when recovery happens after access expiry.
    pub(crate) fn interpret_stored(
        &self,
        context: &AuthorizationContext,
        refresh: &str,
    ) -> Result<RefreshOutcome, OAuthError> {
        let bytes = self.checked_body(context, refresh)?;
        let material = TokenMaterial::parse_stored(
            &bytes,
            self.started_at,
            context.client_id().into(),
            context.redirect_uri().into(),
        )?
        .with_offline_access(true);
        let refresh_token = response_refresh(&bytes)?.unwrap_or_else(|| refresh.to_owned());
        Ok(RefreshOutcome {
            material,
            refresh_token,
        })
    }

    fn checked_body(
        &self,
        context: &AuthorizationContext,
        refresh: &str,
    ) -> Result<Vec<u8>, OAuthError> {
        if !context.offline_access()
            || !super::valid_refresh(refresh)
            || self.binding != binding(context, refresh)
        {
            return Err(OAuthError::InvalidInput);
        }
        self.decode()
    }

    fn decode(&self) -> Result<Vec<u8>, OAuthError> {
        if self.body.len() > LIMIT.div_ceil(3) * 4
            || self.binding.len() != 64
            || !self
                .binding
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.started_at.format(&Rfc3339).is_err()
        {
            return Err(OAuthError::InvalidResponse);
        }
        let bytes = STANDARD
            .decode(&self.body)
            .map_err(|_| OAuthError::InvalidResponse)?;
        if bytes.len() > LIMIT || STANDARD.encode(&bytes) != self.body {
            return Err(OAuthError::InvalidResponse);
        }
        Ok(bytes)
    }
}

fn binding(context: &AuthorizationContext, refresh: &str) -> String {
    let mut value = serde_json::json!(["gateway-luma-refresh-receipt-v1", context, refresh]);
    value.sort_all_objects();
    hex::encode(Sha256::digest(value.to_string().as_bytes()))
}

impl std::fmt::Debug for ReceivedRefresh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaReceivedRefresh { material: <redacted>, publishable: false }")
    }
}
