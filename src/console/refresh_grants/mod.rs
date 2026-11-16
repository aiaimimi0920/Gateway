//! Persistent consumption evidence, separate from configuration transaction admission proofs.
//! Sending has no expiry: losing an owner never proves a one-time grant was not consumed.
mod database;
mod locator;
mod luma;
#[cfg(test)]
mod luma_material_tests;
#[cfg(test)]
mod luma_receipt_binding_tests;
#[cfg(test)]
mod luma_receipt_storage_tests;
#[cfg(test)]
mod luma_receipt_tests;
#[cfg(test)]
mod luma_storage_tests;
#[cfg(test)]
mod luma_tests;
mod material;
mod receipt;
mod record;

use crate::console::{PersistenceError, RouteConfigRedisRevision};
use crate::protocol::grok_oauth::AuthorizationContext;
pub use locator::GrantLocator;
pub(crate) use material::GrantMaterial;
pub use receipt::GrantReceipt;
#[cfg(test)]
mod receipt_tests;
pub use record::{GrantPhase, GrantRecord, GrantReservation, GrantResult};

pub(crate) const MAX_GRANT_BYTES: usize = 524_288;

#[derive(Clone, PartialEq, Eq)]
pub struct GrantRequest {
    pub(crate) provider: String,
    pub(crate) credential: String,
    pub(crate) revision: String,
    pub(crate) material: GrantMaterial,
}

impl GrantRequest {
    pub(crate) fn result_with_lobster(
        &self,
        tokens: &crate::protocol::lobsterai::SessionMaterial,
    ) -> Result<GrantResult, PersistenceError> {
        let mut material = self.material.clone();
        material.access = tokens.access_token.clone();
        material.refresh = tokens.refresh_token.clone();
        material.lobsterai = Some(tokens.context.clone());
        material.expires_at = None;
        let result = GrantResult {
            material,
            expires_in: None,
            expires_at: None,
        };
        result.validate(&self.material)?;
        Ok(result)
    }

    /// Derive identity from validated authority, never from mutable provider ID suffixes.
    pub fn from_revision(
        revision: &RouteConfigRedisRevision,
        provider: &str,
        credential: &str,
    ) -> Result<Self, PersistenceError> {
        let config = revision
            .document()
            .providers
            .iter()
            .find(|p| p.id == provider)
            .ok_or_else(invalid)?;
        let credential_config = config
            .credentials
            .iter()
            .enumerate()
            .find(|(index, c)| {
                crate::routing::config::effective_credential_id(provider, *index, c) == credential
            })
            .map(|(_, c)| c)
            .ok_or_else(invalid)?;
        if credential_config
            .nous_portal
            .as_ref()
            .is_some_and(|context| {
                credential_config
                    .base_url
                    .as_deref()
                    .unwrap_or(&config.base_url)
                    .trim_end_matches('/')
                    != context.inference_target().base_url()
            })
        {
            return Err(invalid());
        }
        Ok(Self {
            provider: provider.into(),
            credential: credential.into(),
            revision: revision.metadata().id().into(),
            material: GrantMaterial::from_credential(credential_config, &config.api_key)?,
        })
    }

    pub fn family_id(&self) -> String {
        self.material.family_id()
    }
    pub fn material_id(&self) -> String {
        self.material.material_id()
    }

    pub fn result(
        &self,
        access: &str,
        refresh: Option<&str>,
        lifetime: u64,
        expiry: &str,
        context: Option<AuthorizationContext>,
    ) -> Result<GrantResult, PersistenceError> {
        self.result_with_accio(access, refresh, lifetime, expiry, context, None)
    }

