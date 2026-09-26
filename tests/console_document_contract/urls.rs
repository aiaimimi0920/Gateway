//! URL validation and repair redaction preserve compiler transport and environment rules.

use super::{diagnostic_codes, document};
use neuro_gateway::console::document::{inspect_route_document, validate_route_document};
use neuro_gateway::console::secrets::redact_route_document;
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

fn environment_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

struct ScopedEnvVar {
    name: &'static str,
    previous: Option<OsString>,
}

impl ScopedEnvVar {
    fn set(name: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }

    fn remove(name: &'static str) -> Self {
        let previous = std::env::var_os(name);
        std::env::remove_var(name);
        Self { name, previous }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}

#[test]
fn websocket_provider_urls_follow_the_real_compiler_transport_rules() {
    let candidate = document(
        r#"
providers:
  - id: xfyun-native
    preset: xfyun-websocket
    base_url: wss://spark-api.xf-yun.com/v1.1/chat
    api_key: appid:api-secret
model_routes: []
aliases: {}
"#,
    );
    validate_route_document(candidate)
        .expect("xfyun websocket providers legitimately use wss URLs");
}

#[test]
fn strict_url_validation_uses_environment_substitution_but_canonical_stays_raw() {
    let _guard = environment_test_lock();
    let _provider = ScopedEnvVar::set("GW_CONSOLE_PROVIDER_BASE", "https://provider.example.com");
    let _credential = ScopedEnvVar::set(
        "GW_CONSOLE_CREDENTIAL_BASE",
        "https://credential.example.com",
    );
    let _keepalive =
        ScopedEnvVar::set("GW_CONSOLE_KEEPALIVE_BASE", "https://keepalive.example.com");
    let _refresh = ScopedEnvVar::set("GW_CONSOLE_REFRESH_BASE", "https://oauth.example.com/token");
    let _unknown = ScopedEnvVar::remove("GW_CONSOLE_UNKNOWN_BASE");
    let candidate = document(
        r#"
providers:
  - id: provider
    base_url: ${GW_CONSOLE_PROVIDER_BASE}
    keepalive:
      serviceUrl: ${GW_CONSOLE_KEEPALIVE_BASE}
    credentials:
      - id: credential
        base_url: ${GW_CONSOLE_CREDENTIAL_BASE}
model_routes: []
aliases: {}
"#,
    );
    let validated = validate_route_document(candidate).expect("resolved URLs are valid");
    assert!(
        String::from_utf8_lossy(validated.canonical_yaml()).contains("${GW_CONSOLE_PROVIDER_BASE}")
    );
    assert!(!String::from_utf8_lossy(validated.canonical_yaml()).contains("provider.example.com"));
    let unknown = document(
        r#"
providers:
  - id: unknown
    base_url: ${GW_CONSOLE_UNKNOWN_BASE}
model_routes: []
aliases: {}
"#,
    );
    let error = validate_route_document(unknown).expect_err("unknown env URL must be diagnosed");
    assert!(diagnostic_codes(&error).contains(&"provider_base_url_invalid"));

    let literal_refresh = document(
        r#"
providers:
  - id: literal-refresh
    base_url: https://example.com
    credentials:
      - id: credential
        refresh_endpoint: ${GW_CONSOLE_REFRESH_BASE}
model_routes: []
aliases: {}
"#,
    );
    let error = validate_route_document(literal_refresh)
        .expect_err("refresh endpoints are literal because the compiler does not substitute them");
    assert!(diagnostic_codes(&error).contains(&"credential_refresh_endpoint_invalid"));
}

#[test]
fn url_userinfo_is_rejected_and_never_returned_by_redaction() {
    let candidate = document(
        r#"
providers:
  - id: provider
    base_url: https://provider-user:provider-url-secret@example.com?api_key=provider-query-secret&region=us
    keepalive:
      serviceUrl: https://keepalive-user:keepalive-url-secret@keeper.example.com
    credentials:
      - id: credential
        base_url: https://credential-user:credential-url-secret@credential.example.com
        refresh_endpoint: https://oauth-user:oauth-url-secret@oauth.example.com/token?client_secret=oauth-query-secret
  - id: websocket
    preset: xfyun-websocket
    base_url: wss://websocket-user:websocket-url-secret@spark-api.xf-yun.com/v1.1/chat
    api_key: appid:api-secret
model_routes: []
aliases: {}
"#,
    );
    let inspection = inspect_route_document(&candidate);
    let codes: Vec<&str> = inspection
        .diagnostics
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    for expected in [
        "provider_base_url_userinfo_forbidden",
        "provider_keepalive_url_userinfo_forbidden",
        "credential_base_url_userinfo_forbidden",
        "credential_refresh_endpoint_userinfo_forbidden",
        "provider_base_url_query_secret_forbidden",
        "credential_refresh_endpoint_query_secret_forbidden",
    ] {
        assert!(codes.contains(&expected), "missing {expected}");
    }
    for misleading in [
        "provider_base_url_invalid",
        "provider_keepalive_url_invalid",
        "credential_base_url_invalid",
        "credential_refresh_endpoint_invalid",
    ] {
        assert!(
            !codes.contains(&misleading),
            "structurally valid unsafe URL should not also report {misleading}"
        );
    }

    let redacted = redact_route_document(&candidate).expect("redaction remains safe for repair UI");
    let serialized = serde_json::to_string(&redacted.document).unwrap();
    for secret in [
        "provider-url-secret",
        "keepalive-url-secret",
        "credential-url-secret",
        "oauth-url-secret",
        "websocket-url-secret",
        "provider-query-secret",
        "oauth-query-secret",
    ] {
        assert!(!serialized.contains(secret), "URL secret leaked: {secret}");
    }
    assert!(!serialized.contains("provider-user"));
    assert!(!serialized.contains("websocket-user"));
    assert!(serialized.contains("region=us"));
}

#[test]
fn url_fragments_are_rejected_and_removed_from_the_management_document() {
    let candidate = document(
        r#"
providers:
  - id: provider
    base_url: https://provider.example.com/v1?region=us#provider-fragment-secret
    keepalive:
      serviceUrl: https://keeper.example.com/health#keepalive-fragment-secret
    credentials:
      - id: credential
        base_url: https://credential.example.com/v1#credential-fragment-secret
        refresh_endpoint: https://oauth.example.com/token#refresh-fragment-secret
  - id: websocket
    preset: xfyun-websocket
    base_url: wss://spark-api.xf-yun.com/v1.1/chat#websocket-fragment-secret
    api_key: appid:api-secret
model_routes: []
aliases: {}
"#,
    );
    let inspection = inspect_route_document(&candidate);
    for (code, path) in [
        (
            "provider_base_url_fragment_forbidden",
            "/providers/0/base_url",
        ),
        (
            "provider_keepalive_url_fragment_forbidden",
            "/providers/0/keepalive/serviceUrl",
        ),
        (
            "credential_base_url_fragment_forbidden",
            "/providers/0/credentials/0/base_url",
        ),
        (
            "credential_refresh_endpoint_fragment_forbidden",
            "/providers/0/credentials/0/refresh_endpoint",
        ),
        (
            "provider_base_url_fragment_forbidden",
            "/providers/1/base_url",
        ),
    ] {
        assert!(
            inspection
                .diagnostics
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.path == path),
            "missing {code} at {path}"
        );
    }

    let redacted = redact_route_document(&candidate).expect("fragment redaction");
    let provider = &redacted.document.providers[0];
    for value in [
        provider.base_url.as_str(),
        provider.keepalive.as_ref().unwrap().service_url.as_str(),
        provider.credentials[0].base_url.as_deref().unwrap(),
        provider.credentials[0].refresh_endpoint.as_deref().unwrap(),
        redacted.document.providers[1].base_url.as_str(),
    ] {
        let url = url::Url::parse(value).expect("redacted endpoint remains a valid URL");
        assert_eq!(url.fragment(), None, "fragment remained in {value}");
    }
    assert!(provider.base_url.contains("region=us"));
    let serialized = serde_json::to_string(&redacted.document).unwrap();
    for secret in [
        "provider-fragment-secret",
        "keepalive-fragment-secret",
        "credential-fragment-secret",
        "refresh-fragment-secret",
        "websocket-fragment-secret",
    ] {
        assert!(
            !serialized.contains(secret),
            "fragment secret leaked: {secret}"
        );
    }
}
