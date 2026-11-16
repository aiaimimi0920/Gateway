//! A received successor is consumption evidence, not a publishable credential or verified identity.
use super::{
    atom, unix_now, wire, AuthorizationContext, AuthorizedTokens, OAuthClient, OAuthError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceivedTokens {
    #[serde(deserialize_with = "super::wire::deserialize_object")]
    response: Value,
    received_at: u64,
}
impl ReceivedTokens {
    /// Captures bounded raw metadata even when JWT, expiry, scope or target admission will fail.
    pub fn from_json(bytes: &[u8]) -> Result<Self, OAuthError> {
        if bytes.len() > super::transport::MAX_RESPONSE {
            return Err(OAuthError::ResponseTooLarge);
        }
        Self::capture(wire::object(bytes)?)
    }
    pub(super) fn capture(response: Value) -> Result<Self, OAuthError> {
        let receipt = Self {
            response,
            received_at: unix_now(),
        };
        receipt.validate_storage()?;
        Ok(receipt)
    }
    pub(crate) fn validate_storage(&self) -> Result<(), OAuthError> {
        if !self.response.is_object()
            || !self
                .response
                .get("refresh_token")
                .and_then(Value::as_str)
                .is_some_and(|s| atom(s, 32_768))
            || self.response.to_string().len() > super::transport::MAX_RESPONSE
            || i64::try_from(self.received_at)
                .ok()
                .and_then(|v| time::OffsetDateTime::from_unix_timestamp(v).ok())
                .is_none()
        {
            return Err(OAuthError::InvalidResponse);
        }
        Ok(())
    }
    pub fn refresh_token(&self) -> Result<&str, OAuthError> {
        self.validate_storage()?;
        self.response["refresh_token"]
            .as_str()
            .ok_or(OAuthError::MissingSuccessor)
    }
    pub fn received_at_unix(&self) -> u64 {
        self.received_at
    }
    pub fn admit(
        &self,
        client: &OAuthClient,
        previous: Option<&AuthorizationContext>,
    ) -> Result<AuthorizedTokens, OAuthError> {
        let tokens = self.interpret(client, previous)?;
        if tokens.expires_at_unix <= unix_now() {
            return Err(OAuthError::Expired);
        }
        Ok(tokens)
    }
    /// Historical receipt validation must not grant a fresh TTL or reject history merely for age.
    pub(crate) fn interpret(
        &self,
        client: &OAuthClient,
        previous: Option<&AuthorizationContext>,
    ) -> Result<AuthorizedTokens, OAuthError> {
        self.validate_storage()?;
        super::context::interpret(&self.response, self.received_at, client, previous)
    }
    pub(crate) fn validate_successor(&self, old_refresh: &str) -> Result<(), OAuthError> {
        self.validate_storage()?;
        if self.refresh_token()? == old_refresh {
            return Err(OAuthError::MissingSuccessor);
        }
        Ok(())
    }
    pub(crate) fn interpret_successor(
        &self,
        client: &OAuthClient,
        old_access: &str,
        old_refresh: &str,
        previous: Option<&AuthorizationContext>,
    ) -> Result<AuthorizedTokens, OAuthError> {
        self.validate_successor(old_refresh)?;
        if previous.is_none() && self.response.get("inference_base_url").is_none() {
            return Err(OAuthError::UnsafeInferenceTarget);
        }
        if let Some(previous) = previous {
            previous.validate_binding(old_access, client)?;
        }
        let tokens = self.interpret(client, previous)?;
        // An exact refresh chain gives continuity; only a known old subject adds a hint constraint.
        if super::context::subject_hint(old_access)
            .ok()
            .flatten()
            .is_some_and(|old| tokens.context.subject_hint() != Some(old.as_str()))
        {
            return Err(OAuthError::SubjectHintMismatch);
        }
        Ok(tokens)
    }
}
impl std::fmt::Debug for ReceivedTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NousReceivedTokens { material: <redacted>, admitted: false }")
    }
}
