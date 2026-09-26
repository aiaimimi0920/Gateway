//! URL diagnostics preserve transport, environment-substitution and secret-safety rules.

use super::super::RouteConfigDiagnostics;
use crate::console::secrets::is_sensitive_key;
use crate::routing::config::subst_env;
use url::Url;

pub(super) fn validate_http_url(
    value: &str,
    code: &str,
    path: String,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let resolved = subst_env(value);
    // Keepalive is an optional integration. An empty value or an unresolved
    // environment placeholder means the integration is intentionally disabled.
    if resolved.trim().is_empty() || resolved.contains("${") {
        return;
    }
    validate_http_url_inner(&resolved, code, path, diagnostics);
}

pub(super) fn validate_literal_http_url(
    value: &str,
    code: &str,
    path: String,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    validate_http_url_inner(value, code, path, diagnostics);
}

fn validate_http_url_inner(
    value: &str,
    code: &str,
    path: String,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let Ok(url) = Url::parse(value) else {
        diagnostics.push_error(code, path, "URL must use HTTP or HTTPS and include a host");
        return;
    };
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        diagnostics.push_error(
            code,
            path.clone(),
            "URL must use HTTP or HTTPS and include a host",
        );
    }
    validate_url_safety(&url, &path, code, diagnostics);
}

pub(super) fn validate_provider_url(
    value: &str,
    code: &str,
    path: String,
    websocket_transport: bool,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let resolved = subst_env(value);
    let Ok(url) = Url::parse(&resolved) else {
        let expected = if websocket_transport {
            "HTTP(S) or WebSocket (WS/WSS)"
        } else {
            "HTTP or HTTPS"
        };
        diagnostics.push_error(
            code,
            path,
            format!("URL must use {expected} and include a host"),
        );
        return;
    };
    let scheme_allowed = matches!(url.scheme(), "http" | "https")
        || (websocket_transport && matches!(url.scheme(), "ws" | "wss"));
    if !scheme_allowed || url.host_str().is_none() {
        let expected = if websocket_transport {
            "HTTP(S) or WebSocket (WS/WSS)"
        } else {
            "HTTP or HTTPS"
        };
        diagnostics.push_error(
            code,
            path.clone(),
            format!("URL must use {expected} and include a host"),
        );
    }
    validate_url_safety(&url, &path, code, diagnostics);
}

fn validate_url_safety(
    url: &Url,
    path: &str,
    code: &str,
    diagnostics: &mut RouteConfigDiagnostics,
) {
    let field = code.strip_suffix("_invalid").unwrap_or(code);
    if !url.username().is_empty() || url.password().is_some() {
        diagnostics.push_error(
            format!("{field}_userinfo_forbidden"),
            path,
            "URL userinfo is not permitted in route configuration",
        );
    }
    if url
        .query_pairs()
        .any(|(key, _)| is_sensitive_key(key.as_ref()))
    {
        diagnostics.push_error(
            format!("{field}_query_secret_forbidden"),
            path,
            "URL query keys classified as sensitive are not permitted",
        );
    }
    if url.fragment().is_some() {
        diagnostics.push_error(
            format!("{field}_fragment_forbidden"),
            path,
            "URL fragments are not permitted in route configuration",
        );
    }
}

pub(super) fn provider_uses_websocket_transport(
    provider: &crate::routing::config::ProviderConfigYaml,
) -> bool {
    [
        provider.preset.as_deref(),
        provider.adapter.as_deref(),
        provider.protocol_family.as_deref(),
        provider.protocol_profile.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        normalized == "xfyun-websocket"
            || normalized == "xfyun-native-websocket"
            || normalized.contains("websocket")
    })
}
