//! ChatGPT session, Codex and payload-merge contracts.
use super::super::normalization::normalize_import_payload;
use super::build_test_provider_account;
use serde_json::Value;

#[test]
fn normalize_chatgpt_web_import_payload_accepts_easyregister_failed_twice_shape() {
    let mut provider = build_test_provider_account(
        "ChatGPT Web Reverse",
        "chatgpt_web_reverse_compatible",
        "chatgpt_web_chat",
        "chatgpt_web_reverse",
        "https://chatgpt.com/backend-api/conversation",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    provider.service_provider_key = "chatgpt_platform".to_string();
    provider.service_provider_label = "ChatGPT Platform".to_string();

    let payload = normalize_import_payload(
        "chatgpt-web-reverse",
        &provider,
        serde_json::json!({
            "email": "fixture@example.com",
            "password": "fixture-password-123",
            "platformAuth": {
                "deviceId": "platform-device-id-1"
            },
            "chatgptLogin": {
                "deviceId": "chatgpt-login-device-id-1",
                "authUrl": "https://auth.openai.com/api/accounts/authorize?client_id=fixture",
                "mailboxRef": "moemail:fixture-session",
                "mailboxSessionId": "fixture-session"
            },
            "chatgptLoginDetails": {
                "oauthTokens": {
                    "refresh_token": "fixture-refresh-token",
                    "id_token": "fixture-id-token"
                },
                "clientBootstrap": {
                    "planType": "free",
                    "accessToken": "eyJhbGciOiJub25lIn0.eyJleHAiOjQxMDAwMDAwMDAsInNlc3Npb25faWQiOiJhdXRoc2Vzc19jaGF0Z3B0X2ZpeHR1cmUifQ."
                }
            }
        }),
        Some("session_auth"),
    )
    .expect("normalized chatgpt web payload from easyregister shape");
    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("eyJhbGciOiJub25lIn0.eyJleHAiOjQxMDAwMDAwMDAsInNlc3Npb25faWQiOiJhdXRoc2Vzc19jaGF0Z3B0X2ZpeHR1cmUifQ.")
    );
    assert_eq!(
        payload.get("baseUrl").and_then(Value::as_str),
        Some("https://chatgpt.com")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("deviceId"))
            .and_then(Value::as_str),
        Some("chatgpt-login-device-id-1")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("chatgptAuthUrl"))
            .and_then(Value::as_str),
        Some("https://auth.openai.com/api/accounts/authorize?client_id=fixture")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("mailboxRef"))
            .and_then(Value::as_str),
        Some("moemail:fixture-session")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("mailboxSessionId"))
            .and_then(Value::as_str),
        Some("fixture-session")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("authSeed"))
            .and_then(Value::as_object)
            .and_then(|seed| seed.get("email"))
            .and_then(Value::as_str),
        Some("fixture@example.com")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("authSeed"))
            .and_then(Value::as_object)
            .and_then(|seed| seed.get("password"))
            .and_then(Value::as_str),
        Some("fixture-password-123")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("sessionId"))
            .and_then(Value::as_str),
        Some("authsess_chatgpt_fixture")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("refreshToken"))
            .and_then(Value::as_str),
        Some("fixture-refresh-token")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("refreshStrategy"))
            .and_then(Value::as_str),
        Some("oauth_token")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("idToken"))
            .and_then(Value::as_str),
        Some("fixture-id-token")
    );
    assert_eq!(
        payload.get("expiresAt").and_then(Value::as_str),
        Some("2099-12-03T16:53:20Z")
    );
    assert_eq!(
        payload
            .get("credentialMaterialKind")
            .and_then(Value::as_str),
        Some("session_auth")
    );
}

#[test]
fn normalize_chatgpt_codex_backend_import_payload_uses_chatgpt_surface_slug() {
    let mut provider = build_test_provider_account(
        "ChatGPT Codex Backend",
        "openai_compatible",
        "openai",
        "chatgpt_codex_backend",
        "https://chatgpt.com/backend-api/codex",
        Some("official_vendor_api"),
        None,
    );
    provider.service_provider_key = "chatgpt_platform".to_string();
    provider.service_provider_label = "ChatGPT Platform".to_string();

    let payload = normalize_import_payload(
        "chatgpt-codex-backend",
        &provider,
        serde_json::json!({
            "access_token": "codex-token-123",
            "account_id": "acct-123"
        }),
        None,
    )
    .expect("normalized chatgpt codex payload");

    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("codex-token-123")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Originator"))
            .and_then(Value::as_str),
        Some("codex_cli_rs")
    );
}

#[test]
fn normalize_chatgpt_codex_backend_import_payload_accepts_easyregister_failed_twice_shape() {
    let mut provider = build_test_provider_account(
        "ChatGPT Codex Backend",
        "openai_compatible",
        "openai",
        "chatgpt_codex_backend",
        "https://chatgpt.com/backend-api/codex",
        Some("official_vendor_api"),
        None,
    );
    provider.service_provider_key = "chatgpt_platform".to_string();
    provider.service_provider_label = "ChatGPT Platform".to_string();

    let payload = normalize_import_payload(
        "chatgpt-codex-backend",
        &provider,
        serde_json::json!({
            "email": "fixture@example.com",
            "chatgptLogin": {
                "workspaceId": "workspace-id-1"
            },
            "chatgptLoginDetails": {
                "clientBootstrap": {
                    "accountId": "account-id-1",
                    "accessToken": "codex-token-from-easyregister"
                }
            }
        }),
        None,
    )
    .expect("normalized chatgpt codex payload from easyregister shape");

    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("codex-token-from-easyregister")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Chatgpt-Account-Id"))
            .and_then(Value::as_str),
        Some("account-id-1")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Originator"))
            .and_then(Value::as_str),
        Some("codex_cli_rs")
    );
}

#[test]
fn normalize_chatgpt_codex_backend_payload_merges_without_duplicate_base_url_fields() {
    let mut provider = build_test_provider_account(
        "ChatGPT Codex Backend",
        "openai_compatible",
        "openai",
        "chatgpt_codex_backend",
        "https://chatgpt.com/backend-api/codex",
        Some("official_vendor_api"),
        None,
    );
    provider.service_provider_key = "chatgpt_platform".to_string();
    provider.service_provider_label = "ChatGPT Platform".to_string();

    let credential_payload = normalize_import_payload(
        "chatgpt-codex-backend",
        &provider,
        serde_json::json!({
            "access_token": "codex-token-123",
            "account_id": "acct-123"
        }),
        None,
    )
    .expect("normalized chatgpt codex payload");

    let merged = crate::db::merge_provider_account_and_credential_payloads(
        &provider.payload,
        &credential_payload,
    );
    let serde_compatible =
        crate::http::routes::internal_provider_accounts::make_provider_payload_serde_compatible(
            &provider, &merged,
        );

    let parsed = serde_json::from_value::<crate::routing::candidate::ProviderAccountPayload>(
        serde_compatible,
    );
    assert!(
        parsed.is_ok(),
        "merged codex payload should deserialize cleanly, got: {parsed:?}"
    );
}
