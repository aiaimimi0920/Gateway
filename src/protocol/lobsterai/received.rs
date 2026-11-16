//! Bounded raw receipts survive rejected admission without authorizing another network exchange.
use super::{receipt, transport, AuthorizationContext, LoginError, SessionMaterial};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReceivedTokens(Value);
impl ReceivedTokens {
    pub fn from_json(bytes: &[u8]) -> Result<Self, LoginError> {
        if bytes.len() > transport::TOKEN_LIMIT {
            return Err(LoginError::ResponseTooLarge);
        }
        let raw =
            crate::protocol::unique_json::parse(bytes).map_err(|_| LoginError::InvalidResponse)?;
        Self::capture(raw)
    }
    pub(super) fn capture(raw: Value) -> Result<Self, LoginError> {
        let receipt = Self(raw);
        receipt.validate_storage()?;
        Ok(receipt)
    }
    pub fn validate_storage(&self) -> Result<(), LoginError> {
        if self.0["code"].as_i64() != Some(0)
            || !self.0["data"].is_object()
            || self.0.to_string().len() > transport::TOKEN_LIMIT
        {
            return Err(LoginError::InvalidResponse);
        }
        Ok(())
    }
    pub fn admit_initial(&self, installation: uuid::Uuid) -> Result<SessionMaterial, LoginError> {
        self.validate_storage()?;
        receipt::interpret(&self.0, installation, None)
    }
    pub fn interpret_successor(
        &self,
        context: &AuthorizationContext,
        access: &str,
        refresh: &str,
    ) -> Result<SessionMaterial, LoginError> {
        self.validate_storage()?;
        context.validate_binding(access, refresh)?;
        receipt::interpret(&self.0, context.installation_id(), Some((context, refresh)))
    }
}
impl std::fmt::Debug for ReceivedTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterReceivedTokens { material: <redacted>, publishable: false }")
    }
}
