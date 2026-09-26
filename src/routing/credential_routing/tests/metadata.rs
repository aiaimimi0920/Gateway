use super::*;

#[test]
fn returns_none_when_api_key_missing() {
    let cred = make_credential(
        "cred-5",
        CredentialKind::UserOwned,
        "openai",
        None, // no api_key
        Some("https://api.openai.com"),
        None,
    );
    assert!(credential_to_candidate(&cred, "gpt-4o").is_none());
}

#[test]
fn returns_none_when_base_url_missing() {
    let cred = make_credential(
        "cred-6",
        CredentialKind::UserOwned,
        "openai",
        Some("sk-test"),
        None, // no base_url
        None,
    );
    assert!(credential_to_candidate(&cred, "gpt-4o").is_none());
}

#[test]
fn priority_user_owned_highest() {
    let user = make_credential(
        "u",
        CredentialKind::UserOwned,
        "openai",
        Some("k"),
        Some("https://a.com"),
        None,
    );
    let account = make_credential(
        "a",
        CredentialKind::AccountCredential,
        "openai",
        Some("k"),
        Some("https://a.com"),
        None,
    );
    let unlimited = make_credential(
        "p",
        CredentialKind::PlatformUnlimited,
        "openai",
        Some("k"),
        Some("https://a.com"),
        None,
    );
    let limited = make_credential(
        "l",
        CredentialKind::PlatformLimited,
        "openai",
        Some("k"),
        Some("https://a.com"),
        None,
    );

    let cu = credential_to_candidate(&user, "m").unwrap();
    let ca = credential_to_candidate(&account, "m").unwrap();
    let cp = credential_to_candidate(&unlimited, "m").unwrap();
    let cl = credential_to_candidate(&limited, "m").unwrap();

    assert!(cu.priority > ca.priority);
    assert!(ca.priority > cp.priority);
    assert!(cp.priority > cl.priority);
}

#[test]
fn extra_body_forwarded_from_credential() {
    let cred = make_credential(
        "cred-eb",
        CredentialKind::UserOwned,
        "openai",
        Some("sk-test"),
        Some("https://api.openai.com"),
        Some(json!({
            "extra_body": {"temperature": 0.7, "top_p": 0.9}
        })),
    );
    let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
    let extra = candidate.payload.extra_body.as_ref().unwrap();
    assert_eq!(extra.len(), 2);
    assert!(extra.contains_key("temperature"));
    assert!(extra.contains_key("top_p"));
}

#[test]
fn session_metadata_forwarded_from_credential_payload() {
    let cred = make_credential(
        "cred-session",
        CredentialKind::AccountCredential,
        "grok",
        Some("sso-token"),
        Some("https://grok.com"),
        Some(json!({
            "preset": "grok",
            "session_auth": {
                "transport": "cookie",
                "primary_cookie_name": "custom-sso",
                "secondary_cookie_name": "custom-sso-rw",
                "expires_at": "2099-01-01T00:00:00.000Z"
            },
            "keepalive": {
                "service_url": "http://grok-keeper:8080",
                "refresh_before_secs": 120
            }
        })),
    );
    let candidate = credential_to_candidate(&cred, "grok-3").unwrap();
    assert_eq!(
        candidate
            .payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.primary_cookie_name()),
        Some("custom-sso")
    );
    assert_eq!(
        candidate
            .payload
            .keepalive
            .as_ref()
            .map(|cfg| cfg.service_url.as_str()),
        Some("http://grok-keeper:8080")
    );
}

#[test]
fn custom_headers_forwarded_from_credential() {
    let mut headers = HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "value".to_string());
    let mut cred = make_credential(
        "cred-h",
        CredentialKind::UserOwned,
        "openai",
        Some("sk-test"),
        Some("https://api.openai.com"),
        None,
    );
    cred.headers = Some(headers);
    let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
    assert_eq!(
        candidate.payload.headers.get("X-Custom-Header"),
        Some(&"value".to_string())
    );
}
