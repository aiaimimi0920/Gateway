use super::*;
use serde_json::json;

#[test]
fn gemini_canvas_debug_complete_snapshot_redacts_extra_and_url_credentials() {
    let input = json!({
        "url": "https://fixture-user:fixture-pass@example.test/path?at=fixture-at&ok=yes#fixture-fragment",
        "extra": {"password": "fixture-password", "nested": "{\"token\":\"fixture-token\"}"},
        "status": 200,
        "headers": {"items": [{"name": "Cookie", "value": "fixture-cookie"}]}
    });
    let result = sanitize_debug_snapshot(&input);
    assert!(!result.to_string().contains("fixture-"));
    assert_eq!(result["status"], 200);
    assert!(result["url"].as_str().unwrap().contains("ok=yes"));
    assert!(input.to_string().contains("fixture-password"));
}

#[test]
fn gemini_canvas_debug_full_text_is_sanitized_before_preview_cut() {
    let jwt = format!("eyJ{}.{}.signature", "a".repeat(400), "b".repeat(400));
    assert_eq!(sanitize_debug_text(&jwt), "[REDACTED]");
    assert_eq!(
        sanitize_debug_text("{\"sid\":\"fixture-secret\",}"),
        OMITTED
    );
    assert_eq!(
        sanitize_debug_text(&"x".repeat(MAX_INPUT_BYTES + 1)),
        OMITTED
    );
}

#[test]
fn gemini_canvas_debug_complete_snapshot_limits_large_existing_values() {
    let input = json!({"items": vec!["x".repeat(2048); 1000]});
    let result = sanitize_debug_snapshot(&input);
    assert_eq!(result["items"].as_array().unwrap().len(), MAX_FIELDS);
    assert!(result["items"][0].as_str().unwrap().chars().count() <= 512);
    let mut nested = json!({"token": "fixture-deep"});
    for _ in 0..30 {
        nested = json!({"child": nested});
    }
    let result = sanitize_debug_snapshot(&nested);
    assert!(!result.to_string().contains("fixture-deep"));
    assert!(result.to_string().len() < 512);
}
