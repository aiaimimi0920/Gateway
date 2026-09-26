//! Browser and account-worker import payload contracts.
use super::super::normalization::normalize_import_payload;
use super::build_test_provider_account;
use serde_json::Value;

#[test]
fn normalize_qwen_web_import_payload_converts_browser_worker_output() {
    let provider = build_test_provider_account(
        "Qwen WebUI Replay Live",
        "qwen_web_compatible",
        "qwen_web_chat",
        "qwen_web_chat",
        "https://chat.qwen.ai",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    let payload = normalize_import_payload(
        "qwen-web-chat",
        &provider,
        serde_json::json!({
            "authToken": "token-123",
            "cookieHeader": "token=token-123; other=1",
            "expiresAt": "2099-01-01T00:00:00.000Z",
            "selectedModel": "qwen3-coder-plus",
            "selectedDisplayModel": "Qwen3-Coder",
            "authProbe": {
                "userId": "user-123",
                "email": "user@example.com"
            }
        }),
        None,
    )
    .expect("normalized qwen web payload");
    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("token-123")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Cookie"))
            .and_then(Value::as_str),
        Some("token=token-123; other=1")
    );
    assert_eq!(
        payload
            .get("supportedModels")
            .and_then(Value::as_array)
            .and_then(|models| models.first())
            .and_then(Value::as_str),
        Some("qwen3-coder-plus")
    );
    assert_eq!(
        payload.get("selectedDisplayModel").and_then(Value::as_str),
        Some("Qwen3-Coder")
    );
    assert_eq!(
        payload.get("credentialMaterialKey").and_then(Value::as_str),
        Some("qwen-web-user:user-123")
    );
    assert_eq!(
        payload.get("serviceProviderKey").and_then(Value::as_str),
        Some("qwen_platform")
    );
    assert_eq!(
        payload.get("providerSurfaceKey").and_then(Value::as_str),
        Some("qwen-web-chat")
    );
    assert_eq!(
        payload
            .get("credentialMaterialKind")
            .and_then(Value::as_str),
        Some("session_auth")
    );
    assert_eq!(
        payload.get("accountName").and_then(Value::as_str),
        Some("user@example.com")
    );
}

#[test]
fn normalize_qwen_web_import_payload_preserves_auth_seed() {
    let provider = build_test_provider_account(
        "Qwen WebUI Replay Live",
        "qwen_web_compatible",
        "qwen_web_chat",
        "qwen_web_chat",
        "https://chat.qwen.ai",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    let payload = normalize_import_payload(
        "qwen-web-chat",
        &provider,
        serde_json::json!({
            "authToken": "token-123",
            "email": "user@example.com",
            "passwordSha256": "abcdef123456"
        }),
        None,
    )
    .expect("normalized qwen web payload with auth seed");
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|body| body.get("authSeed"))
            .and_then(Value::as_object)
            .and_then(|seed| seed.get("email"))
            .and_then(Value::as_str),
        Some("user@example.com")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|body| body.get("authSeed"))
            .and_then(Value::as_object)
            .and_then(|seed| seed.get("passwordSha256"))
            .and_then(Value::as_str),
        Some("abcdef123456")
    );
}

#[test]
fn normalize_chataibot_import_payload_accepts_manual_token_source() {
    let mut provider = build_test_provider_account(
        "ChatAIBot Images",
        "chataibot_compatible",
        "chataibot_images",
        "chataibot",
        "https://chataibot.pro",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    provider.service_provider_key = "chataibot_platform".to_string();
    provider.service_provider_label = "ChatAIBot".to_string();
    provider.payload = serde_json::json!({
        "baseUrl": "https://chataibot.pro",
        "defaultModel": "google-nano-banana-2"
    });

    let payload = normalize_import_payload(
        "chataibot-images",
        &provider,
        serde_json::json!({
            "authToken": "token-456",
            "cookieHeader": "cf_clearance=abc; locale=en",
            "selectedModel": "google-nano-banana-2",
            "accountName": "artist@example.com",
            "userId": "user-456",
            "expiresAt": "2099-01-01T00:00:00.000Z"
        }),
        None,
    )
    .expect("normalized chataibot payload from manual token source");

    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("token-456")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Cookie"))
            .and_then(Value::as_str),
        Some("token=token-456; cf_clearance=abc; locale=en")
    );
    assert_eq!(
        payload
            .get("supportedModels")
            .and_then(Value::as_array)
            .and_then(|models| models.first())
            .and_then(Value::as_str),
        Some("google-nano-banana-2")
    );
    assert_eq!(
        payload.get("accountName").and_then(Value::as_str),
        Some("artist@example.com")
    );
    assert_eq!(
        payload.get("credentialMaterialKey").and_then(Value::as_str),
        Some("chataibot-user:user-456")
    );
    assert_eq!(
        payload.get("providerSurfaceKey").and_then(Value::as_str),
        Some("chataibot-images")
    );
    assert_eq!(
        payload
            .get("credentialMaterialKind")
            .and_then(Value::as_str),
        Some("session_auth")
    );
    assert_eq!(
        payload.get("serviceProviderKey").and_then(Value::as_str),
        Some("chataibot_platform")
    );
}

