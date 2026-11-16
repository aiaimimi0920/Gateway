//! The two subscriptions are separate authorities; email or a matching model never merges them.
use super::OAuthError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Site {
    China,
    Global,
}
impl Site {
    pub fn issuer(self) -> &'static str {
        match self {
            Self::China => "https://dimagent.cn",
            Self::Global => "https://dimagent.com",
        }
    }
}

/// An explicit public client registration, not a borrowed DimCode or plugin client identity.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthClient {
    pub(super) site: Site,
    pub(super) client_id: String,
    pub(super) scopes: Vec<String>,
}
impl OAuthClient {
    pub fn new(site: Site, client_id: &str, scopes: &[&str]) -> Result<Self, OAuthError> {
        let mut seen = std::collections::HashSet::new();
        if !atom(client_id, 512)
            || scopes.is_empty()
            || scopes.len() > 32
            || !scopes.contains(&"openid")
            || scopes.iter().any(|scope| {
                !atom(scope, 128)
                    || scope.bytes().any(|b| matches!(b, b'"' | b'\\'))
                    || !seen.insert(*scope)
            })
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            site,
            client_id: client_id.into(),
            scopes: scopes.iter().map(|s| (*s).into()).collect(),
        })
    }
    pub fn site(&self) -> Site {
        self.site
    }
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }
}
pub(super) fn atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
impl std::fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DimOAuthClient { explicit_registration: true }")
    }
}
