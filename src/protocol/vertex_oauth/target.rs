//! Full Vertex target metadata is explicit; it is not an Express Key or an IAM assertion.
use super::OAuthError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexTarget {
    project: String,
    location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quota_project: Option<String>,
}
impl VertexTarget {
    pub fn new(
        project: &str,
        location: &str,
        quota_project: Option<&str>,
    ) -> Result<Self, OAuthError> {
        let target = Self {
            project: project.into(),
            location: location.into(),
            quota_project: quota_project.map(str::to_owned),
        };
        target.validate()?;
        Ok(target)
    }
    pub fn validate(&self) -> Result<(), OAuthError> {
        if !resource_segment(&self.project)
            || self
                .quota_project
                .as_deref()
                .is_some_and(|value| !resource_segment(value))
            || !location_label(&self.location)
        {
            return Err(OAuthError::InvalidInput);
        }
        Ok(())
    }
    pub fn project(&self) -> &str {
        &self.project
    }
    pub fn location(&self) -> &str {
        &self.location
    }
    pub fn quota_project(&self) -> Option<&str> {
        self.quota_project.as_deref()
    }
    pub fn base_url(&self) -> Result<String, OAuthError> {
        self.validate()?;
        let host = match self.location.as_str() {
            "global" => "aiplatform.googleapis.com".into(),
            // Google's multi-region locations have explicit .rep. endpoints, not regional aliases.
            "us" | "eu" => format!("aiplatform.{}.rep.googleapis.com", self.location),
            _ => format!("{}-aiplatform.googleapis.com", self.location),
        };
        Ok(format!(
            "https://{host}/v1/projects/{}/locations/{}/publishers/google",
            self.project, self.location
        ))
    }
}

// Lexical path/header safety, not a guessed project-ID allowlist or an existence/IAM check.
// Numeric project numbers and legacy domain-scoped IDs remain representable.
fn resource_segment(value: &str) -> bool {
    value.len() <= 256
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}
fn location_label(value: &str) -> bool {
    // The location plus "-aiplatform" must fit one 63-byte DNS label. No frozen region list.
    value.len() <= 52
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
