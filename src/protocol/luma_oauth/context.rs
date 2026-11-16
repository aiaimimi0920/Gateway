//! Non-secret grant metadata; storage integrity and secret consent remain the management owner's job.
use super::{atom, Authorization, OAuthError, TokenMaterial, ISSUER, RESOURCE, TOKEN_ENDPOINT};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Fields", into = "Fields")]
pub struct AuthorizationContext {
    client_id: String,
    redirect_uri: String,
    expires_at: Option<OffsetDateTime>,
    offline_access: bool,
}

// Fixed authority is written explicitly so a reader cannot silently reinterpret old metadata.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
    issuer: String,
    resource: String,
    token_endpoint: String,
    client_id: String,
    redirect_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    offline_access: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl AuthorizationContext {
    pub fn from_material(material: &TokenMaterial) -> Result<Self, OAuthError> {
        let context = Self::from_stored_material(material)?;
        context.require_unexpired()?;
        Ok(context)
    }

    pub(crate) fn from_stored_material(material: &TokenMaterial) -> Result<Self, OAuthError> {
        let mut context = Self::new(
            material.client_id().into(),
            material.redirect_uri().into(),
            material.expires_at(),
        )?;
        context.offline_access = material.offline_access();
        Ok(context)
    }

    pub(crate) fn permits_rotation(&self, next: &Self) -> bool {
        self.offline_access
            && next.offline_access
            && self.client_id == next.client_id
            && self.redirect_uri == next.redirect_uri
    }

    fn new(
        client_id: String,
        redirect_uri: String,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<Self, OAuthError> {
        if !atom(&client_id, 512) {
            return Err(OAuthError::InvalidInput);
        }
        Authorization::callback_port(&redirect_uri)?;
        if let Some(at) = expires_at {
            at.format(&Rfc3339).map_err(|_| OAuthError::InvalidInput)?;
        }
        Ok(Self {
            client_id,
            redirect_uri,
            expires_at,
            offline_access: false,
        })
    }

    /// Recombine trusted stored metadata with its separately admitted secret, never infer JWT identity.
    /// This does not attest that arbitrary caller-supplied metadata belongs to the bearer.
    pub fn restore_material(&self, access: &str) -> Result<TokenMaterial, OAuthError> {
        self.require_unexpired()?;
        Ok(
            TokenMaterial::restore(access, self.expires_at, &self.client_id, &self.redirect_uri)?
                .with_offline_access(self.offline_access),
        )
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn offline_access(&self) -> bool {
        self.offline_access
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }
    pub fn expires_at(&self) -> Option<OffsetDateTime> {
        self.expires_at
    }

    fn require_unexpired(&self) -> Result<(), OAuthError> {
        if self
            .expires_at
            .is_some_and(|at| at <= OffsetDateTime::now_utc())
        {
            return Err(OAuthError::Expired);
        }
        Ok(())
    }
}

impl TryFrom<Fields> for AuthorizationContext {
    type Error = OAuthError;
    fn try_from(fields: Fields) -> Result<Self, Self::Error> {
        if fields.issuer != ISSUER
            || fields.resource != RESOURCE
            || fields.token_endpoint != TOKEN_ENDPOINT
        {
            return Err(OAuthError::InvalidInput);
        }
        let expires_at = fields
            .expires_at
            .map(|value| {
                OffsetDateTime::parse(&value, &Rfc3339).map_err(|_| OAuthError::InvalidInput)
            })
            .transpose()?;
        // Expired metadata stays readable for account management, but cannot restore usable material.
        let mut context = Self::new(fields.client_id, fields.redirect_uri, expires_at)?;
        context.offline_access = fields.offline_access;
        Ok(context)
    }
}

impl From<AuthorizationContext> for Fields {
    fn from(context: AuthorizationContext) -> Self {
        Self {
            issuer: ISSUER.into(),
            resource: RESOURCE.into(),
            token_endpoint: TOKEN_ENDPOINT.into(),
            client_id: context.client_id,
            redirect_uri: context.redirect_uri,
            offline_access: context.offline_access,
            expires_at: context
                .expires_at
                .map(|at| at.format(&Rfc3339).expect("validated timestamp")),
        }
    }
}

impl std::fmt::Debug for AuthorizationContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LumaAuthorizationContext { authority: <redacted>, identity: unverified }")
    }
}
