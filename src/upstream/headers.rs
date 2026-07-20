// ---------------------------------------------------------------------------
// upstream/headers.rs — Build HTTP headers from a ProviderAccountPayload
// ---------------------------------------------------------------------------

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use rquest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};

use crate::protocol::kiro;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::accio as accio_upstream;
use crate::upstream::grok as grok_upstream;

const COOKIE_CHUNK_SIZE: usize = 3180;
const ENCODE_URI_COMPONENT_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

// ---------------------------------------------------------------------------
// build_upstream_headers
// ---------------------------------------------------------------------------

/// Build an [`rquest::header::HeaderMap`] for the upstream HTTP request from
/// the given [`ProviderAccountPayload`].
///
/// Three adapter types are handled:
///
/// | Adapter                 | Auth header                                      |
/// |-------------------------|--------------------------------------------------|
/// | `openai_compatible`     | `Authorization: Bearer <key>` (default), `x-api-key: <key>`, or `api-key: <key>` depending on `auth_mode` |
/// | `anthropic_compatible`  | `x-api-key: <key>` + `anthropic-version` + optional `anthropic-beta` |
/// | `grok_compatible`       | cookie auth derived from `session_auth` + `x-xai-request-id` |
/// | `kiro_compatible`       | bearer auth + AWS/Kiro runtime headers (`x-amz-user-agent`, `x-amzn-codewhisperer-optout`, etc.) |
/// | `freebuff_compatible`   | bearer auth + FreeBuff runtime headers (`Accept: application/json, text/event-stream`, fixed SDK UA) |
/// | `gemini_business_compatible` | bearer/header auth derived from `session_auth` |
/// | `gemini_web_compatible` | Google web cookie auth using `__Secure-1PSID` + optional `__Secure-1PSIDTS` |
/// | `chatgpt_web_reverse_compatible` | bearer auth plus ChatGPT site-specific reverse-web headers |
/// | `chataibot_compatible` | cookie auth derived from `session_auth` |
/// | `lumalabs_compatible` | cookie auth derived from `session_auth` |
/// | `producer_compatible` | bearer/header auth derived from `session_auth` |
/// | `suno_compatible` | bearer/header auth derived from `session_auth` + runtime cookie header |
/// | `udio_compatible` | cookie auth derived from `session_auth` |
/// | `search_api_compatible` | `Authorization: Bearer <key>` by default, or `<auth_header_name>: <auth_token|api_key>` for search providers like You.com |
/// | `custom_http`           | `<auth_header_name>: <auth_token>` (fallback to `api_key`) |
///
/// In all cases, `Content-Type: application/json` is set, and any headers in
/// `payload.headers` are appended last (overriding defaults).
///
/// `extra_headers` allows the caller to forward additional client-supplied
/// headers (e.g. from `PipelineContext::request_headers`).  These are inserted
/// *before* the provider-specific `payload.headers` so that provider config
/// takes priority.
pub fn build_upstream_headers(payload: &ProviderAccountPayload) -> HeaderMap {
    build_upstream_headers_with(payload, None)
}

