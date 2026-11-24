//! Declarative provider specification, strict HTTPS origin validation, and UI field descriptors.

use serde::{Deserialize, Serialize};
use url::Url;

/// Authentication credential shape supported by a provider channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAuthKind {
    BearerApiKey,
    CustomHeaderKey { header_name: &'static str },
    OAuthToken,
    ServiceKeyJson,
    BrowserSessionCookie,
}

/// Public or native inference surface supported by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointSurface {
    ChatCompletions,
    Responses,
    AnthropicMessages,
    Embeddings,
    Images,
    Videos,
    AudioSpeech,
    AudioTranscriptions,
    Search,
    DecisionSystemOne,
}

/// Declarative console input field metadata for schema-driven UI rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderUiFieldSpec {
    pub key: &'static str,
    pub label_zh: &'static str,
    pub label_en: &'static str,
    pub secret: bool,
    pub required: bool,
    pub placeholder: &'static str,
}

/// Single cohesive declaration for a provider line, avoiding 10-module shotgun boilerplate.
#[derive(Debug, Clone, Copy)]
pub struct ProviderDescriptor {
    pub line_feature: &'static str,
    pub preset_id: &'static str,
    pub vendor_key: &'static str,
    pub vendor_name: &'static str,
    pub default_base_url: &'static str,
    /// Exact allowed HTTPS origin + base-path prefixes (e.g. `"https://api.example.com/v1"`).
    pub allowed_base_urls: &'static [&'static str],
    pub models_catalog_path: Option<&'static str>,
    pub auth_kind: ProviderAuthKind,
    pub endpoints: &'static [EndpointSurface],
    pub ui_fields: &'static [ProviderUiFieldSpec],
}

impl ProviderDescriptor {
    /// Validate that a candidate base URL strictly uses HTTPS without credentials, query,
    /// fragment, or lookalike host spoofing, and matches one of `allowed_base_urls` when non-empty.
    pub fn validate_base_url(&self, candidate: &str) -> Result<String, String> {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return Err("provider base_url must not be empty".to_string());
        }
        let parsed = Url::parse(trimmed).map_err(|err| format!("invalid base_url: {err}"))?;
        if parsed.scheme() != "https" {
            return Err("provider base_url must use https".to_string());
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err("provider base_url must not embed userinfo credentials".to_string());
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err("provider base_url must not include query or fragment".to_string());
        }
        if parsed.host_str().is_none() {
            return Err("provider base_url must include a valid host".to_string());
        }
        let normalized = trimmed.trim_end_matches('/').to_string();
        if !self.allowed_base_urls.is_empty()
            && !self
                .allowed_base_urls
                .iter()
                .any(|allowed| allowed.trim_end_matches('/') == normalized)
        {
            return Err(format!(
                "base_url '{normalized}' is not an approved origin for preset '{}'",
                self.preset_id
            ));
        }
        Ok(normalized)
    }

    /// Resolve the canonical `/models` catalog endpoint URL if this provider exposes one.
    pub fn resolve_models_url(&self, base_url: &str) -> Result<Option<String>, String> {
        let validated = self.validate_base_url(base_url)?;
        let Some(models_path) = self.models_catalog_path else {
            return Ok(None);
        };
        let suffix = models_path.trim_start_matches('/');
        Ok(Some(format!("{validated}/{suffix}")))
    }

    pub fn supports_endpoint(&self, surface: EndpointSurface) -> bool {
        self.endpoints.contains(&surface)
    }
}

/// Find a descriptor by preset identifier in a static descriptor slice.
pub fn find_by_preset<'a>(
    registry: &'a [ProviderDescriptor],
    preset_id: &str,
) -> Option<&'a ProviderDescriptor> {
    registry.iter().find(|item| item.preset_id == preset_id)
}
