use super::input::{archived_at_for_status, normalize_provider_credential_status};
use super::payload::merge_provider_account_and_credential_payloads;
use serde_json::json;
use time::OffsetDateTime;

#[test]
fn active_status_clears_archived_at() {
    let now = OffsetDateTime::now_utc();
    let archived = Some(now - time::Duration::hours(1));
    assert_eq!(archived_at_for_status("active", archived, now), None);
    assert_eq!(archived_at_for_status("cooling", archived, now), None);
    assert_eq!(archived_at_for_status("disabled", archived, now), None);
}

#[test]
fn archived_status_preserves_or_sets_timestamp() {
    let now = OffsetDateTime::now_utc();
    let existing = Some(now - time::Duration::hours(2));
    assert_eq!(archived_at_for_status("archived", existing, now), existing);
    assert_eq!(archived_at_for_status("archived", None, now), Some(now));
}

#[test]
fn normalize_provider_credential_status_uses_trimmed_value_or_fallback() {
    assert_eq!(
        normalize_provider_credential_status(Some(" active "), "archived"),
        "active"
    );
    assert_eq!(
        normalize_provider_credential_status(Some("   "), "cooling"),
        "cooling"
    );
    assert_eq!(
        normalize_provider_credential_status(None, "active"),
        "active"
    );
}

#[test]
fn merge_provider_payload_preserves_non_empty_account_values_when_overlay_is_blank() {
    let account = json!({
        "baseUrl": "https://chataibot.pro",
        "apiKey": "",
        "headers": {
            "Origin": "https://chataibot.pro"
        },
        "sessionAuth": {
            "transport": "cookie",
            "primaryCookieName": "token"
        }
    });
    let credential = json!({
        "baseUrl": "",
        "apiKey": "jwt-token",
        "headers": {
            "Cookie": "token=jwt-token"
        }
    });

    let merged = merge_provider_account_and_credential_payloads(&account, &credential);

    assert_eq!(merged["baseUrl"], "https://chataibot.pro");
    assert_eq!(merged["apiKey"], "jwt-token");
    assert_eq!(merged["headers"]["Origin"], "https://chataibot.pro");
    assert_eq!(merged["headers"]["Cookie"], "token=jwt-token");
    assert_eq!(merged["sessionAuth"]["transport"], "cookie");
}

#[test]
fn merge_provider_payload_preserves_account_base_url_when_credential_has_empty_serde_defaults() {
    let merged = merge_provider_account_and_credential_payloads(
        &json!({
            "adapter": "qwen_web_compatible",
            "baseUrl": "https://chat.qwen.ai",
            "apiKey": "",
            "headers": {
                "Origin": "https://chat.qwen.ai"
            }
        }),
        &json!({
            "apiKey": "session-token",
            "baseUrl": "",
            "headers": {
                "Cookie": "token=abc"
            }
        }),
    );

    assert_eq!(
        merged.get("baseUrl").and_then(|value| value.as_str()),
        Some("https://chat.qwen.ai")
    );
    assert_eq!(
        merged.get("apiKey").and_then(|value| value.as_str()),
        Some("session-token")
    );
    assert_eq!(
        merged
            .get("headers")
            .and_then(|value| value.get("Origin"))
            .and_then(|value| value.as_str()),
        Some("https://chat.qwen.ai")
    );
    assert_eq!(
        merged
            .get("headers")
            .and_then(|value| value.get("Cookie"))
            .and_then(|value| value.as_str()),
        Some("token=abc")
    );
}
