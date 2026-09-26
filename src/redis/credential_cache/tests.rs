use super::*;

#[test]
fn test_compute_ttl_seconds_none_when_no_expiry() {
    assert!(compute_ttl_seconds(None).is_none());
}

#[test]
fn test_compute_ttl_seconds_past_returns_none() {
    // A date well in the past.
    assert!(compute_ttl_seconds(Some("2000-01-01T00:00:00.000Z")).is_none());
}

#[test]
fn test_compute_ttl_seconds_future_returns_some() {
    // A date far in the future.
    let ttl = compute_ttl_seconds(Some("2099-01-01T00:00:00.000Z"));
    assert!(ttl.is_some());
    assert!(ttl.unwrap() > 0);
}

#[test]
fn test_credential_kind_priority_ordering() {
    use CredentialKind::*;
    assert!(UserOwned.priority() < AccountCredential.priority());
    assert!(AccountCredential.priority() < PlatformUnlimited.priority());
    assert!(PlatformUnlimited.priority() < PlatformLimited.priority());
}

#[test]
fn test_credential_entry_roundtrip_json() {
    let entry = CredentialEntry {
        id: "cred-1".to_string(),
        kind: CredentialKind::UserOwned,
        project_id: "proj-1".to_string(),
        user_id: "user-1".to_string(),
        provider: "openai".to_string(),
        api_key: Some("sk-test".to_string()),
        api_base_url: None,
        headers: None,
        account_payload: None,
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: None,
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    };
    let s = serde_json::to_string(&entry).unwrap();
    let back: CredentialEntry = serde_json::from_str(&s).unwrap();
    assert_eq!(back.id, entry.id);
    assert_eq!(back.kind, entry.kind);
    assert_eq!(back.api_key, entry.api_key);
}

// ── supports_model tests ──────────────────────────────────────────────

fn make_entry_with_models(models: Vec<&str>) -> CredentialEntry {
    let payload = if models.is_empty() {
        None
    } else {
        Some(serde_json::json!({
            "supported_models": models,
        }))
    };
    CredentialEntry {
        id: "cred-m".to_string(),
        kind: CredentialKind::UserOwned,
        project_id: "proj-1".to_string(),
        user_id: "user-1".to_string(),
        provider: "openai".to_string(),
        api_key: Some("sk-test".to_string()),
        api_base_url: Some("https://api.openai.com".to_string()),
        headers: None,
        account_payload: payload,
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: None,
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    }
}

#[test]
fn supports_model_exact_match() {
    let entry = make_entry_with_models(vec!["gpt-4o", "gpt-4o-mini"]);
    assert!(entry.supports_model("gpt-4o"));
    assert!(entry.supports_model("gpt-4o-mini"));
    assert!(!entry.supports_model("claude-sonnet"));
}

#[test]
fn supports_model_glob_match() {
    let entry = make_entry_with_models(vec!["gpt-*", "claude-*"]);
    assert!(entry.supports_model("gpt-4o"));
    assert!(entry.supports_model("gpt-5-codex"));
    assert!(entry.supports_model("claude-sonnet-4-6"));
    assert!(!entry.supports_model("llama-3"));
}

#[test]
fn supports_model_empty_list_matches_any() {
    // No account_payload at all
    let entry = make_entry_with_models(vec![]);
    assert!(entry.supports_model("anything"));
    assert!(entry.supports_model("gpt-4o"));

    // account_payload with no supported_models key
    let mut entry2 = make_entry_with_models(vec![]);
    entry2.account_payload = Some(serde_json::json!({}));
    assert!(entry2.supports_model("anything"));
}

#[test]
fn preset_name_extraction() {
    let entry = CredentialEntry {
        id: "c".to_string(),
        kind: CredentialKind::UserOwned,
        project_id: "p".to_string(),
        user_id: "u".to_string(),
        provider: "openai".to_string(),
        api_key: None,
        api_base_url: None,
        headers: None,
        account_payload: Some(serde_json::json!({"preset": "codex"})),
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: None,
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    };
    assert_eq!(entry.preset_name(), Some("codex"));
}

#[test]
fn extra_body_fields_extraction() {
    let entry = CredentialEntry {
        id: "c".to_string(),
        kind: CredentialKind::UserOwned,
        project_id: "p".to_string(),
        user_id: "u".to_string(),
        provider: "openai".to_string(),
        api_key: None,
        api_base_url: None,
        headers: None,
        account_payload: Some(serde_json::json!({
            "extra_body": {"store": false, "max_tokens": 1024}
        })),
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: None,
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    };
    let eb = entry.extra_body_fields();
    assert_eq!(eb.get("store"), Some(&serde_json::json!(false)));
    assert_eq!(eb.get("max_tokens"), Some(&serde_json::json!(1024)));
}

