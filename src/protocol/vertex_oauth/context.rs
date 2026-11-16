//! Explicit authorization snapshot; token material remains in the existing credential secret owner.
use super::{
    valid_atom, OAuthClient, OAuthError, RefreshGrant, TokenMaterial, VertexTarget,
    CLOUD_PLATFORM_SCOPE,
};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantType {
    DesktopAuthorizationCode,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationContext {
    grant_type: GrantType,
    client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    requested_scope: String,
    scope_reported: bool,
    target: VertexTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_expires_at: Option<String>,
}
impl AuthorizationContext {
    pub fn from_authorization(
        client: OAuthClient,
        target: VertexTarget,
        material: &TokenMaterial,
    ) -> Result<Self, OAuthError> {
        // The session owner must pass its captured client/target, never infer them from the token.
        if material.refresh_token().is_none() {
            return Err(OAuthError::MissingRefreshToken);
        }
        let context = Self {
            grant_type: GrantType::DesktopAuthorizationCode,
            client_id: client.client_id,
            client_secret: client.client_secret,
            requested_scope: CLOUD_PLATFORM_SCOPE.into(),
            scope_reported: material.reported_scope().is_some(),
            target,
            refresh_expires_at: material
                .refresh_expires_at()
                .map(|value| value.format(&Rfc3339))
                .transpose()
                .map_err(|_| OAuthError::InvalidInput)?,
        };
        context.validate()?;
        Ok(context)
    }
    fn checked(&self) -> Result<Option<OffsetDateTime>, OAuthError> {
        if !valid_atom(&self.client_id, 1024)
            || self
                .client_secret
                .as_deref()
                .is_some_and(|secret| !valid_atom(secret, 4096))
            || self.requested_scope != CLOUD_PLATFORM_SCOPE
        {
            return Err(OAuthError::InvalidInput);
        }
        self.target.validate()?;
        self.refresh_expires_at()
    }
    pub fn validate(&self) -> Result<(), OAuthError> {
        self.checked().map(|_| ())
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn client_secret(&self) -> Option<&str> {
        self.client_secret.as_deref()
    }
    /// Only the existing secret owner removes private client material from management views.
    pub(crate) fn take_client_secret(&mut self) -> Option<String> {
        self.client_secret.take()
    }
    pub fn target(&self) -> &VertexTarget {
        &self.target
    }
    pub fn scope_reported(&self) -> bool {
        self.scope_reported
    }
    pub fn refresh_expires_at(&self) -> Result<Option<OffsetDateTime>, OAuthError> {
        self.refresh_expires_at
            .as_deref()
            .map(|value| {
                if value.len() > 64 {
                    return Err(OAuthError::InvalidInput);
                }
                let expiry =
                    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| OAuthError::InvalidInput)?;
                if expiry.unix_timestamp() <= 0 {
                    return Err(OAuthError::InvalidInput);
                }
                // Expired snapshots remain readable; RefreshGrant is the live-send boundary.
                Ok(expiry)
            })
            .transpose()
    }
    pub fn refresh_grant(&self, token: &str) -> Result<RefreshGrant, OAuthError> {
        let expiry = self.checked()?;
        let client = OAuthClient {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
        };
        RefreshGrant::new(client, token, expiry)
    }
    pub fn rotated(&self, material: &TokenMaterial) -> Result<Self, OAuthError> {
        let old = self.checked()?;
        let expiry = match (old, material.refresh_expires_at()) {
            (Some(old), Some(new)) => Some(old.min(new)),
            (old, new) => old.or(new),
        };
        let mut next = self.clone();
        next.scope_reported |= material.reported_scope().is_some();
        next.refresh_expires_at = expiry
            .map(|value| value.format(&Rfc3339))
            .transpose()
            .map_err(|_| OAuthError::InvalidInput)?;
        Ok(next)
    }
    pub fn permits_rotation(&self, next: &Self) -> bool {
        let (Ok(old), Ok(new)) = (self.checked(), next.checked()) else {
            return false;
        };
        self.grant_type == next.grant_type
            && self.client_id == next.client_id
            && self.client_secret == next.client_secret
            && self.requested_scope == next.requested_scope
            && self.target == next.target
            && (!self.scope_reported || next.scope_reported)
            && match (old, new) {
                (None, _) => true,
                (Some(old), Some(new)) => new <= old,
                (Some(_), None) => false,
            }
    }
}
impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VertexAuthorizationContext { explicit_grant: true, material: <redacted> }")
    }
}
