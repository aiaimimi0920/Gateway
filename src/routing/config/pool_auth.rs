//! Provider-local eligibility, not an OAuth authenticity claim or a global fallback policy.
use super::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialPoolAuthPolicy {
    #[default]
    Any,
    #[serde(rename = "oauth_only")]
    OAuthOnly,
}

impl CredentialPoolAuthPolicy {
    pub(super) fn is_any(&self) -> bool {
        *self == Self::Any
    }

    pub(super) fn allows(self, credential: &CompiledCredential) -> bool {
        self.is_any()
            || (!credential.discovery_required
                && credential
                    .refresh_config
                    .as_ref()
                    .is_some_and(|owner| owner.has_active_refresh_material()))
    }
}
