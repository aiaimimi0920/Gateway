//! Provider source-profile input validation and source classification.

use crate::db;
use crate::error::GatewayError;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSourceProfileBody {
    pub source_kind: String,
    #[serde(default)]
    pub aggregator_api_mode: Option<String>,
    #[serde(default)]
    pub web_reverse_access_mode: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSourceProfilePatchBody {
    pub source_profile: ProviderSourceProfileBody,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSourceProfileBackfillBody {
    #[serde(default)]
    pub provider_account_ids: Option<Vec<String>>,
    #[serde(default)]
    pub only_missing: Option<bool>,
}

pub(super) struct NormalizedSourceProfileInput {
    pub(super) source_kind: String,
    pub(super) aggregator_api_mode: Option<String>,
    pub(super) web_reverse_access_mode: Option<String>,
    pub(super) notes: Option<String>,
}

pub(super) fn normalize_explicit_source_profile(
    input: ProviderSourceProfileBody,
) -> Result<NormalizedSourceProfileInput, GatewayError> {
    let source_kind = input.source_kind.trim().to_lowercase();
    if !matches!(
        source_kind.as_str(),
        "official_model_api" | "official_vendor_api" | "aggregator_api" | "web_reverse_api"
    ) {
        return Err(GatewayError::bad_request(
            "provider sourceProfile.sourceKind 不合法。",
        ));
    }

    let aggregator_api_mode = input
        .aggregator_api_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);
    if aggregator_api_mode
        .as_deref()
        .is_some_and(|value| !matches!(value, "hosted_compute" | "upstream_forward"))
    {
        return Err(GatewayError::bad_request(
            "provider sourceProfile.aggregatorApiMode 不合法。",
        ));
    }

    let web_reverse_access_mode = input
        .web_reverse_access_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase);
    if web_reverse_access_mode
        .as_deref()
        .is_some_and(|value| !matches!(value, "direct_http_replay" | "browser_challenge"))
    {
        return Err(GatewayError::bad_request(
            "provider sourceProfile.webReverseAccessMode 不合法。",
        ));
    }

    if source_kind != "aggregator_api" && aggregator_api_mode.is_some() {
        return Err(GatewayError::bad_request(
            "只有 aggregator_api 允许设置 aggregatorApiMode。",
        ));
    }
    if source_kind != "web_reverse_api" && web_reverse_access_mode.is_some() {
        return Err(GatewayError::bad_request(
            "只有 web_reverse_api 允许设置 webReverseAccessMode。",
        ));
    }

    let notes = input
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(500).collect::<String>());

    Ok(NormalizedSourceProfileInput {
        source_kind,
        aggregator_api_mode,
        web_reverse_access_mode,
        notes,
    })
}

pub(super) fn infer_source_profile(
    existing: &db::GatewayProviderAccountView,
) -> NormalizedSourceProfileInput {
    if has_chatgpt_codex_backend_base_url(&existing.payload) {
        return NormalizedSourceProfileInput {
            source_kind: "official_vendor_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            notes: Some("自动推导：ChatGPT site Codex backend special case".to_string()),
        };
    }

    let hostname = read_provider_hostname(&existing.payload);
    if adapter_belongs_to_web_reverse_source(existing.adapter.as_str()) {
        return NormalizedSourceProfileInput {
            source_kind: "web_reverse_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: Some(resolve_source_access_mode_from_execution(
                existing.execution_mode,
                existing.endpoint_execution_modes.as_ref(),
            )),
            notes: Some(format!("自动推导：{}", existing.adapter)),
        };
    }

    if matches!(
        existing.adapter.as_str(),
        "search_api_compatible"
            | "linkup_compatible"
            | "kiro_compatible"
            | "codex_cli"
            | "claude_code"
    ) {
        return NormalizedSourceProfileInput {
            source_kind: "official_vendor_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            notes: Some(format!("自动推导：{}", existing.adapter)),
        };
    }

    if hostname
        .as_deref()
        .is_some_and(is_known_official_vendor_host)
    {
        let host = hostname.unwrap_or_default();
        return NormalizedSourceProfileInput {
            source_kind: "official_vendor_api".to_string(),
            aggregator_api_mode: None,
            web_reverse_access_mode: None,
            notes: Some(format!("自动推导：official host {host}")),
        };
    }

    if hostname
        .as_deref()
        .is_some_and(is_known_hosted_aggregator_host)
    {
        let host = hostname.unwrap_or_default();
        return NormalizedSourceProfileInput {
            source_kind: "aggregator_api".to_string(),
            aggregator_api_mode: Some("hosted_compute".to_string()),
            web_reverse_access_mode: None,
            notes: Some(format!("自动推导：hosted aggregator {host}")),
        };
    }

    NormalizedSourceProfileInput {
        source_kind: "aggregator_api".to_string(),
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        notes: Some(match hostname {
            Some(host) => format!("自动推导：compatible upstream {host}"),
            None => "自动推导：generic compatible provider".to_string(),
        }),
    }
}

