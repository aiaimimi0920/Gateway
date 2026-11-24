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
pub const RIVER_SOURCE: &str = "https://api.river.ai/v1/models";
pub const COHERE_SOURCE: &str = "https://api.cohere.com/v1/models?endpoint=chat&page_size=1000";
const LIMIT: usize = 2 * 1024 * 1024;
mod anthropic;
mod cloudflare;
mod cohere;
mod commandcode_endpoints;
pub use cloudflare::fetch_catalog as fetch_cloudflare_catalog;
mod gemini;
mod qwen;
pub mod qwen_endpoint;
pub use qwen::fetch_catalog as fetch_qwen_catalog;

#[derive(Clone, Copy)]
pub enum CatalogProvider {
    Gemini,
    Anthropic,
    OpenAi,
    Nvidia,
    OpenRouter,
    Poe,
    Groq,
    River,
    Agnes,
    SenseNova,
    CommandCode,
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
            Self::River => RIVER_SOURCE,
            Self::Agnes => crate::protocol::agnes::SOURCE,
            Self::SenseNova => crate::protocol::sensenova::SOURCE,
            Self::CommandCode => crate::protocol::commandcode::SOURCE,
            Self::Cohere => COHERE_SOURCE,
        }
    }

    pub fn accepts(self, preset: Option<&str>, base: &str) -> bool {
        match self {
            Self::CommandCode => {
                preset == Some(crate::protocol::commandcode::PRESET)
                    && base.trim_end_matches('/') == crate::protocol::commandcode::BASE
            }
            Self::SenseNova => {
                preset == Some(crate::protocol::sensenova::PRESET)
                    && base.trim_end_matches('/') == crate::protocol::sensenova::BASE
            }
            Self::Agnes => {
                preset == Some(crate::protocol::agnes::PRESET)
                    && base.trim_end_matches('/') == crate::protocol::agnes::BASE
            }
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
            Self::River => {
                preset == Some("river-api")
                    && base.trim_end_matches('/') == "https://api.river.ai/v1"
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
            | Self::River
            | Self::SenseNova
            | Self::CommandCode
            | Self::Agnes
            | Self::Cohere => LIMIT,
            Self::OpenRouter => 8 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelCatalog {
    pub source_url: String,
    /// Qwen shared directory origins do not identify the inference workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_base_url: Option<String>,
    pub checked_at: String,
    pub models: Vec<String>,
    /// Declared routes, not entitlement or proof that the runtime supports them.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub supported_endpoints: std::collections::BTreeMap<String, Vec<String>>,
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
        commandcode_endpoints::validate(self, preset)?;
        let qwen = preset == Some("qwen-official-api")
            && qwen_endpoint::QwenEndpoint::parse(base).is_some_and(|endpoint| {
                endpoint.matches_catalog(&self.source_url, self.account_base_url.as_deref())
            });
        let cloudflare = preset == Some(crate::protocol::cloudflare_workers_ai::PRESET)
            && crate::protocol::cloudflare_workers_ai::Endpoint::parse(base).is_some_and(
                |endpoint| {
                    self.source_url == endpoint.source_url()
                        && self.account_base_url.as_deref() == Some(endpoint.base_url.as_str())
                },
            );
        anyhow::ensure!(
            qwen || cloudflare
                || (self.account_base_url.is_none()
                    && [
                        CatalogProvider::Gemini,
                        CatalogProvider::Anthropic,
                        CatalogProvider::OpenAi,
                        CatalogProvider::Nvidia,
                        CatalogProvider::OpenRouter,
                        CatalogProvider::Poe,
                        CatalogProvider::Groq,
                        CatalogProvider::River,
                        CatalogProvider::Agnes,
                        CatalogProvider::SenseNova,
                        CatalogProvider::CommandCode,
                        CatalogProvider::Cohere
                    ]
                    .iter()
                    .any(|kind| kind.accepts(preset, base) && self.source_url == kind.source())),
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
        anyhow::ensure!(
            preset != Some(crate::protocol::sensenova::PRESET)
                || self
                    .models
                    .iter()
                    .all(|id| crate::protocol::sensenova::text_model(id)),
            "SenseNova text catalog cannot include image generation models."
        );
        let unique: HashSet<_> = self.models.iter().collect();
        anyhow::ensure!(
            preset != Some(crate::protocol::agnes::PRESET)
                || self
                    .models
                    .iter()
                    .all(|id| crate::protocol::agnes::text_model(id)),
            "Agnes text catalog cannot include image or video models."
        );
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
    #[cfg(test)]
    if matches!(
        kind,
        CatalogProvider::Agnes | CatalogProvider::SenseNova | CatalogProvider::CommandCode
    ) {
        // 必须显式注入本地 scope，测试漏接时不能访问真实目录。
        let origin = if matches!(kind, CatalogProvider::CommandCode) {
            crate::protocol::commandcode::test_support::test_origin()
        } else if matches!(kind, CatalogProvider::SenseNova) {
            crate::protocol::sensenova::test_support::test_origin()
        } else {
            crate::protocol::agnes::test_support::test_origin()
        }
        .expect("Product catalog requires loopback scope");
        let url = url::Url::parse(&origin).expect("test origin");
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        let client = rquest::Client::builder()
            .no_proxy()
            .redirect(rquest::redirect::Policy::none())
            .build()
            .unwrap();
        let path = url::Url::parse(kind.source()).unwrap().path().to_owned();
        return fetch_from(&client, &format!("{origin}{path}"), key, kind).await;
    }
    let client = crate::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .build()
        .map_err(|_| unavailable())?;
    fetch_from(&client, kind.source(), key, kind).await
}

/// Uses the same parser and official destination; only the isolated devtest client differs.
pub(crate) async fn fetch_sensenova_fixture(
    client: &rquest::Client,
    key: &str,
) -> Result<ModelCatalog, GatewayError> {
    if !crate::upstream::client::sensenova_fixture::synthetic_key(key) {
        return Err(
            GatewayError::bad_request("SenseNova fixture requires a synthetic Key")
                .with_code("sensenova_fixture_key_invalid"),
        );
    }
    let kind = CatalogProvider::SenseNova;
    fetch_from(client, kind.source(), key, kind).await
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
        // Command Code's public directory needs no account identity, including ambient headers.
        let request = client.get(url);
        let request = if matches!(kind, CatalogProvider::CommandCode) {
            request.default_headers(false)
        } else {
            request.bearer_auth(key)
        };
        let response = request
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
        let mut supported_endpoints = std::collections::BTreeMap::new();
        for item in data {
            if matches!(kind, CatalogProvider::CommandCode)
                && !item["supported_endpoints"]
                    .as_array()
                    .is_some_and(|routes| {
                        routes.iter().any(|route| {
                            matches!(
                                route.as_str(),
                                Some(
                                    "/chat/completions" | "/responses" | "/messages" | "/systemone"
                                )
                            )
                        })
                    })
            {
                continue;
            }
            let id = item["id"]
                .as_str()
                .filter(|id| valid_id(id))
                .ok_or_else(unavailable)?;
            if matches!(kind, CatalogProvider::Agnes) && !crate::protocol::agnes::text_model(id) {
                continue;
            }
            if matches!(kind, CatalogProvider::SenseNova)
                && (!crate::protocol::sensenova::text_model(id)
                    || !item["output_modalities"]
                        .as_array()
                        .is_some_and(|m| m.iter().any(|v| v == "text")))
            {
                continue;
            }
            if seen.insert(id) {
                models.push(id.to_owned());
                if matches!(kind, CatalogProvider::CommandCode) {
                    supported_endpoints.insert(id.to_owned(), commandcode_endpoints::read(item)?);
                }
            }
        }
        if models.is_empty() {
            return Err(unavailable());
        }
        Ok(ModelCatalog {
            source_url: kind.source().into(),
            account_base_url: None,
            models,
            supported_endpoints,
            checked_at: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap(),
        })
    })
    .await
    .map_err(|_| unavailable())?
}

#[cfg(test)]
#[path = "agnes_catalog_tests.rs"]
mod agnes_tests;
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
#[path = "river_catalog_tests.rs"]
mod river_tests;
#[cfg(test)]
#[path = "model_catalog_tests.rs"]
mod tests;

#[cfg(all(test, feature = "line-sensenova-token-plan"))]
#[path = "sensenova_catalog_tests.rs"]
mod sensenova_tests;

#[cfg(all(test, feature = "line-commandcode-provider-api"))]
#[path = "commandcode_catalog_tests.rs"]
mod commandcode_tests;
