use serde_json::Value;

use super::types::GeminiCanvasPureHttpSession;
use crate::error::GatewayError;

pub fn direct_http_referrer(base_url: &str, share_id: &str) -> String {
    format!("{}/share/{}", base_url.trim_end_matches('/'), share_id)
}

pub fn storage_state_to_pure_http_session(
    storage_state: &Value,
    target_url: &str,
    base_url: &str,
    auth_user: &str,
) -> Result<GeminiCanvasPureHttpSession, GatewayError> {
    let target_host = host_from_url(target_url).ok_or_else(|| {
        GatewayError::server_error("Gemini Canvas pure HTTP target URL was malformed.")
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_bad_url")
    })?;
    let base_host = host_from_url(base_url).unwrap_or_else(|| "gemini.google.com".to_string());
    let target_path = path_from_url(target_url).unwrap_or_else(|| "/".to_string());
    let base_path = path_from_url(base_url).unwrap_or_else(|| "/".to_string());
    let target_scheme = scheme_from_url(target_url).unwrap_or("https");
    let base_scheme = scheme_from_url(base_url).unwrap_or("https");
    let cookies = storage_state
        .get("cookies")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas browser-state object did not contain a Playwright cookies array.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_missing_cookies")
        })?;

    #[derive(Debug, Clone)]
    struct MatchedCookie {
        name: String,
        value: String,
        path: String,
        exact_host: bool,
        original_index: usize,
    }

    let mut matched_cookies: Vec<MatchedCookie> = Vec::new();
    for (index, cookie) in cookies.iter().enumerate() {
        let Some(name) = cookie.get("name").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        let Some(value) = cookie.get("value").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if name.is_empty() || value.is_empty() {
            continue;
        }

        let matches_target = cookie_matches_url(cookie, &target_host, &target_path, target_scheme);
        let matches_base = cookie_matches_url(cookie, &base_host, &base_path, base_scheme);
        let matches_target_origin = cookie_matches_origin(cookie, &target_host, target_scheme);
        let matches_base_origin = cookie_matches_origin(cookie, &base_host, base_scheme);
        if !matches_target && !matches_base && !matches_target_origin && !matches_base_origin {
            continue;
        }

        let cookie_domain = cookie
            .get("domain")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let path = cookie
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("/")
            .to_string();
        matched_cookies.push(MatchedCookie {
            name: name.to_string(),
            value: value.to_string(),
            path,
            exact_host: !cookie_domain.starts_with('.')
                && cookie_domain.eq_ignore_ascii_case(&base_host),
            original_index: index,
        });
    }

    if matched_cookies.is_empty() && is_fixture_runtime_host(&target_host, &base_host) {
        for (index, cookie) in cookies.iter().enumerate() {
            let Some(name) = cookie.get("name").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            let Some(value) = cookie.get("value").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if name.is_empty() || value.is_empty() {
                continue;
            }
            let cookie_domain = cookie
                .get("domain")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if !cookie_domain.contains("google.") && !cookie_domain.contains("gemini.google.com") {
                continue;
            }
            let path = cookie
                .get("path")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("/")
                .to_string();
            matched_cookies.push(MatchedCookie {
                name: name.to_string(),
                value: value.to_string(),
                path,
                exact_host: false,
                original_index: index,
            });
        }
    }

    matched_cookies.sort_by(|a, b| {
        b.path
            .len()
            .cmp(&a.path.len())
            .then_with(|| b.exact_host.cmp(&a.exact_host))
            .then_with(|| a.original_index.cmp(&b.original_index))
    });

    let sapisid = matched_cookies
        .iter()
        .find(|cookie| {
            matches!(
                cookie.name.as_str(),
                "__Secure-1PAPISID" | "__Secure-3PAPISID" | "SAPISID"
            )
        })
        .map(|cookie| cookie.value.clone())
        .ok_or_else(|| {
            GatewayError::service_unavailable(
                "Gemini Canvas browser-state did not include SAPISID-compatible Google cookies for pure HTTP signing.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_missing_sapisid")
        })?;
    if matched_cookies.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas browser-state did not include any cookies usable for pure HTTP replay.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_pure_http_missing_cookie_header"));
    }

    let cookie_header = matched_cookies
        .iter()
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>()
        .join("; ");

    Ok(GeminiCanvasPureHttpSession {
        cookie_header,
        sapisid,
        auth_user: auth_user.trim().to_string(),
    })
}

