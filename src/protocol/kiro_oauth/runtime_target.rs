//! Explicit compatibility inference destination, separate from the registration's directory Region.
use super::OAuthError;
use crate::routing::candidate::ProviderAccountPayload;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTarget {
    base_url: String,
    profile_arn: String,
}
impl RuntimeTarget {
    pub fn new(base: &str, profile: &str) -> Result<Self, OAuthError> {
        let value = Self {
            base_url: base.trim_end_matches('/').into(),
            profile_arn: profile.into(),
        };
        value.validate()?;
        if base != value.base_url && base != format!("{}/", value.base_url) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(value)
    }
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
    pub fn profile_arn(&self) -> &str {
        &self.profile_arn
    }
    pub(crate) fn validate(&self) -> Result<(), OAuthError> {
        let parts: Vec<_> = self.profile_arn.splitn(7, ':').collect();
        if self.profile_arn.len() > 2048
            || parts.len() != 6
            || parts[0] != "arn"
            || parts[1] != "aws"
            || parts[2] != "codewhisperer"
            || !matches!(parts[3], "us-east-1" | "eu-central-1")
            || parts[4].len() != 12
            || !parts[4].bytes().all(|b| b.is_ascii_digit())
            || parts[5].strip_prefix("profile/").is_none_or(|id| {
                id.is_empty()
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            })
            || self.base_url != format!("https://runtime.{}.kiro.dev", parts[3])
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(())
    }
    pub(crate) fn from_payload(payload: &ProviderAccountPayload) -> Result<Self, OAuthError> {
        if !super::runtime::owns_payload(payload) {
            return Err(OAuthError::InvalidInput);
        }
        let profile = payload
            .extra_body
            .as_ref()
            .and_then(|body| body.get("profileArn"))
            .and_then(serde_json::Value::as_str)
            .ok_or(OAuthError::InvalidInput)?;
        Self::new(&payload.base_url, profile)
    }
}
impl std::fmt::Debug for RuntimeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KiroRuntimeTarget { destination: <redacted> }")
    }
}
