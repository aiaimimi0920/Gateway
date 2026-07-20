use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;
use url::Url;

use super::{ChatGptWebBootstrap, CHATGPT_WEB_DEFAULT_POW_SCRIPT};
use crate::error::GatewayError;

pub fn normalize_site_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return "https://chatgpt.com".to_string();
    }

    if let Ok(mut parsed) = Url::parse(trimmed) {
        let path = parsed.path();
        let should_collapse_to_site_root = path == "/"
            || path.is_empty()
            || path == "/backend-api"
            || path.starts_with("/backend-api/")
            || path.starts_with("/c/");
        if should_collapse_to_site_root {
            parsed.set_path("/");
            parsed.set_query(None);
            parsed.set_fragment(None);
            return parsed.to_string().trim_end_matches('/').to_string();
        }
    }

    if let Some((prefix, _)) = trimmed.split_once("/backend-api/") {
        return prefix.trim_end_matches('/').to_string();
    }

    trimmed.trim_end_matches('/').to_string()
}

pub fn parse_bootstrap_from_html(html: &str) -> Result<ChatGptWebBootstrap, GatewayError> {
    let (sources, data_build) = parse_pow_resources(html);
    Ok(ChatGptWebBootstrap {
        pow_script_sources: sources,
        pow_data_build: data_build,
    })
}

pub fn bootstrap_from_payload_cache(
    extra_body: Option<&HashMap<String, Value>>,
) -> Option<ChatGptWebBootstrap> {
    let extra_body = extra_body?;
    let sources = extra_body
        .get("chatgptPowSources")
        .or_else(|| extra_body.get("powSources"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let data_build = extra_body
        .get("chatgptPowDataBuild")
        .or_else(|| extra_body.get("powDataBuild"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if sources.is_empty() && data_build.is_none() {
        return None;
    }
    Some(ChatGptWebBootstrap {
        pow_script_sources: sources,
        pow_data_build: data_build,
    })
}

pub fn merge_bootstrap_from_fallback(
    mut primary: ChatGptWebBootstrap,
    fallback: Option<&ChatGptWebBootstrap>,
) -> ChatGptWebBootstrap {
    if let Some(fallback) = fallback {
        if primary.pow_script_sources.is_empty() {
            primary.pow_script_sources = fallback.pow_script_sources.clone();
        }
        if primary.pow_data_build.is_none() {
            primary.pow_data_build = fallback.pow_data_build.clone();
        }
    }
    if primary.pow_script_sources.is_empty() {
        primary
            .pow_script_sources
            .push(CHATGPT_WEB_DEFAULT_POW_SCRIPT.to_string());
    }
    primary
}

fn parse_pow_resources(html: &str) -> (Vec<String>, Option<String>) {
    static SCRIPT_SRC_RE: OnceLock<Regex> = OnceLock::new();
    static DATA_BUILD_RE: OnceLock<Regex> = OnceLock::new();
    let script_src_re = SCRIPT_SRC_RE.get_or_init(|| {
        Regex::new(r#"(?is)<script[^>]+src=["']([^"']+)["']"#).expect("valid script src regex")
    });
    let data_build_re = DATA_BUILD_RE.get_or_init(|| {
        Regex::new(r#"<html[^>]*data-build=["']([^"']*)["']"#).expect("valid data build regex")
    });
    let mut sources = script_src_re
        .captures_iter(html)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_string()))
        .collect::<Vec<_>>();
    if sources.is_empty() {
        sources.push(CHATGPT_WEB_DEFAULT_POW_SCRIPT.to_string());
    }
    let data_build = sources
        .iter()
        .find_map(|src| {
            Regex::new(r#"c/[^/]*/_"#)
                .ok()
                .and_then(|pattern| pattern.find(src).map(|value| value.as_str().to_string()))
        })
        .or_else(|| {
            data_build_re
                .captures(html)
                .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()))
        });
    (sources, data_build)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_bootstrap_resources_extracts_scripts_and_build() {
        let html = r#"<html data-build="build-123"><script src="https://chatgpt.com/c/foo/_/bar.js"></script></html>"#;
        let bootstrap = parse_bootstrap_from_html(html).expect("bootstrap");
        assert_eq!(bootstrap.pow_script_sources.len(), 1);
        assert_eq!(bootstrap.pow_data_build.as_deref(), Some("c/foo/_"));
    }

    #[test]
    fn bootstrap_from_payload_cache_reads_cached_pow_material() {
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "chatgptPowSources".to_string(),
            json!(["https://chatgpt.com/backend-api/sentinel/sdk.js"]),
        );
        extra_body.insert("chatgptPowDataBuild".to_string(), json!("build-1"));
        let bootstrap = bootstrap_from_payload_cache(Some(&extra_body)).expect("bootstrap");
        assert_eq!(bootstrap.pow_script_sources.len(), 1);
        assert_eq!(bootstrap.pow_data_build.as_deref(), Some("build-1"));
    }

    #[test]
    fn normalize_site_base_url_collapses_legacy_conversation_endpoint() {
        assert_eq!(
            normalize_site_base_url("https://chatgpt.com/backend-api/conversation"),
            "https://chatgpt.com"
        );
        assert_eq!(
            normalize_site_base_url("https://chatgpt.com/backend-api/conversation?foo=bar"),
            "https://chatgpt.com"
        );
    }
}