pub fn pure_http_session_from_cookie_header(
    cookie_header: &str,
    auth_user: &str,
) -> Result<GeminiCanvasPureHttpSession, GatewayError> {
    let cookie_header = cookie_header.trim();
    if cookie_header.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas browser-state did not include any cookies usable for pure HTTP replay.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_pure_http_missing_cookie_header"));
    }

    let sapisid = cookie_header
        .split(';')
        .filter_map(|chunk| {
            let (name, value) = chunk.trim().split_once('=')?;
            let name = name.trim();
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            matches!(
                name,
                "__Secure-1PAPISID" | "__Secure-3PAPISID" | "SAPISID"
            )
            .then_some(value.to_string())
        })
        .next()
        .ok_or_else(|| {
            GatewayError::service_unavailable(
                "Gemini Canvas browser-state did not include SAPISID-compatible Google cookies for pure HTTP signing.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_missing_sapisid")
        })?;

    Ok(GeminiCanvasPureHttpSession {
        cookie_header: cookie_header.to_string(),
        sapisid,
        auth_user: auth_user.trim().to_string(),
    })
}

fn is_fixture_runtime_host(target_host: &str, base_host: &str) -> bool {
    [target_host, base_host].iter().any(|host| {
        host.eq_ignore_ascii_case("host.docker.internal")
            || host.eq_ignore_ascii_case("localhost")
            || host.eq_ignore_ascii_case("127.0.0.1")
    })
}

pub fn build_sapisid_authorization(
    sapisid: &str,
    origin: &str,
    timestamp_secs: i64,
) -> Result<String, GatewayError> {
    let sapisid = sapisid.trim();
    if sapisid.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas pure HTTP signing requires a non-empty SAPISID cookie.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_pure_http_missing_sapisid"));
    }
    let signing_input = format!("{timestamp_secs} {sapisid} {}", origin.trim());
    let digest = <sha1::Sha1 as sha1::Digest>::digest(signing_input.as_bytes());
    let hash = hex::encode(digest);
    Ok(format!(
        "SAPISIDHASH {timestamp_secs}_{hash} SAPISID1PHASH {timestamp_secs}_{hash} SAPISID3PHASH {timestamp_secs}_{hash}"
    ))
}

fn host_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()?
        .split('@')
        .last()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

fn cookie_domain_matches(cookie_domain: &str, host: &str) -> bool {
    let raw_domain = cookie_domain.trim().to_ascii_lowercase();
    let domain = raw_domain.trim_start_matches('.');
    let host = host.trim().to_ascii_lowercase();
    if domain.is_empty() || host.is_empty() {
        return false;
    }
    if raw_domain.starts_with('.') {
        host == domain || host.ends_with(&format!(".{domain}"))
    } else {
        host == domain
    }
}

fn current_unix_timestamp_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn scheme_from_url(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    if trimmed.starts_with("https://") {
        Some("https")
    } else if trimmed.starts_with("http://") {
        Some("http")
    } else {
        None
    }
}

fn path_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let path = without_scheme
        .split_once('/')
        .map(|(_, rest)| {
            let path = rest.split(['?', '#']).next().unwrap_or_default();
            if path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", path)
            }
        })
        .unwrap_or_else(|| "/".to_string());
    Some(path)
}

fn cookie_path_matches(cookie_path: &str, request_path: &str) -> bool {
    let cookie_path = if cookie_path.trim().is_empty() {
        "/"
    } else {
        cookie_path.trim()
    };
    let request_path = if request_path.trim().is_empty() {
        "/"
    } else {
        request_path.trim()
    };

    if cookie_path == "/" {
        return true;
    }
    if request_path == cookie_path {
        return true;
    }
    if !request_path.starts_with(cookie_path) {
        return false;
    }

    cookie_path.ends_with('/')
        || request_path
            .as_bytes()
            .get(cookie_path.len())
            .is_some_and(|byte| *byte == b'/')
}

fn cookie_matches_url(cookie: &Value, host: &str, path: &str, scheme: &str) -> bool {
    if !cookie_matches_origin(cookie, host, scheme) {
        return false;
    }

    let cookie_path = cookie
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    cookie_path_matches(cookie_path, path)
}

fn cookie_matches_origin(cookie: &Value, host: &str, scheme: &str) -> bool {
    let cookie_domain = cookie
        .get("domain")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if cookie_domain.is_empty() || host.trim().is_empty() {
        return false;
    }

    if cookie
        .get("secure")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !scheme.eq_ignore_ascii_case("https")
    {
        return false;
    }

    if let Some(expires) = cookie.get("expires").and_then(Value::as_f64) {
        let now_secs = current_unix_timestamp_i64() as f64;
        if expires > 0.0 && expires <= now_secs {
            return false;
        }
    }

    if !cookie_domain_matches(cookie_domain, host) {
        return false;
    }
    true
}