#[test]
fn normalize_chataibot_import_payload_accepts_session_worker_output_shape() {
    let mut provider = build_test_provider_account(
        "ChatAIBot Images",
        "chataibot_compatible",
        "chataibot_images",
        "chataibot",
        "https://chataibot.pro",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    provider.service_provider_key = "chataibot_platform".to_string();
    provider.service_provider_label = "ChatAIBot".to_string();
    provider.payload = serde_json::json!({
        "baseUrl": "https://chataibot.pro",
        "defaultModel": "qwen-lora"
    });

    let payload = normalize_import_payload(
        "chataibot-images",
        &provider,
        serde_json::json!({
            "apiKey": "token-123",
            "headers": {
                "Cookie": "token=token-123; cf_clearance=abc"
            },
            "supportedModels": ["qwen-lora", "google-nano-banana-2"],
            "selectedDisplayModel": "ChatAIBot Free Images",
            "accountName": "artist@example.com",
            "credentialMaterialKey": "chataibot-user:user-123",
            "expiresAt": "2099-01-01T00:00:00.000Z"
        }),
        None,
    )
    .expect("normalized chataibot payload from worker output");

    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("token-123")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Cookie"))
            .and_then(Value::as_str),
        Some("token=token-123; cf_clearance=abc")
    );
    assert_eq!(
        payload
            .get("supportedModels")
            .and_then(Value::as_array)
            .map(|models| { models.iter().filter_map(Value::as_str).collect::<Vec<_>>() }),
        Some(vec!["qwen-lora", "google-nano-banana-2"])
    );
    assert_eq!(
        payload.get("selectedDisplayModel").and_then(Value::as_str),
        Some("ChatAIBot Free Images")
    );
    assert_eq!(
        payload.get("credentialMaterialKey").and_then(Value::as_str),
        Some("chataibot-user:user-123")
    );
    assert_eq!(
        payload
            .get("credentialMaterialKind")
            .and_then(Value::as_str),
        Some("session_auth")
    );
}

#[test]
fn normalize_gemini_web_import_payload_converts_cookie_session_source() {
    let mut provider = build_test_provider_account(
        "Gemini Web Chat",
        "gemini_web_compatible",
        "gemini_web_chat",
        "gemini_web",
        "https://gemini.google.com",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    provider.service_provider_key = "gemini_platform".to_string();
    provider.service_provider_label = "Gemini Platform".to_string();

    let payload = normalize_import_payload(
        "gemini-web-chat",
        &provider,
        serde_json::json!({
            "__Secure-1PSID": "psid-primary",
            "__Secure-1PSIDTS": "psidts-secondary",
            "cookieHeader": "__Secure-1PSID=psid-primary; __Secure-1PSIDTS=psidts-secondary; NID=test",
            "defaultModel": "gemini-web-chat-live",
            "accessToken": "snlm0e-token",
            "buildLabel": "boq-gemini",
            "sessionId": "fsid-1",
            "language": "en",
            "requestContextHeader": "[\"ctx\",1]",
            "credentialMaterialKey": "gemini-web-session-main"
        }),
        Some("session_auth"),
    )
    .expect("normalized gemini web payload");

    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("psid-primary")
    );
    assert_eq!(
        payload.get("authToken").and_then(Value::as_str),
        Some("psidts-secondary")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Cookie"))
            .and_then(Value::as_str),
        Some("__Secure-1PSID=psid-primary; __Secure-1PSIDTS=psidts-secondary; NID=test")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("accessToken"))
            .and_then(Value::as_str),
        Some("snlm0e-token")
    );
    assert_eq!(
        payload.get("providerSurfaceKey").and_then(Value::as_str),
        Some("gemini-web-chat")
    );
    assert_eq!(
        payload
            .get("credentialMaterialKind")
            .and_then(Value::as_str),
        Some("session_auth")
    );
}

#[test]
fn normalize_accio_import_payload_converts_manager_account_file() {
    let provider = build_test_provider_account(
        "Accio Live",
        "accio_compatible",
        "accio",
        "openai_responses",
        "https://phoenix-gw.alibaba.com",
        Some("web_reverse_api"),
        Some("direct_http_replay"),
    );
    let payload = normalize_import_payload(
        "accio",
        &provider,
        serde_json::json!({
            "id": "acct-123",
            "name": "Accio Team",
            "email": "accio@example.com",
            "accessToken": "accio-token-123",
            "refreshToken": "refresh-123",
            "utdid": "utd-accio-123",
            "cookie": "cna=test-cna; other=value",
            "disabledModels": {
                "claude-opus-4-6": "quota_empty"
            },
            "expiresAt": "2099-01-01T00:00:00.000Z"
        }),
        None,
    )
    .expect("normalized accio payload");
    assert_eq!(
        payload.get("apiKey").and_then(Value::as_str),
        Some("accio-token-123")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("utdid"))
            .and_then(Value::as_str),
        Some("utd-accio-123")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("x-cna"))
            .and_then(Value::as_str),
        Some("test-cna")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("token"))
            .and_then(Value::as_str),
        Some("accio-token-123")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("accessToken"))
            .and_then(Value::as_str),
        Some("accio-token-123")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("empid"))
            .and_then(Value::as_str),
        Some("acct-123")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("tenant"))
            .and_then(Value::as_str),
        Some("")
    );
    assert_eq!(
        payload
            .get("extraBody")
            .and_then(Value::as_object)
            .and_then(|extra| extra.get("iaiTag"))
            .and_then(Value::as_str),
        Some("phoenix-desktop")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("Cookie"))
            .and_then(Value::as_str),
        Some("cna=test-cna; other=value")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("x-language"))
            .and_then(Value::as_str),
        Some("zh-CN")
    );
    assert_eq!(
        payload
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("x-os"))
            .and_then(Value::as_str),
        Some("win32")
    );
    assert_eq!(
        payload
            .get("excludedModels")
            .and_then(Value::as_array)
            .and_then(|models| models.first())
            .and_then(Value::as_str),
        Some("claude-opus-4-6")
    );
    assert_eq!(
        payload.get("credentialMaterialKey").and_then(Value::as_str),
        Some("accio-account:acct-123")
    );
    assert_eq!(
        payload.get("accountName").and_then(Value::as_str),
        Some("accio@example.com")
    );
}
