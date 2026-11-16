//! A DB authority digest is minted by its locked storage owner, not by a fake route revision.
use super::{invalid, valid_digest, GrantMaterial, GrantRequest};
use crate::console::PersistenceError;

impl GrantRequest {
    pub(crate) fn from_database_snapshot(
        provider: &str,
        credential: &str,
        authority: &str,
        material: GrantMaterial,
    ) -> Result<Self, PersistenceError> {
        if !valid_digest(authority)
            || [provider, credential]
                .iter()
                .any(|id| id.is_empty() || id.len() > 512)
        {
            return Err(invalid());
        }
        material.validate()?;
        Ok(Self {
            provider: provider.into(),
            credential: credential.into(),
            revision: authority.into(),
            material,
        })
    }
}
