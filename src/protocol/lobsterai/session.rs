//! Persistent personal-session binding. The local generation is not a provider token lifetime.
use super::{atom, receipt::Identity, LoginError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorizationContext {
    #[serde(with = "uuid_text")]
    installation_id: uuid::Uuid,
    identity: Identity,
    generation: u64,
    token_binding: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reauthorization_id: Option<String>,
}
impl AuthorizationContext {
    pub(super) fn admitted(
        installation_id: uuid::Uuid,
        identity: Identity,
        generation: u64,
        access: &str,
        refresh: &str,
        reauthorization_id: Option<&str>,
    ) -> Self {
        Self {
            installation_id,
            identity,
            generation,
            token_binding: binding(access, refresh),
            reauthorization_id: reauthorization_id.map(str::to_owned),
        }
    }
    pub fn installation_id(&self) -> uuid::Uuid {
        self.installation_id
    }
    /// Absent for legacy grants, preserving their existing durable family IDs.
    pub(crate) fn reauthorization_id(&self) -> Option<&str> {
        self.reauthorization_id.as_deref()
    }
    pub fn user_id(&self) -> &str {
        self.identity.user_id()
    }
    pub(super) fn identity(&self) -> &Identity {
        &self.identity
    }
    pub(super) fn successor_generation(&self) -> Result<u64, LoginError> {
        self.generation
            .checked_add(1)
            .ok_or(LoginError::ReauthorizationRequired)
    }
    pub fn validate_binding(&self, access: &str, refresh: &str) -> Result<(), LoginError> {
        if self.reauthorization_id.as_ref().is_some_and(|id| {
            uuid::Uuid::parse_str(id)
                .ok()
                .is_none_or(|uuid| uuid.is_nil() || uuid.to_string() != *id)
        }) {
            return Err(LoginError::InvalidInput);
        }
        if self.installation_id.is_nil()
            || !atom(access, 32768)
            || !atom(refresh, 32768)
            || self.token_binding != binding(access, refresh)
        {
            return Err(LoginError::InvalidInput);
        }
        self.identity.validate()
    }
    pub fn permits_rotation(&self, next: &Self) -> bool {
        self.same_account(next)
            && self.reauthorization_id == next.reauthorization_id
            && self.generation.checked_add(1) == Some(next.generation)
    }
    /// A fresh browser grant has a new local generation even when the server repeats tokens.
    pub fn reauthorization(&self, next: &Self) -> Result<Self, LoginError> {
        if !self.same_account(next) {
            return Err(LoginError::IdentityMismatch);
        }
        let mut context = next.clone();
        context.generation = self.successor_generation()?;
        context.reauthorization_id = Some(uuid::Uuid::new_v4().to_string());
        Ok(context)
    }
    fn same_account(&self, next: &Self) -> bool {
        self.installation_id == next.installation_id && self.identity == next.identity
    }
}
fn binding(access: &str, refresh: &str) -> String {
    hex::encode(Sha256::digest(
        serde_json::json!(["lobsterai-session-v1", access, refresh]).to_string(),
    ))
}
/// Tokens are saved through the existing secret owner, never inside the non-secret context.
pub struct SessionMaterial {
    pub access_token: String,
    pub refresh_token: String,
    pub context: AuthorizationContext,
}
impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterAuthorizationContext { identity: <redacted>, expiry: unknown }")
    }
}
impl std::fmt::Debug for SessionMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterSessionMaterial { material: <redacted> }")
    }
}

// Keep the existing uuid dependency features; the persisted identifier uses canonical text.
mod uuid_text {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &uuid::Uuid, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<uuid::Uuid, D::Error> {
        uuid::Uuid::parse_str(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
