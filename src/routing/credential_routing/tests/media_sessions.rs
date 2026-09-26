use super::*;

#[test]
fn converts_gemini_business_credential_to_custom_image_adapter() {
    let cred = make_credential(
        "cred-gemini-business",
        CredentialKind::AccountCredential,
        "gemini-business",
        Some("jwt-token"),
        Some("https://biz-discoveryengine.googleapis.com/v1alpha"),
        Some(json!({
            "preset": "gemini-business",
            "extra_body": {
                "configId": "cfg-123",
                "session": "projects/demo/sessions/123"
            }
        })),
    );
    let candidate = credential_to_candidate(&cred, "nano-banana-pro").unwrap();
    assert_eq!(candidate.adapter, "gemini_business_compatible");
    assert_eq!(candidate.protocol_family, "gemini_business_images");
    assert_eq!(
        candidate
            .payload
            .session_auth
            .as_ref()
            .and_then(|cfg| cfg.header_name()),
        Some("Authorization")
    );
    let extra = candidate.payload.extra_body.as_ref().unwrap();
    assert_eq!(extra.get("configId"), Some(&json!("cfg-123")));
    assert_eq!(
        extra.get("session"),
        Some(&json!("projects/demo/sessions/123"))
    );
}

#[test]
fn converts_chataibot_credential_to_cookie_image_adapter() {
    let cred = make_credential(
        "cred-chataibot",
        CredentialKind::AccountCredential,
        "chataibot.pro",
        Some("jwt-token"),
        Some("https://chataibot.pro"),
        Some(json!({
            "preset": "chataibot"
        })),
    );
    let candidate = credential_to_candidate(&cred, "google-nano-banana").unwrap();
    assert_eq!(candidate.adapter, "chataibot_compatible");
    assert_eq!(candidate.protocol_family, "chataibot_images");
    assert_eq!(
        candidate
            .payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.primary_cookie_name()),
        Some("token")
    );
    assert_eq!(
        candidate
            .payload
            .session_auth
            .as_ref()
            .and_then(|cfg| cfg.secondary_cookie_name()),
        None
    );
}

#[test]
fn converts_lumalabs_credential_to_cookie_image_adapter() {
    let cred = make_credential(
        "cred-lumalabs",
        CredentialKind::AccountCredential,
        "lumalabs.ai",
        Some("wos-session-token"),
        Some("https://app.lumalabs.ai"),
        Some(json!({
            "preset": "lumalabs",
            "extra_body": {
                "realmId": "4675f69b-4aa5-4b25-bfc7-c480d8efd537"
            }
        })),
    );
    let candidate = credential_to_candidate(&cred, "uni-1").unwrap();
    assert_eq!(candidate.adapter, "lumalabs_compatible");
    assert_eq!(candidate.protocol_family, "lumalabs_images");
    assert_eq!(
        candidate
            .payload
            .session_auth
            .as_ref()
            .map(|cfg| cfg.primary_cookie_name()),
        Some("wos-session")
    );
    assert_eq!(
        candidate
            .payload
            .extra_body
            .as_ref()
            .and_then(|extra| extra.get("realmId")),
        Some(&json!("4675f69b-4aa5-4b25-bfc7-c480d8efd537"))
    );
}

#[test]
fn lumalabs_video_model_hint_maps_to_same_adapter_family() {
    let cred = make_credential(
        "cred-lumalabs-video",
        CredentialKind::AccountCredential,
        "ray-2",
        Some("wos-session-token"),
        Some("https://app.lumalabs.ai"),
        Some(json!({
            "preset": "lumalabs",
            "extra_body": {
                "realmId": "4675f69b-4aa5-4b25-bfc7-c480d8efd537"
            }
        })),
    );
    let candidate = credential_to_candidate(&cred, "ray-2").unwrap();
    assert_eq!(candidate.adapter, "lumalabs_compatible");
    assert_eq!(candidate.protocol_family, "lumalabs_images");
    assert_eq!(candidate.protocol_profile, "lumalabs");
}

#[test]
fn gemini_canvas_provider_maps_to_browser_state_adapter_and_runtime_key() {
    let cred = make_credential(
        "cred-canvas",
        CredentialKind::AccountCredential,
        "gemini-canvas",
        Some(""),
        Some("https://gemini.google.com"),
        Some(json!({
            "runtime_state_object_key": "objects/gemini-canvas/auth-1.json",
            "account_name": "canvas-main"
        })),
    );
    let candidate = credential_to_candidate(&cred, "gemini-3-flash-preview").unwrap();
    assert_eq!(candidate.adapter, "gemini_canvas_compatible");
    assert_eq!(candidate.protocol_family, "gemini_canvas_images");
    assert_eq!(
        candidate.payload.runtime_state_object_key.as_deref(),
        Some("objects/gemini-canvas/auth-1.json")
    );
    assert_eq!(
        candidate.payload.account_name.as_deref(),
        Some("canvas-main")
    );
}

#[test]
fn gemini_canvas_provider_allows_missing_api_key_when_runtime_state_exists() {
    let cred = make_credential(
        "cred-canvas-no-key",
        CredentialKind::AccountCredential,
        "gemini-canvas",
        None,
        Some("https://gemini.google.com"),
        Some(json!({
            "runtime_state_object_key": "objects/gemini-canvas/auth-2.json"
        })),
    );
    let candidate = credential_to_candidate(&cred, "gemini-2.5-flash-image-preview")
        .expect("gemini canvas should not require api_key");
    assert_eq!(candidate.payload.api_key, "");
    assert_eq!(
        candidate.payload.runtime_state_object_key.as_deref(),
        Some("objects/gemini-canvas/auth-2.json")
    );
}
