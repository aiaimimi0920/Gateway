//! Credential-owned device receipt. Private metadata is stored, not sent as an invented business header.
use super::{atom, LoginError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fingerprint_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fingerprint_hash: Option<String>,
}
impl SessionContext {
    pub fn fingerprint_id(&self) -> Option<&str> {
        self.fingerprint_id.as_deref()
    }
    pub fn fingerprint_hash(&self) -> Option<&str> {
        self.fingerprint_hash.as_deref()
    }
    pub(crate) fn take_secrets(&mut self) -> [Option<String>; 2] {
        [self.fingerprint_id.take(), self.fingerprint_hash.take()]
    }
    pub fn validate(&self) -> Result<(), LoginError> {
        match (self.fingerprint_id(), self.fingerprint_hash()) {
            (None, None) => Ok(()), // Public redaction and an explicitly cleared context.
            (Some(id), Some(hash)) if atom(id, 512) && atom(hash, 4096) => Ok(()),
            _ => Err(LoginError::InvalidInput),
        }
    }
}
impl std::fmt::Debug for SessionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffSessionContext { material: <redacted> }")
    }
}
