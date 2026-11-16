//! Receipt storage preserves consumed successors without granting Result/replay/publication authority.
use super::{invalid, GrantMaterial, GrantResult};
use crate::console::PersistenceError;
use crate::protocol::nous_portal::{OAuthClient, ReceivedTokens, TOKEN_ENDPOINT};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum GrantReceipt {
    LumaOauth {
        tokens: crate::protocol::luma_oauth::ReceivedRefresh,
    },
    NousPortal {
        tokens: ReceivedTokens,
    },
    LobsterAi {
        tokens: crate::protocol::lobsterai::ReceivedTokens,
    },
}
impl GrantReceipt {
    pub(crate) fn validate(&self, original: &GrantMaterial) -> Result<(), PersistenceError> {
        match self {
            Self::LumaOauth { tokens } => {
                original.validate()?;
                let context = original.luma_oauth.as_ref().ok_or_else(invalid)?;
                tokens
                    .validate_binding(context, &original.refresh)
                    .map_err(|_| invalid())
            }
            Self::LobsterAi { tokens } => {
                if original.lobsterai.is_none() {
                    return Err(invalid());
                }
                original.validate()?;
                tokens.validate_storage().map_err(|_| invalid())
            }
            Self::NousPortal { tokens } => {
                if original.endpoint != TOKEN_ENDPOINT
                    || OAuthClient::new(&original.client).is_err()
                {
                    return Err(invalid());
                }
                tokens.validate_storage().map_err(|_| invalid())
            }
        }
    }
    pub(crate) fn validates_result(
        &self,
        original: &GrantMaterial,
        result: &GrantResult,
    ) -> Result<(), PersistenceError> {
        self.validate(original)?;
        match self {
            Self::LumaOauth { tokens } => {
                if super::luma::result(original, tokens)? != *result {
                    return Err(invalid());
                }
                Ok(())
            }
            Self::LobsterAi { tokens } => {
                let context = original.lobsterai.as_ref().ok_or_else(invalid)?;
                let admitted = tokens
                    .interpret_successor(context, &original.access, &original.refresh)
                    .map_err(|_| invalid())?;
                if result.expires_in.is_some()
                    || result.expires_at.is_some()
                    || result.material.access != admitted.access_token
                    || result.material.refresh != admitted.refresh_token
                    || result.material.lobsterai.as_ref() != Some(&admitted.context)
                {
                    return Err(invalid());
                }
                Ok(())
            }
            Self::NousPortal { tokens } => {
                let client = OAuthClient::new(&original.client).map_err(|_| invalid())?;
                let admitted = tokens
                    .interpret_successor(
                        &client,
                        &original.access,
                        &original.refresh,
                        original.nous_portal.as_ref(),
                    )
                    .map_err(|_| invalid())?;
                let expiry = time::OffsetDateTime::parse(
                    result.expires_at.as_deref().ok_or_else(invalid)?,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(|_| invalid())?;
                if result.material.access != admitted.access_token
                    || result.material.refresh != admitted.refresh_token
                    || time::OffsetDateTime::from_unix_timestamp(admitted.expires_at_unix as i64)
                        .ok()
                        != Some(expiry)
                    || result.expires_in.ok_or_else(invalid)? > admitted.expires_in
                    || original.nous_portal.is_some()
                        && result.material.nous_portal.as_ref() != Some(&admitted.context)
                {
                    return Err(invalid());
                }
                Ok(())
            }
        }
    }
}
impl std::fmt::Debug for GrantReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GrantReceipt { material: <redacted>, publishable: false }")
    }
}
