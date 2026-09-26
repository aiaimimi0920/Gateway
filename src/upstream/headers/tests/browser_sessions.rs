use super::*;

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
fn session_auth_does_not_synthesize_headers_from_an_empty_api_key() {
    let mut payload = make_payload("suno_compatible");
    payload.api_key.clear();

    let headers = build_upstream_headers(&payload);
    assert!(headers.get("cookie").is_none());
    assert!(headers.get("authorization").is_none());

    payload
        .headers
        .insert("cookie".to_string(), "manual=session".to_string());
    let headers = build_upstream_headers(&payload);
    assert_eq!(header_str(&headers, "cookie"), Some("manual=session"));
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