    pub(crate) fn result_with_accio(
        &self,
        access: &str,
        refresh: Option<&str>,
        lifetime: u64,
        expiry: &str,
        context: Option<AuthorizationContext>,
        sgcookie: Option<String>,
    ) -> Result<GrantResult, PersistenceError> {
        let mut next = self.material.clone();
        next.access = access.into();
        next.expires_at = Some(expiry.into());
        if let Some(refresh) = refresh {
            next.refresh = refresh.into();
        }
        if let Some(context) = context {
            next.context = Some(context);
        }
        if let Some(cookie) = sgcookie {
            next.accio_sgcookie = Some(cookie);
        }
        let result = GrantResult {
            material: next,
            expires_in: Some(lifetime),
            expires_at: Some(expiry.into()),
        };
        result.validate(&self.material)?;
        Ok(result)
    }

    pub(crate) fn matches(&self, revision: &RouteConfigRedisRevision) -> bool {
        Self::from_revision(revision, &self.provider, &self.credential).is_ok_and(|r| r == *self)
    }

    pub(crate) fn result_with_vertex(
        &self,
        access: &str,
        refresh: Option<&str>,
        lifetime: u64,
        expiry: &str,
        context: Option<AuthorizationContext>,
        sgcookie: Option<String>,
        vertex: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
    ) -> Result<GrantResult, PersistenceError> {
        let mut result =
            self.result_with_accio(access, refresh, lifetime, expiry, context, sgcookie)?;
        if let Some(context) = vertex {
            result.material.vertex_oauth = Some(context);
        }
        result.validate(&self.material)?;
        Ok(result)
    }

    pub(crate) fn result_with_dim(
        &self,
        access: &str,
        refresh: Option<&str>,
        lifetime: u64,
        expiry: &str,
        context: Option<AuthorizationContext>,
        sgcookie: Option<String>,
        vertex: Option<crate::protocol::vertex_oauth::AuthorizationContext>,
        dim: Option<crate::protocol::dim_oauth::AuthorizationContext>,
    ) -> Result<GrantResult, PersistenceError> {
        let mut result =
            self.result_with_vertex(access, refresh, lifetime, expiry, context, sgcookie, vertex)?;
        if let Some(context) = dim {
            result.material.dim_oauth = Some(context);
        }
        result.validate(&self.material)?;
        Ok(result)
    }

    pub(crate) fn can_follow(&self, prior: &GrantRecord) -> bool {
        prior.phase() == GrantPhase::Published
            && prior
                .result()
                .is_some_and(|r| r.material.equivalent(&self.material))
    }

    pub(crate) fn result_with_nous(
        &self,
        access: &str,
        refresh: &str,
        lifetime: u64,
        expiry: &str,
        context: crate::protocol::nous_portal::AuthorizationContext,
    ) -> Result<GrantResult, PersistenceError> {
        let mut next = self.material.clone();
        next.access = access.into();
        next.refresh = refresh.into();
        next.expires_at = Some(expiry.into());
        next.nous_portal = Some(context);
        let result = GrantResult {
            material: next,
            expires_in: Some(lifetime),
            expires_at: Some(expiry.into()),
        };
        result.validate(&self.material)?;
        Ok(result)
    }
}

impl std::fmt::Debug for GrantRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrantRequest { material: <redacted> }")
    }
}

pub(crate) fn invalid() -> PersistenceError {
    PersistenceError::recovery_required(
        "OAuth refresh grant evidence is invalid or conflicts with authority",
    )
}

pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// Bootstrap may recreate timestamp/message metadata under the same content-derived revision ID.
/// Grant admission binds the complete document; storage still CASes the actual immutable raw bytes.
pub(crate) fn same_authority_document(
    left: &RouteConfigRedisRevision,
    right: &RouteConfigRedisRevision,
) -> Result<bool, PersistenceError> {
    Ok(left.metadata().id() == right.metadata().id()
        && left.metadata().document_digest() == right.metadata().document_digest()
        && left.metadata().yaml_digest() == right.metadata().yaml_digest()
        && serde_json::to_value(left.document()).map_err(|_| invalid())?
            == serde_json::to_value(right.document()).map_err(|_| invalid())?)
}
