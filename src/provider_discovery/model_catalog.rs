//! Public catalog metadata, deliberately separate from protocol/generation evidence.
use crate::error::GatewayError;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Duration};

pub const NVIDIA_BASE: &str = "https://integrate.api.nvidia.com";
pub const OPENAI_SOURCE: &str = "https://api.openai.com/v1/models";
pub const ANTHROPIC_SOURCE: &str = "https://api.anthropic.com/v1/models?limit=1000";
pub const GEMINI_SOURCE: &str =
    "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000";
pub const NVIDIA_SOURCE: &str = "https://integrate.api.nvidia.com/v1/models";
pub const OPENROUTER_SOURCE: &str = "https://openrouter.ai/api/v1/models";
pub const POE_SOURCE: &str = "https://api.poe.com/v1/models";
pub const GROQ_SOURCE: &str = "https://api.groq.com/openai/v1/models";
pub const COHERE_SOURCE: &str = "https://api.cohere.com/v1/models?endpoint=chat&page_size=1000";
const LIMIT: usize = 2 * 1024 * 1024;
mod anthropic;
mod cohere;
mod gemini;

#[derive(Clone, Copy)]
pub enum CatalogProvider {
    Gemini,
    Anthropic,
    OpenAi,
    Nvidia,
    OpenRouter,
    Poe,
    Groq,
    Cohere,
}