/// Like [`build_upstream_headers`] but accepts optional extra headers to merge.
pub fn build_upstream_headers_with(
    payload: &ProviderAccountPayload,
    extra_headers: Option<&std::collections::HashMap<String, String>>,
) -> HeaderMap {
    let mut map = HeaderMap::new();

    // Always set Content-Type.
    map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    match payload.canonical_adapter() {
        "openai_compatible" => {
            let use_api_key = payload
                .auth_mode
                .as_deref()
                .map(|m| m == "api-key")
                .unwrap_or(false);
            let use_x_api_key = payload
                .auth_mode
                .as_deref()
                .map(|m| m == "x-api-key")
                .unwrap_or(false);

            if use_api_key {
                insert_header(&mut map, "api-key", &payload.api_key);
            } else if use_x_api_key {
                insert_header(&mut map, "x-api-key", &payload.api_key);
            } else {
                let bearer = format!("Bearer {}", payload.api_key);
                if let Ok(v) = HeaderValue::from_str(&bearer) {
                    map.insert(AUTHORIZATION, v);
                }
            }
        }

        "anthropic_compatible" => {
            insert_header(&mut map, "x-api-key", &payload.api_key);

            let version = payload.anthropic_version.as_deref().unwrap_or("2023-06-01");
            insert_header(&mut map, "anthropic-version", version);

            if let Some(betas) = &payload.beta_headers {
                if !betas.is_empty() {
                    insert_header(&mut map, "anthropic-beta", &betas.join(","));
                }
            }
        }

        "gemini_api_compatible" | "gemini_api_modular_compatible" => {
            if let Some(header_name) = payload.auth_header_name.as_deref() {
                let token = payload.auth_token.as_deref().unwrap_or(&payload.api_key);
                insert_header(&mut map, header_name, token);
            } else {
                insert_header(&mut map, "x-goog-api-key", &payload.api_key);
            }
        }

        "bedrock_converse_compatible" | "cohere_compatible" => {
            if let Some(header_name) = payload.auth_header_name.as_deref() {
                let token = payload.auth_token.as_deref().unwrap_or(&payload.api_key);
                insert_header(&mut map, header_name, token);
            } else {
                let bearer = format!("Bearer {}", payload.api_key);
                if let Ok(v) = HeaderValue::from_str(&bearer) {
                    map.insert(AUTHORIZATION, v);
                }
            }
        }

        "accio_compatible" => {
            // Accio does NOT use Authorization headers - auth is in the body.
            accio_upstream::apply_runtime_headers(payload, &mut map);
        }

        "grok_compatible" => {
            grok_upstream::apply_runtime_headers(payload, &mut map);
        }

        "kiro_compatible" => {
            apply_session_auth_headers(&mut map, payload);
            insert_header(&mut map, "x-amzn-codewhisperer-optout", "true");
            insert_header(&mut map, "x-amzn-kiro-agent-mode", "vibe");

            let refresh_token = kiro::read_payload_string(
                payload.extra_body.as_ref(),
                &["kiroRefreshToken", "refreshToken"],
            );
            let machine_id = kiro::generate_machine_id(
                kiro::read_payload_string(
                    payload.extra_body.as_ref(),
                    &["kiroMachineId", "machineId"],
                )
                .as_deref(),
                refresh_token.as_deref(),
            )
            .unwrap_or_else(|| {
                "0000000000000000000000000000000000000000000000000000000000000000".to_string()
            });
            let version =
                kiro::read_payload_string(payload.extra_body.as_ref(), &["kiroVersion", "version"])
                    .unwrap_or_else(|| kiro::KIRO_DEFAULT_VERSION.to_string());
            let system_version = kiro::read_payload_string(
                payload.extra_body.as_ref(),
                &["kiroSystemVersion", "systemVersion"],
            )
            .unwrap_or_else(|| kiro::KIRO_DEFAULT_SYSTEM_VERSION.to_string());
            let node_version = kiro::read_payload_string(
                payload.extra_body.as_ref(),
                &["kiroNodeVersion", "nodeVersion"],
            )
            .unwrap_or_else(|| kiro::KIRO_DEFAULT_NODE_VERSION.to_string());
            let api_region = kiro::read_payload_string(
                payload.extra_body.as_ref(),
                &["kiroApiRegion", "apiRegion"],
            );
            let host = api_region
                .as_deref()
                .map(|region| format!("q.{region}.amazonaws.com"))
                .or_else(|| infer_host_from_base_url(&payload.base_url))
                .unwrap_or_else(|| "q.us-east-1.amazonaws.com".to_string());

            insert_header(
                &mut map,
                "x-amz-user-agent",
                &format!("aws-sdk-js/1.0.34 KiroIDE-{version}-{machine_id}"),
            );
            insert_header(
                &mut map,
                "user-agent",
                &format!(
                    "aws-sdk-js/1.0.34 ua/2.1 os/{system_version} lang/js md/nodejs#{node_version} api/codewhispererstreaming#1.0.34 m/E KiroIDE-{version}-{machine_id}"
                ),
            );
            insert_header(&mut map, "host", &host);
            insert_header(
                &mut map,
                "amz-sdk-invocation-id",
                &uuid::Uuid::new_v4().to_string(),
            );
            insert_header(&mut map, "amz-sdk-request", "attempt=1; max=3");
            insert_header(&mut map, "connection", "close");
        }

        "freebuff_compatible" => {
            let bearer = format!("Bearer {}", payload.api_key);
            if let Ok(v) = HeaderValue::from_str(&bearer) {
                map.insert(AUTHORIZATION, v);
            }
            insert_header(&mut map, "accept", "application/json, text/event-stream");
            let user_agent = crate::protocol::freebuff::read_payload_string(
                payload.extra_body.as_ref(),
                &["freebuffUserAgent"],
            )
            .unwrap_or_else(|| crate::protocol::freebuff::FREEBUFF_DEFAULT_USER_AGENT.to_string());
            insert_header(&mut map, "user-agent", &user_agent);
        }

        "gemini_web_compatible" | "gemini_web_reverse_modular_compatible" => {
            apply_gemini_web_session_headers(&mut map, payload);
        }

        "gemini_business_compatible"
        | "chataibot_compatible"
        | "lumalabs_compatible"
        | "producer_compatible"
        | "suno_compatible"
        | "udio_compatible" => {
            apply_session_auth_headers(&mut map, payload);
        }

        "chatgpt_web_reverse_compatible" => {
            let has_runtime_cookie = payload
                .headers
                .get("Cookie")
                .or_else(|| payload.headers.get("cookie"))
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
            if !(payload.api_key.trim().is_empty() && has_runtime_cookie) {
                apply_session_auth_headers(&mut map, payload);
            }
        }

        "search_api_compatible" => {
            if let Some(header_name) = payload.auth_header_name.as_deref() {
                let token = payload.auth_token.as_deref().unwrap_or(&payload.api_key);
                insert_header(&mut map, header_name, token);
            } else {
                let bearer = format!("Bearer {}", payload.api_key);
                if let Ok(v) = HeaderValue::from_str(&bearer) {
                    map.insert(AUTHORIZATION, v);
                }
            }
        }

        "custom_http" => {
            // Auth header name defaults to "Authorization".
            let header_name = payload
                .auth_header_name
                .as_deref()
                .unwrap_or("Authorization");

            // Token defaults to api_key when auth_token is absent.
            let token = payload.auth_token.as_deref().unwrap_or(&payload.api_key);

            insert_header(&mut map, header_name, token);
        }

        // Unknown adapter — set bearer auth and hope for the best.
        _ => {
            let bearer = format!("Bearer {}", payload.api_key);
            if let Ok(v) = HeaderValue::from_str(&bearer) {
                map.insert(AUTHORIZATION, v);
            }
        }
    }

    // Append client-forwarded extra headers (lower priority than provider config).
    if let Some(extra) = extra_headers {
        for (name, value) in extra {
            // Skip auth-related headers — those are handled by adapter logic above.
            match name.to_lowercase().as_str() {
                "authorization" | "x-api-key" | "api-key" | "x-goog-api-key" | "content-type" => {}
                _ => {
                    insert_header(&mut map, name, value);
                }
            }
        }
    }

    // Append / override with provider-specific custom headers.
    for (name, value) in &payload.headers {
        if matches!(
            payload.canonical_adapter(),
            "gemini_web_compatible" | "gemini_web_reverse_modular_compatible"
        ) && name.eq_ignore_ascii_case("cookie")
        {
            continue;
        }
        if payload.canonical_adapter() == "accio_compatible" {
            let normalized = name.to_ascii_lowercase();
            if !matches!(
                normalized.as_str(),
                "utdid"
                    | "x-utdid"
                    | "version"
                    | "x-app-version"
                    | "appkey"
                    | "app_key"
                    | "cookie"
                    | "x-cna"
                    | "x-language"
                    | "language"
                    | "x-os"
                    | "os"
            ) {
                continue;
            }
        }
        insert_header(&mut map, name, value);
    }

    map
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn insert_header(map: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(n), Ok(v)) = (
        HeaderName::from_bytes(name.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        map.insert(n, v);
    }
}

fn apply_session_auth_headers(map: &mut HeaderMap, payload: &ProviderAccountPayload) {
    let transport = payload
        .session_auth
        .as_ref()
        .map(|cfg| cfg.transport.as_str())
        .unwrap_or("cookie");

    match transport {
        "bearer" => {
            let header_name = payload
                .session_auth
                .as_ref()
                .and_then(|cfg| cfg.header_name())
                .unwrap_or("authorization");
            insert_header(map, header_name, &format!("Bearer {}", payload.api_key));
        }
        "header" => {
            let header_name = payload
                .session_auth
                .as_ref()
                .and_then(|cfg| cfg.header_name())
                .unwrap_or("x-session-token");
            insert_header(map, header_name, &payload.api_key);
        }
        _ => {
            let (primary, secondary) = payload
                .session_auth
                .as_ref()
                .map(|cfg| {
                    (
                        cfg.primary_cookie_name().to_string(),
                        cfg.secondary_cookie_name().map(str::to_string),
                    )
                })
                .unwrap_or_else(|| ("sso".to_string(), Some("sso-rw".to_string())));

            let mut cookies = render_cookie_pairs(&primary, &payload.api_key);
            if let Some(secondary) = secondary {
                cookies.extend(render_cookie_pairs(&secondary, &payload.api_key));
            }
            insert_header(map, "cookie", &cookies.join("; "));
        }
    }
}

fn apply_gemini_web_session_headers(map: &mut HeaderMap, payload: &ProviderAccountPayload) {
    let config = payload.session_auth.as_ref();
    let primary = config
        .map(|cfg| cfg.primary_cookie_name())
        .unwrap_or("__Secure-1PSID");
    let secondary = config
        .and_then(|cfg| cfg.secondary_cookie_name())
        .unwrap_or("__Secure-1PSIDTS");

    let mut cookies = Vec::new();
    cookies.extend(render_cookie_pairs(primary, &payload.api_key));
    if let Some(auth_token) = payload
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        cookies.extend(render_cookie_pairs(secondary, auth_token));
    }
    if let Some(existing_cookie) = payload
        .headers
        .get("Cookie")
        .or_else(|| payload.headers.get("cookie"))
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let filtered_pairs = existing_cookie
            .split(';')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .filter(|entry| {
                let name = entry.split('=').next().unwrap_or_default().trim();
                !name.eq_ignore_ascii_case(primary) && !name.eq_ignore_ascii_case(secondary)
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        cookies.extend(filtered_pairs);
    }
    insert_header(map, "cookie", &cookies.join("; "));
}

fn infer_host_from_base_url(base_url: &str) -> Option<String> {
    let trimmed = base_url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme.split('/').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn render_cookie_pairs(name: &str, value: &str) -> Vec<String> {
    create_cookie_chunks(name, value)
        .into_iter()
        .map(|(chunk_name, chunk_value)| format!("{chunk_name}={chunk_value}"))
        .collect()
}

fn create_cookie_chunks(name: &str, value: &str) -> Vec<(String, String)> {
    let mut encoded = utf8_percent_encode(value, ENCODE_URI_COMPONENT_SET).to_string();
    if encoded.len() <= COOKIE_CHUNK_SIZE {
        return vec![(name.to_string(), value.to_string())];
    }

    let mut chunks = Vec::new();
    while !encoded.is_empty() {
        let mut encoded_head = encoded.chars().take(COOKIE_CHUNK_SIZE).collect::<String>();

        if let Some(last_escape_pos) = encoded_head.rfind('%') {
            if last_escape_pos > COOKIE_CHUNK_SIZE.saturating_sub(3) {
                encoded_head.truncate(last_escape_pos);
            }
        }

        let value_head = loop {
            if encoded_head.is_empty() {
                break String::new();
            }

            match percent_decode_str(&encoded_head).decode_utf8() {
                Ok(decoded) => break decoded.into_owned(),
                Err(_) => {
                    if encoded_head.len() > 3
                        && encoded_head.as_bytes()[encoded_head.len() - 3] == b'%'
                    {
                        encoded_head.truncate(encoded_head.len() - 3);
                    } else {
                        let fallback = percent_decode_str(&encoded_head)
                            .decode_utf8_lossy()
                            .into_owned();
                        break fallback;
                    }
                }
            }
        };

        chunks.push(value_head);
        encoded = encoded[encoded_head.len()..].to_string();
    }

    chunks
        .into_iter()
        .enumerate()
        .map(|(idx, chunk)| (format!("{name}.{idx}"), chunk))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_payload(adapter: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "sk-test-1234".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn header_str<'a>(map: &'a HeaderMap, name: &str) -> Option<&'a str> {
        map.get(name).and_then(|v| v.to_str().ok())
    }

    // ── openai_compatible ─────────────────────────────────────────────────

    #[test]
    fn openai_bearer_auth_by_default() {
        let payload = make_payload("openai_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
        assert_eq!(
            header_str(&headers, "content-type"),
            Some("application/json")
        );
    }

    #[test]
    fn openai_x_api_key_mode() {
        let mut payload = make_payload("openai_compatible");
        payload.auth_mode = Some("x-api-key".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-api-key"), Some("sk-test-1234"));
        // Should NOT set Authorization in x-api-key mode.
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn openai_api_key_mode() {
        let mut payload = make_payload("openai_compatible");
        payload.auth_mode = Some("api-key".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "api-key"), Some("sk-test-1234"));
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn search_api_can_request_json_response_with_custom_accept_header() {
        let mut payload = make_payload("search_api_compatible");
        payload
            .headers
            .insert("Accept".to_string(), "application/json".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
        assert_eq!(header_str(&headers, "accept"), Some("application/json"));
        assert_eq!(
            header_str(&headers, "content-type"),
            Some("application/json")
        );
    }

    // ── anthropic_compatible ──────────────────────────────────────────────

    #[test]
    fn anthropic_sets_required_headers() {
        let payload = make_payload("anthropic_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-api-key"), Some("sk-test-1234"));
        assert_eq!(
            header_str(&headers, "anthropic-version"),
            Some("2023-06-01")
        );
        assert_eq!(
            header_str(&headers, "content-type"),
            Some("application/json")
        );
    }

    #[test]
    fn anthropic_uses_custom_version() {
        let mut payload = make_payload("anthropic_compatible");
        payload.anthropic_version = Some("2024-01-01".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "anthropic-version"),
            Some("2024-01-01")
        );
    }

    #[test]
    fn anthropic_joins_beta_headers() {
        let mut payload = make_payload("anthropic_compatible");
        payload.beta_headers = Some(vec![
            "tools-2024-04-04".to_string(),
            "computer-use-2024-10-22".to_string(),
        ]);
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "anthropic-beta"),
            Some("tools-2024-04-04,computer-use-2024-10-22")
        );
    }

    #[test]
    fn anthropic_no_beta_header_when_empty() {
        let mut payload = make_payload("anthropic_compatible");
        payload.beta_headers = Some(vec![]);
        let headers = build_upstream_headers(&payload);
        assert!(headers.get("anthropic-beta").is_none());
    }

    #[test]
    fn gemini_api_defaults_to_x_goog_api_key() {
        let payload = make_payload("gemini_api_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-goog-api-key"), Some("sk-test-1234"));
    }

    #[test]
    fn gemini_api_modular_defaults_to_x_goog_api_key() {
        let payload = make_payload("gemini_api_modular_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-goog-api-key"), Some("sk-test-1234"));
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn chatgpt_web_reverse_uses_session_bearer_auth() {
        let mut payload = make_payload("chatgpt_web_reverse_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
    }

    #[test]
    fn chatgpt_web_reverse_skips_bearer_when_cookie_only_runtime_is_present() {
        let mut payload = make_payload("chatgpt_web_reverse_compatible");
        payload.api_key = String::new();
        payload.headers.insert(
            "Cookie".to_string(),
            "__Secure-next-auth.session-token=abc; cf_clearance=def".to_string(),
        );
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });

        let headers = build_upstream_headers(&payload);
        assert!(headers.get("authorization").is_none());
        assert_eq!(
            header_str(&headers, "cookie"),
            Some("__Secure-next-auth.session-token=abc; cf_clearance=def")
        );
    }

    #[test]
    fn gemini_web_reverse_modular_uses_google_cookie_session_auth() {
        let mut payload = make_payload("gemini_web_reverse_modular_compatible");
        payload.auth_token = Some("psidts-test-5678".to_string());
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("__Secure-1PSID".to_string()),
            secondary_cookie_name: Some("__Secure-1PSIDTS".to_string()),
            header_name: None,
            expires_at: None,
        });
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "cookie"),
            Some("__Secure-1PSID=sk-test-1234; __Secure-1PSIDTS=psidts-test-5678")
        );
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn gemini_api_can_use_custom_auth_header() {
        let mut payload = make_payload("gemini_api_compatible");
        payload.auth_header_name = Some("authorization".to_string());
        payload.auth_token = Some("Bearer gemini-token".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer gemini-token")
        );
    }

    #[test]
    fn gemini_api_ignores_forwarded_x_goog_api_key_override() {
        let payload = make_payload("gemini_api_compatible");
        let mut extra_headers = HashMap::new();
        extra_headers.insert(
            "x-goog-api-key".to_string(),
            "gateway-platform-key".to_string(),
        );
        extra_headers.insert(
            "authorization".to_string(),
            "Bearer gateway-platform-key".to_string(),
        );
        let headers = build_upstream_headers_with(&payload, Some(&extra_headers));
        assert_eq!(header_str(&headers, "x-goog-api-key"), Some("sk-test-1234"));
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn cohere_defaults_to_bearer_auth() {
        let payload = make_payload("cohere_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
    }

    #[test]
    fn bedrock_converse_can_use_custom_auth_header() {
        let mut payload = make_payload("bedrock_converse_compatible");
        payload.auth_header_name = Some("x-amz-custom-auth".to_string());
        payload.auth_token = Some("signed-bedrock-token".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "x-amz-custom-auth"),
            Some("signed-bedrock-token")
        );
    }

    // ── custom_http ───────────────────────────────────────────────────────

    #[test]
    fn custom_http_uses_auth_header_name_and_token() {
        let mut payload = make_payload("custom_http");
        payload.auth_header_name = Some("x-custom-auth".to_string());
        payload.auth_token = Some("my-token-999".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-custom-auth"), Some("my-token-999"));
    }

    #[test]
    fn custom_http_falls_back_to_api_key_when_auth_token_absent() {
        let mut payload = make_payload("custom_http");
        payload.auth_header_name = Some("x-custom-auth".to_string());
        // No auth_token set — falls back to api_key.
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-custom-auth"), Some("sk-test-1234"));
    }

    // ── custom override headers ───────────────────────────────────────────

    #[test]
    fn custom_headers_are_appended() {
        let mut payload = make_payload("openai_compatible");
        payload
            .headers
            .insert("x-trace-id".to_string(), "trace-abc".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-trace-id"), Some("trace-abc"));
    }

    #[test]
    fn custom_headers_can_override_default_headers() {
        let mut payload = make_payload("openai_compatible");
        payload.headers.insert(
            "content-type".to_string(),
            "application/json; charset=utf-8".to_string(),
        );
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "content-type"),
            Some("application/json; charset=utf-8")
        );
    }

    // ── accio_compatible ─────────────────────────────────────────────────

    #[test]
    fn accio_sets_required_headers() {
        let payload = make_payload("accio_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "content-type"),
            Some("application/json")
        );
        assert_eq!(header_str(&headers, "accept"), Some("text/event-stream"));
        assert_eq!(header_str(&headers, "user-agent"), Some("node"));
        assert_eq!(header_str(&headers, "accept-language"), Some("zh-CN"));
        assert_eq!(header_str(&headers, "sec-fetch-mode"), Some("cors"));
        assert_eq!(header_str(&headers, "version"), Some("0.5.9"));
        assert_eq!(header_str(&headers, "x-app-version"), Some("0.5.9"));
        assert_eq!(header_str(&headers, "x-os"), Some("win32"));
        assert_eq!(header_str(&headers, "x-language"), Some("zh-CN"));
        assert!(headers.get("appKey").is_none());
        // No Authorization header for Accio.
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn accio_custom_headers_override_defaults() {
        let mut payload = make_payload("accio_compatible");
        payload
            .headers
            .insert("x-utdid".to_string(), "utd-test".to_string());
        payload
            .headers
            .insert("x-app-version".to_string(), "0.5.9".to_string());
        payload
            .headers
            .insert("appKey".to_string(), "99999999".to_string());
        payload
            .headers
            .insert("x-language".to_string(), "en-US".to_string());
        payload
            .headers
            .insert("x-os".to_string(), "darwin".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "utdid"), Some("utd-test"));
        assert_eq!(header_str(&headers, "version"), Some("0.5.9"));
        assert_eq!(header_str(&headers, "x-app-version"), Some("0.5.9"));
        assert_eq!(header_str(&headers, "appKey"), Some("99999999"));
        assert_eq!(header_str(&headers, "x-language"), Some("en-US"));
        assert_eq!(header_str(&headers, "x-os"), Some("darwin"));
    }

    #[test]
    fn accio_forwards_runtime_cookie_and_cna_headers() {
        let mut payload = make_payload("accio_compatible");
        payload.headers.insert(
            "Cookie".to_string(),
            "cna=test-cna; other=value".to_string(),
        );
        payload
            .headers
            .insert("x-cna".to_string(), "test-cna".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "cookie"),
            Some("cna=test-cna; other=value")
        );
        assert_eq!(header_str(&headers, "x-cna"), Some("test-cna"));
        assert_eq!(header_str(&headers, "version"), Some("0.5.9"));
    }

    // ── grok_compatible ────────────────────────────────────────────────

    #[test]
    fn grok_uses_cookie_auth_and_request_id() {
        let payload = make_payload("grok_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "cookie"),
            Some("sso=sk-test-1234; sso-rw=sk-test-1234")
        );
        assert!(headers.get("x-xai-request-id").is_some());
        assert_eq!(
            header_str(&headers, "content-type"),
            Some("application/json")
        );
    }

    #[test]
    fn grok_uses_session_auth_cookie_names_when_present() {
        let mut payload = make_payload("grok_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("custom-sso".to_string()),
            secondary_cookie_name: Some("custom-sso-rw".to_string()),
            header_name: None,
            expires_at: None,
        });
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "cookie"),
            Some("custom-sso=sk-test-1234; custom-sso-rw=sk-test-1234")
        );
    }

    #[test]
    fn grok_can_render_bearer_session_transport() {
        let mut payload = make_payload("grok_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
        assert!(headers.get("cookie").is_none());
    }

    #[test]
    fn gemini_business_uses_session_auth_header_transport() {
        let mut payload = make_payload("gemini_business_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
    }

    #[test]
    fn chataibot_uses_token_cookie_auth() {
        let mut payload = make_payload("chataibot_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("token".to_string()),
            secondary_cookie_name: None,
            header_name: None,
            expires_at: None,
        });
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "cookie"), Some("token=sk-test-1234"));
    }

    #[test]
    fn lumalabs_uses_wos_session_cookie_auth() {
        let mut payload = make_payload("lumalabs_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("wos-session".to_string()),
            secondary_cookie_name: None,
            header_name: None,
            expires_at: None,
        });
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "cookie"),
            Some("wos-session=sk-test-1234")
        );
    }

    #[test]
    fn cookie_transport_chunks_large_supabase_style_sessions() {
        let mut payload = make_payload("udio_compatible");
        payload.api_key = format!("base64-{}", "a".repeat(4000));
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "cookie".to_string(),
            primary_cookie_name: Some("sb-ssr-production-auth-token".to_string()),
            secondary_cookie_name: None,
            header_name: None,
            expires_at: None,
        });

        let headers = build_upstream_headers(&payload);
        let cookie = header_str(&headers, "cookie").unwrap_or_default();

        assert!(cookie.contains("sb-ssr-production-auth-token.0="));
        assert!(cookie.contains("sb-ssr-production-auth-token.1="));
        assert!(!cookie.contains("sb-ssr-production-auth-token=base64-"));
    }

    #[test]
    fn suno_uses_bearer_session_auth_and_runtime_cookie_header() {
        let mut payload = make_payload("suno_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        payload
            .headers
            .insert("cookie".to_string(), "__client=abc; other=def".to_string());

        let headers = build_upstream_headers(&payload);

        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
        assert_eq!(
            header_str(&headers, "cookie"),
            Some("__client=abc; other=def")
        );
    }

    #[test]
    fn search_api_uses_bearer_auth() {
        let payload = make_payload("search_api_compatible");
        let headers = build_upstream_headers(&payload);
        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
        assert_eq!(
            header_str(&headers, "content-type"),
            Some("application/json")
        );
    }

    #[test]
    fn search_api_compatible_can_use_custom_auth_header() {
        let mut payload = make_payload("search_api_compatible");
        payload.auth_header_name = Some("X-API-Key".to_string());
        let headers = build_upstream_headers(&payload);
        assert_eq!(header_str(&headers, "x-api-key"), Some("sk-test-1234"));
        assert!(headers.get("authorization").is_none());
    }

    #[test]
    fn freebuff_uses_bearer_auth_and_sdk_headers() {
        let mut payload = make_payload("freebuff_compatible");
        payload.extra_body = Some(HashMap::from([(
            "freebuffUserAgent".to_string(),
            serde_json::json!("freebuff-test-agent/1.0"),
        )]));

        let headers = build_upstream_headers(&payload);

        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
        assert_eq!(
            header_str(&headers, "accept"),
            Some("application/json, text/event-stream")
        );
        assert_eq!(
            header_str(&headers, "user-agent"),
            Some("freebuff-test-agent/1.0")
        );
    }

    #[test]
    fn kiro_uses_runtime_bearer_and_aws_headers() {
        let mut payload = make_payload("kiro_compatible");
        payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
            transport: "bearer".to_string(),
            primary_cookie_name: None,
            secondary_cookie_name: None,
            header_name: Some("authorization".to_string()),
            expires_at: None,
        });
        payload.extra_body = Some(HashMap::from([
            (
                "kiroRefreshToken".to_string(),
                serde_json::json!("refresh-123"),
            ),
            (
                "kiroApiRegion".to_string(),
                serde_json::json!("eu-central-1"),
            ),
        ]));

        let headers = build_upstream_headers(&payload);

        assert_eq!(
            header_str(&headers, "authorization"),
            Some("Bearer sk-test-1234")
        );
        assert_eq!(
            header_str(&headers, "x-amzn-codewhisperer-optout"),
            Some("true")
        );
        assert_eq!(header_str(&headers, "x-amzn-kiro-agent-mode"), Some("vibe"));
        assert_eq!(
            header_str(&headers, "host"),
            Some("q.eu-central-1.amazonaws.com")
        );
        assert!(header_str(&headers, "x-amz-user-agent").is_some());
        assert!(header_str(&headers, "user-agent").is_some());
        assert!(header_str(&headers, "amz-sdk-invocation-id").is_some());
    }
}
