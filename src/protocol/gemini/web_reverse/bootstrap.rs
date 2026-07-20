use std::collections::HashMap;

use regex::Regex;
use serde_json::Value;

use super::{GeminiWebBootstrap, GEMINI_WEB_DEFAULT_APP_PATH};
use crate::error::GatewayError;

pub fn parse_bootstrap_from_app_html(
    html: &str,
    fallback_language: Option<&str>,
) -> Result<GeminiWebBootstrap, GatewayError> {
    let access_token = extract_bootstrap_string(html, "SNlM0e");
    let build_label = extract_bootstrap_string(html, "cfb2h");
    let session_id = extract_bootstrap_string(html, "FdrFJe");
    let language = extract_bootstrap_string(html, "TuX5cc")
        .or_else(|| fallback_language.map(str::to_string))
        .unwrap_or_else(|| "en".to_string());
    let push_id = extract_bootstrap_string(html, "qKIAYe");
    let client_pctx = extract_bootstrap_string(html, "Ylro7b");
    let app_page_path = extract_app_page_path_from_html(html);

    Ok(GeminiWebBootstrap {
        access_token,
        build_label,
        session_id,
        language,
        push_id,
        client_pctx,
        app_page_path,
    })
}

pub fn bootstrap_from_payload_cache(
    extra_body: Option<&HashMap<String, Value>>,
) -> Option<GeminiWebBootstrap> {
    let extra_body = extra_body?;
    let access_token = extra_body
        .get("accessToken")
        .or_else(|| extra_body.get("access_token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let build_label = extra_body
        .get("buildLabel")
        .or_else(|| extra_body.get("build_label"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let session_id = extra_body
        .get("sessionId")
        .or_else(|| extra_body.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let language = extra_body
        .get("language")
        .or_else(|| extra_body.get("hl"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("en")
        .to_string();
    let push_id = extra_body
        .get("pushId")
        .or_else(|| extra_body.get("push_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let client_pctx = extra_body
        .get("clientPctx")
        .or_else(|| extra_body.get("client_pctx"))
        .or_else(|| extra_body.get("Ylro7b"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let app_page_path = ["appPagePath", "app_page_path", "pagePath", "page_path"]
        .iter()
        .find_map(|key| extra_body.get(*key).and_then(Value::as_str))
        .and_then(normalize_app_page_path_candidate)
        .or_else(|| {
            ["pageId", "page_id"]
                .iter()
                .find_map(|key| extra_body.get(*key).and_then(Value::as_str))
                .and_then(normalize_app_page_path_candidate)
        });
    Some(GeminiWebBootstrap {
        access_token,
        build_label,
        session_id,
        language,
        push_id,
        client_pctx,
        app_page_path,
    })
}

pub fn merge_bootstrap_from_fallback(
    mut primary: GeminiWebBootstrap,
    fallback: Option<&GeminiWebBootstrap>,
) -> GeminiWebBootstrap {
    let Some(fallback) = fallback else {
        return primary;
    };

    if primary.access_token.is_none() {
        primary.access_token = fallback.access_token.clone();
    }
    if primary.build_label.is_none() {
        primary.build_label = fallback.build_label.clone();
    }
    if primary.session_id.is_none() {
        primary.session_id = fallback.session_id.clone();
    }
    if primary.push_id.is_none() {
        primary.push_id = fallback.push_id.clone();
    }
    if primary.client_pctx.is_none() {
        primary.client_pctx = fallback.client_pctx.clone();
    }
    if primary.app_page_path.is_none() {
        primary.app_page_path = fallback.app_page_path.clone();
    }
    if primary.language.trim().is_empty() {
        primary.language = fallback.language.clone();
    }

    primary
}

pub fn normalize_app_page_path_candidate(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(path) = extract_path_from_url_like(trimmed) {
        return normalize_app_page_path_candidate(&path);
    }
    let without_query = trimmed.split(['?', '#']).next().unwrap_or_default().trim();
    if without_query.is_empty() {
        return None;
    }
    if without_query == GEMINI_WEB_DEFAULT_APP_PATH {
        return Some(GEMINI_WEB_DEFAULT_APP_PATH.to_string());
    }
    if without_query.starts_with("/app/") {
        let normalized = without_query.trim_end_matches('/');
        return (!normalized.is_empty()).then(|| normalized.to_string());
    }
    if without_query
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Some(format!("/app/{without_query}"));
    }
    None
}

pub fn extract_app_page_path_from_html(html: &str) -> Option<String> {
    for key in ["appPagePath", "app_page_path", "pagePath", "page_path"] {
        if let Some(value) = extract_bootstrap_string(html, key)
            .and_then(|value| normalize_app_page_path_candidate(&value))
        {
            return Some(value);
        }
    }
    for key in ["pageId", "page_id"] {
        if let Some(value) = extract_bootstrap_string(html, key)
            .and_then(|value| normalize_app_page_path_candidate(&value))
        {
            return Some(value);
        }
    }
    let regex = Regex::new(r#"(https://gemini\.google\.com)?(/app/[A-Za-z0-9_-]+)"#).ok()?;
    regex
        .captures(html)
        .and_then(|captures| captures.get(2))
        .map(|value| value.as_str())
        .and_then(normalize_app_page_path_candidate)
}

pub fn extract_app_page_path_from_url(url: &str) -> Option<String> {
    normalize_app_page_path_candidate(url)
}

fn extract_bootstrap_string(html: &str, key: &str) -> Option<String> {
    let pattern = format!(
        r#"["']{}["']\s*(?::|,)\s*["']([^"']+)["']"#,
        regex::escape(key)
    );
    let regex = Regex::new(pattern.as_str()).ok()?;
    regex
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

fn extract_path_from_url_like(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))?;
    let (_, rest) = without_scheme.split_once('/')?;
    Some(format!("/{}", rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bootstrap_fields_from_app_html() {
        let html = r#"<html><body>{"SNlM0e":"token-123","cfb2h":"bl-1","FdrFJe":"sid-1","TuX5cc":"zh-CN","qKIAYe":"push-1","Ylro7b":"pctx-1","pageId":"6524687326273f8c"}</body></html>"#;
        let bootstrap = parse_bootstrap_from_app_html(html, None).unwrap();
        assert_eq!(bootstrap.access_token.as_deref(), Some("token-123"));
        assert_eq!(bootstrap.build_label.as_deref(), Some("bl-1"));
        assert_eq!(bootstrap.session_id.as_deref(), Some("sid-1"));
        assert_eq!(bootstrap.language, "zh-CN");
        assert_eq!(bootstrap.push_id.as_deref(), Some("push-1"));
        assert_eq!(bootstrap.client_pctx.as_deref(), Some("pctx-1"));
        assert_eq!(
            bootstrap.app_page_path.as_deref(),
            Some("/app/6524687326273f8c")
        );
    }

    #[test]
    fn parse_bootstrap_without_token_keeps_optional_access_token_empty() {
        let html =
            r#"<html><body>{"cfb2h":"bl-1","FdrFJe":"sid-1","TuX5cc":"zh-CN"}</body></html>"#;
        let bootstrap = parse_bootstrap_from_app_html(html, None).unwrap();
        assert_eq!(bootstrap.access_token, None);
        assert_eq!(bootstrap.build_label.as_deref(), Some("bl-1"));
        assert_eq!(bootstrap.session_id.as_deref(), Some("sid-1"));
        assert_eq!(bootstrap.language, "zh-CN");
    }

    #[test]
    fn merge_bootstrap_uses_payload_cache_as_fallback() {
        let primary = GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-live".to_string()),
            session_id: Some("sid-live".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };
        let fallback = GeminiWebBootstrap {
            access_token: Some("at-cache".to_string()),
            build_label: Some("bl-cache".to_string()),
            session_id: Some("sid-cache".to_string()),
            language: "en".to_string(),
            push_id: Some("push-cache".to_string()),
            client_pctx: Some("pctx-cache".to_string()),
            app_page_path: Some("/app/6524687326273f8c".to_string()),
        };

        let merged = merge_bootstrap_from_fallback(primary, Some(&fallback));
        assert_eq!(merged.access_token.as_deref(), Some("at-cache"));
        assert_eq!(merged.build_label.as_deref(), Some("bl-live"));
        assert_eq!(merged.session_id.as_deref(), Some("sid-live"));
        assert_eq!(merged.language, "zh-CN");
        assert_eq!(merged.push_id.as_deref(), Some("push-cache"));
        assert_eq!(merged.client_pctx.as_deref(), Some("pctx-cache"));
        assert_eq!(
            merged.app_page_path.as_deref(),
            Some("/app/6524687326273f8c")
        );
    }
}
