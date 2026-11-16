//! Crate-only restoration from authenticated local storage, never from management/public input.
use super::{BusinessError, BusinessRequests, SessionClaim};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u8,
    scope: [u8; 32],
    attempt: String,
    model_key: String,
}
fn valid_model(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}
impl BusinessRequests<'_> {
    pub(crate) fn claim_receipt(&self, claim: &SessionClaim) -> Result<Vec<u8>, BusinessError> {
        self.check_claim(claim)?;
        serde_json::to_vec(&Receipt {
            version: 1,
            scope: claim.scope,
            attempt: claim.attempt.clone(),
            model_key: self.selected.key.into(),
        })
        .map_err(|_| BusinessError::InvalidInput)
    }
}
impl super::RecoveryRequests<'_> {
    pub(crate) fn stored_claim_scope(&self) -> StoredClaimScope {
        StoredClaimScope(self.authority.binding.scope)
    }
    pub(crate) fn restore_claim(
        &self,
        bytes: &[u8],
    ) -> Result<(SessionClaim, String), BusinessError> {
        self.stored_claim_scope().restore_claim(bytes)
    }
}
/// Local sealed readers only. No device/network capability, purchase or active-session authority.
pub(crate) struct StoredClaimScope([u8; 32]);
impl super::CatalogSession {
    pub(crate) fn stored_claim_scope(&self) -> StoredClaimScope {
        StoredClaimScope(self.binding.scope)
    }
}
impl StoredClaimScope {
    pub(crate) fn restore_claim(
        &self,
        bytes: &[u8],
    ) -> Result<(SessionClaim, String), BusinessError> {
        if bytes.len() > 4096 {
            return Err(BusinessError::InvalidResponse);
        }
        let receipt: Receipt =
            serde_json::from_slice(bytes).map_err(|_| BusinessError::InvalidResponse)?;
        let id =
            uuid::Uuid::parse_str(&receipt.attempt).map_err(|_| BusinessError::InvalidResponse)?;
        if receipt.version != 1
            || id.get_version_num() != 4
            || id.to_string() != receipt.attempt
            || !valid_model(&receipt.model_key)
        {
            return Err(BusinessError::InvalidResponse);
        }
        if receipt.scope != self.0 {
            return Err(super::CatalogError::ScopeMismatch.into());
        }
        Ok((
            SessionClaim {
                scope: receipt.scope,
                instance: format!("cli:{}", receipt.attempt),
                attempt: receipt.attempt,
            },
            receipt.model_key,
        ))
    }
}