#[test]
fn session_auth_and_keepalive_extraction() {
    let entry = CredentialEntry {
        id: "c".to_string(),
        kind: CredentialKind::AccountCredential,
        project_id: "p".to_string(),
        user_id: "u".to_string(),
        provider: "grok".to_string(),
        api_key: Some("sso-token".to_string()),
        api_base_url: Some("https://grok.com".to_string()),
        headers: None,
        account_payload: Some(serde_json::json!({
            "session_auth": {
                "transport": "cookie",
                "primary_cookie_name": "sso",
                "secondary_cookie_name": "sso-rw"
            },
            "keepalive": {
                "service_url": "http://grok-keeper:8080",
                "refresh_before_secs": 120
            }
        })),
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    };
    let session = entry.session_auth().expect("session_auth");
    assert_eq!(session.primary_cookie_name(), "sso");
    let keepalive = entry.keepalive_config().expect("keepalive");
    assert_eq!(keepalive.service_url, "http://grok-keeper:8080");
    assert_eq!(keepalive.refresh_before_secs(), 120);
}

#[test]
fn account_payload_runtime_extraction_supports_platform_camel_case() {
    let entry = CredentialEntry {
        id: "platform-camel".to_string(),
        kind: CredentialKind::AccountCredential,
        project_id: "p".to_string(),
        user_id: "u".to_string(),
        provider: "chatgpt-web".to_string(),
        api_key: Some("session-token".to_string()),
        api_base_url: Some("https://chatgpt.com".to_string()),
        headers: None,
        account_payload: Some(serde_json::json!({
            "extraBody": {
                "credentialMaterialKey": "chatgpt-session-main",
                "appUrl": "https://chatgpt.com"
            },
            "sessionAuth": {
                "transport": "bearer",
                "headerName": "authorization"
            },
            "keepalive": {
                "serviceUrl": "http://keeper:8080",
                "refreshBeforeSecs": 120
            }
        })),
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    };

    let extra_body = entry.extra_body_fields();
    assert_eq!(
        extra_body.get("credentialMaterialKey"),
        Some(&serde_json::json!("chatgpt-session-main"))
    );
    let session = entry.session_auth().expect("session_auth");
    assert_eq!(session.transport, "bearer");
    assert_eq!(session.header_name(), Some("authorization"));
    let keepalive = entry.keepalive_config().expect("keepalive");
    assert_eq!(keepalive.service_url, "http://keeper:8080");
    assert_eq!(keepalive.refresh_before_secs(), 120);
}

#[test]
fn browser_runtime_metadata_extraction_supports_snake_and_camel_case() {
    let entry = CredentialEntry {
        id: "canvas-cred".to_string(),
        kind: CredentialKind::AccountCredential,
        project_id: "p".to_string(),
        user_id: "u".to_string(),
        provider: "gemini-canvas".to_string(),
        api_key: Some(String::new()),
        api_base_url: Some("https://gemini.google.com".to_string()),
        headers: None,
        account_payload: Some(serde_json::json!({
            "runtimeStateObjectKey": "objects/gemini-canvas/auth-1.json",
            "account_name": "canvas-main"
        })),
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: Some("2099-01-01T00:00:00.000Z".to_string()),
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    };

    assert_eq!(
        entry.runtime_state_object_key(),
        Some("objects/gemini-canvas/auth-1.json")
    );
    assert_eq!(entry.account_name(), Some("canvas-main"));
}

#[tokio::test]
#[ignore = "requires live Redis"]
async fn test_set_get_delete_credential() {
    let pool = crate::redis::pool::create_pool("redis://127.0.0.1:6379").unwrap();
    let entry = CredentialEntry {
        id: "test-cred-integration".to_string(),
        kind: CredentialKind::UserOwned,
        project_id: "proj-x".to_string(),
        user_id: "user-x".to_string(),
        provider: "openai".to_string(),
        api_key: Some("sk-test".to_string()),
        api_base_url: None,
        headers: None,
        account_payload: None,
        quota_total_tokens: None,
        quota_remaining_tokens: None,
        expires_at: None,
        created_at: "2024-01-01T00:00:00.000Z".to_string(),
        updated_at: "2024-01-01T00:00:00.000Z".to_string(),
    };
    set_credential(&pool, &entry).await.unwrap();
    let got = get_credential(&pool, &entry.id).await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().id, entry.id);
    delete_credential(&pool, &entry.id).await.unwrap();
    let gone = get_credential(&pool, &entry.id).await.unwrap();
    assert!(gone.is_none());
}
