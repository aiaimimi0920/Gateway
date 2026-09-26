use super::*;

#[test]
fn converts_openai_credential_without_preset() {
    let cred = make_credential(
        "cred-1",
        CredentialKind::UserOwned,
        "openai",
        Some("sk-test"),
        Some("https://api.openai.com"),
        None,
    );
    let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
    assert_eq!(candidate.provider_account_id, "cred:cred-1");
    assert_eq!(candidate.label, "credential-cred-1");
    assert_eq!(candidate.adapter, "openai_compatible");
    assert_eq!(candidate.protocol_family, OPENAI_CHAT_FAMILY);
    assert_eq!(candidate.payload.api_key, "sk-test");
    assert_eq!(candidate.payload.base_url, "https://api.openai.com");
    assert_eq!(candidate.payload.credential_id.as_deref(), Some("cred-1"));
    assert_eq!(candidate.upstream_model.as_deref(), Some("gpt-4o"));
    assert_eq!(candidate.priority, 100); // UserOwned
}

#[test]
fn converts_anthropic_credential_without_preset() {
    let cred = make_credential(
        "cred-2",
        CredentialKind::AccountCredential,
        "anthropic",
        Some("sk-ant-test"),
        Some("https://api.anthropic.com"),
        None,
    );
    let candidate = credential_to_candidate(&cred, "claude-sonnet-4-6").unwrap();
    assert_eq!(candidate.adapter, "anthropic_compatible");
    assert_eq!(candidate.protocol_family, "anthropic");
    assert_eq!(candidate.priority, 80); // AccountCredential
}

#[test]
fn converts_credential_with_codex_preset() {
    let cred = make_credential(
        "cred-3",
        CredentialKind::PlatformUnlimited,
        "codex",
        Some("tok_xxx"),
        Some("https://chatgpt.com/backend-api/codex"),
        Some(json!({
            "preset": "codex",
            "supported_models": ["gpt-5-codex"]
        })),
    );
    let candidate = credential_to_candidate(&cred, "gpt-5-codex").unwrap();
    assert_eq!(candidate.payload.adapter, "openai_compatible");
    // Codex preset injects User-Agent and Originator headers
    assert!(candidate.payload.headers.contains_key("User-Agent"));
    assert!(candidate.payload.headers.contains_key("Originator"));
    // Codex preset injects "store": false
    let extra = candidate.payload.extra_body.as_ref().unwrap();
    assert_eq!(extra.get("store"), Some(&json!(false)));
    assert_eq!(candidate.priority, 60); // PlatformUnlimited
}

#[test]
fn converts_credential_with_unknown_preset_falls_back_to_raw() {
    let cred = make_credential(
        "cred-4",
        CredentialKind::PlatformLimited,
        "openai",
        Some("sk-test"),
        Some("https://api.example.com"),
        Some(json!({
            "preset": "nonexistent-preset-xyz"
        })),
    );
    let candidate = credential_to_candidate(&cred, "gpt-4o").unwrap();
    // Should still work, falling back to raw payload
    assert_eq!(candidate.payload.adapter, "openai_compatible");
    assert_eq!(candidate.payload.api_key, "sk-test");
    assert_eq!(candidate.priority, 40); // PlatformLimited
}

#[test]
fn converts_grok_credential_to_grok_adapter() {
    let cred = make_credential(
        "cred-grok",
        CredentialKind::AccountCredential,
        "grok",
        Some("sso-token"),
        Some("https://grok.com"),
        Some(json!({
            "preset": "grok"
        })),
    );
    let candidate = credential_to_candidate(&cred, "grok-3").unwrap();
    assert_eq!(candidate.adapter, "grok_compatible");
    assert_eq!(candidate.protocol_family, "openai");
    assert_eq!(candidate.payload.adapter, "grok_compatible");
    assert_eq!(
        candidate
            .payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.primary_cookie_name()),
        Some("sso")
    );
    assert_eq!(
        candidate.payload.chat_completions_path.as_deref(),
        Some("/rest/app-chat/conversations/new")
    );
}

#[cfg(feature = "line-qwen-web-reverse")]
#[test]
fn converts_qwen_web_historical_preset_alias_to_qwen_web_candidate() {
    let cred = make_credential(
        "cred-qwen-web-alias",
        CredentialKind::AccountCredential,
        "qwen",
        Some("qwen-session"),
        Some("https://chat.qwen.ai"),
        Some(json!({
            "preset": "qwen-web"
        })),
    );
    let candidate = credential_to_candidate(&cred, "qwen-web-model").unwrap();
    assert_eq!(candidate.payload.adapter, "qwen_web_compatible");
    assert_eq!(candidate.adapter, "qwen_web_compatible");
    assert_eq!(candidate.protocol_profile, "qwen_web_chat");
    assert_eq!(candidate.protocol_family, "qwen_web_chat");
}

#[test]
fn converts_freebuff_credential_to_freebuff_adapter() {
    let cred = make_credential(
        "cred-freebuff",
        CredentialKind::AccountCredential,
        "freebuff",
        Some("fb-token"),
        Some("https://codebuff.com"),
        Some(json!({
            "extra_body": {
                "freebuffAgentId": "base2-free"
            }
        })),
    );
    let candidate = credential_to_candidate(&cred, "z-ai/glm-5.1").unwrap();
    assert_eq!(candidate.adapter, "freebuff_compatible");
    assert_eq!(candidate.protocol_family, "freebuff");
    assert_eq!(candidate.payload.adapter, "freebuff_compatible");
    assert_eq!(
        candidate
            .payload
            .extra_body
            .as_ref()
            .and_then(|extra| extra.get("freebuffAgentId"))
            .and_then(|value| value.as_str()),
        Some("base2-free")
    );
}