fn read_provider_hostname(payload: &Value) -> Option<String> {
    let base_url = read_provider_base_url(payload)?;
    let without_scheme = base_url
        .split("://")
        .nth(1)
        .unwrap_or(base_url)
        .split('/')
        .next()
        .unwrap_or(base_url)
        .trim();
    let host = without_scheme
        .split('@')
        .next_back()
        .unwrap_or(without_scheme)
        .split(':')
        .next()
        .unwrap_or(without_scheme)
        .trim()
        .to_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(host)
}

fn read_provider_base_url(payload: &Value) -> Option<&str> {
    let obj = payload.as_object()?;
    ["baseUrl", "base_url", "endpoint", "url"]
        .iter()
        .find_map(|key| obj.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn adapter_belongs_to_web_reverse_source(adapter: &str) -> bool {
    matches!(
        adapter,
        "accio_compatible"
            | "chatgpt_web_reverse_compatible"
            | "producer_compatible"
            | "gemini_business_compatible"
            | "chataibot_compatible"
            | "lumalabs_compatible"
            | "gemini_canvas_compatible"
            | "suno_compatible"
            | "udio_compatible"
    )
}

fn has_chatgpt_codex_backend_base_url(payload: &Value) -> bool {
    read_provider_base_url(payload)
        .is_some_and(crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_base_url)
}

fn resolve_source_access_mode_from_execution(
    execution_mode: crate::routing::candidate::ProviderExecutionMode,
    endpoint_execution_modes: Option<
        &HashMap<String, crate::routing::candidate::ProviderExecutionMode>,
    >,
) -> String {
    let has_browser_backed_endpoint = execution_mode
        == crate::routing::candidate::ProviderExecutionMode::BrowserBacked
        || endpoint_execution_modes
            .map(|modes| {
                modes.values().any(|mode| {
                    *mode == crate::routing::candidate::ProviderExecutionMode::BrowserBacked
                })
            })
            .unwrap_or(false);
    if has_browser_backed_endpoint {
        "browser_challenge".to_string()
    } else {
        "direct_http_replay".to_string()
    }
}

fn is_known_hosted_aggregator_host(hostname: &str) -> bool {
    hostname == "api.siliconflow.cn" || hostname == "ai.gitee.com"
}

fn is_known_official_vendor_host(hostname: &str) -> bool {
    [
        "openai.com",
        "anthropic.com",
        "x.ai",
        "groq.com",
        "googleapis.com",
        "generativelanguage.googleapis.com",
        "nvidia.com",
        "mistral.ai",
        "cohere.ai",
        "moonshot.cn",
        "zhipuai.cn",
        "linkup.so",
        "tavily.com",
        "exa.ai",
        "ydc-index.io",
        "websearchapi.ai",
        "jina.ai",
    ]
    .iter()
    .any(|suffix| hostname == *suffix || hostname.ends_with(&format!(".{suffix}")))
}