impl CatalogProvider {
    pub fn source(self) -> &'static str {
        match self {
            Self::Gemini => GEMINI_SOURCE,
            Self::Anthropic => ANTHROPIC_SOURCE,
            Self::OpenAi => OPENAI_SOURCE,
            Self::Nvidia => NVIDIA_SOURCE,
            Self::OpenRouter => OPENROUTER_SOURCE,
            Self::Poe => POE_SOURCE,
            Self::Groq => GROQ_SOURCE,
            Self::Cohere => COHERE_SOURCE,
        }
    }

    pub fn accepts(self, preset: Option<&str>, base: &str) -> bool {
        match self {
            Self::Gemini => {
                preset == Some("gemini-api")
                    && base.trim_end_matches('/')
                        == "https://generativelanguage.googleapis.com/v1beta"
            }
            Self::Anthropic => {
                preset == Some("anthropic")
                    && base.trim_end_matches('/') == "https://api.anthropic.com"
            }
            Self::OpenAi => {
                preset == Some("openai")
                    && base.trim_end_matches('/') == "https://api.openai.com/v1"
            }
            Self::Cohere => {
                preset == Some("cohere-chat")
                    && base.trim_end_matches('/') == "https://api.cohere.com"
            }
            Self::Groq => {
                preset == Some("groq-openai")
                    && base.trim_end_matches('/') == "https://api.groq.com/openai/v1"
            }
            Self::Poe => {
                preset == Some("poe-openai")
                    && matches!(
                        base.trim_end_matches('/'),
                        "https://api.poe.com" | "https://api.poe.com/v1"
                    )
            }
            Self::Nvidia => preset == Some("nvidia-openai") && official_nvidia_base(base),
            Self::OpenRouter => {
                matches!(preset, Some("openrouter-openai" | "openai"))
                    && matches!(
                        base.trim_end_matches('/'),
                        "https://openrouter.ai"
                            | "https://openrouter.ai/api"
                            | "https://openrouter.ai/api/v1"
                    )
            }
        }
    }

    fn limit(self) -> usize {
        match self {
            Self::Gemini
            | Self::Anthropic
            | Self::OpenAi
            | Self::Nvidia
            | Self::Poe
            | Self::Groq
            | Self::Cohere => LIMIT,
            Self::OpenRouter => 8 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCatalog {
    pub source_url: String,
    pub checked_at: String,
    pub models: Vec<String>,
}

pub fn official_nvidia_base(base: &str) -> bool {
    matches!(
        base.trim_end_matches('/'),
        NVIDIA_BASE | "https://integrate.api.nvidia.com/v1"
    )
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id.trim() == id
        && !id.chars().any(|c| c.is_control() || c == '*' || c == '?')
}

impl ModelCatalog {
    pub fn validate(&self, preset: Option<&str>, base: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            [
                CatalogProvider::Gemini,
                CatalogProvider::Anthropic,
                CatalogProvider::OpenAi,
                CatalogProvider::Nvidia,
                CatalogProvider::OpenRouter,
                CatalogProvider::Poe,
                CatalogProvider::Groq,
                CatalogProvider::Cohere
            ]
            .iter()
            .any(|kind| kind.accepts(preset, base) && self.source_url == kind.source()),
            "Model catalog does not match official account origin."
        );
        anyhow::ensure!(
            time::OffsetDateTime::parse(
                &self.checked_at,
                &time::format_description::well_known::Rfc3339
            )
            .is_ok(),
            "Invalid catalog timestamp."
        );
        let unique: HashSet<_> = self.models.iter().collect();
        anyhow::ensure!(
            !self.models.is_empty()
                && self.models.len() <= 2048
                && unique.len() == self.models.len()
                && self.models.iter().all(|id| valid_id(id)),
            "Invalid model catalog IDs."
        );
        Ok(())
    }

    /// Metadata widens the declared pool, never generation or health evidence.
    pub fn append_models_to(&self, declared: &mut Vec<String>) {
        let mut seen: HashSet<String> = declared.iter().cloned().collect();
        declared.extend(
            self.models
                .iter()
                .filter(|id| seen.insert((*id).clone()))
                .cloned(),
        );
    }
}

pub async fn fetch_catalog(kind: CatalogProvider, key: &str) -> Result<ModelCatalog, GatewayError> {
    let client = crate::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .map_err(|_| unavailable())?;
    fetch_from(&client, kind.source(), key, kind).await
}

fn unavailable() -> GatewayError {
    GatewayError::service_unavailable(
        "Cannot read official model catalog; previous catalog is unchanged.",
    )
}

// Destination is private: production callers can only select fixed official sources.
async fn fetch_from(
    client: &rquest::Client,
    url: &str,
    key: &str,
    kind: CatalogProvider,
) -> Result<ModelCatalog, GatewayError> {
    if matches!(kind, CatalogProvider::Gemini) {
        return gemini::fetch(client, url, key).await;
    }
    if matches!(kind, CatalogProvider::Anthropic) {
        return anthropic::fetch(client, url, key).await;
    }
    if matches!(kind, CatalogProvider::Cohere) {
        return cohere::fetch(client, url, key).await;
    }
    tokio::time::timeout(Duration::from_secs(30), async {
        let response = client
            .get(url)
            .bearer_auth(key)
            .timeout(Duration::from_secs(25))
            .send()
            .await
            .map_err(|_| unavailable())?;
        if !response.status().is_success() {
            return Err(unavailable());
        }
        if response
            .content_length()
            .is_some_and(|size| size > kind.limit() as u64)
        {
            return Err(unavailable());
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| unavailable())?;
            if bytes.len().saturating_add(chunk.len()) > kind.limit() {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        let body: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
        let data = body["data"]
            .as_array()
            .filter(|data| !data.is_empty() && data.len() <= 2048)
            .ok_or_else(unavailable)?;
        let mut seen = HashSet::new();
        let mut models = Vec::new();
        for item in data {
            let id = item["id"]
                .as_str()
                .filter(|id| valid_id(id))
                .ok_or_else(unavailable)?;
            if seen.insert(id) {
                models.push(id.to_owned());
            }
        }
        Ok(ModelCatalog {
            source_url: kind.source().into(),
            models,
            checked_at: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap(),
        })
    })
    .await
    .map_err(|_| unavailable())?
}

#[cfg(test)]
#[path = "anthropic_catalog_tests.rs"]
mod anthropic_tests;
#[cfg(test)]
#[path = "cohere_catalog_tests.rs"]
mod cohere_tests;
#[cfg(test)]
#[path = "gemini_catalog_tests.rs"]
mod gemini_tests;
#[cfg(test)]
#[path = "model_catalog_tests.rs"]
mod tests;
