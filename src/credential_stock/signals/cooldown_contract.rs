use super::{signal_suppressed_by_cooldown, CredentialStockPolicyView};

fn policy(last_signal_at: Option<&str>, cooldown_seconds: i64) -> CredentialStockPolicyView {
    CredentialStockPolicyView {
        id: "policy".to_string(),
        stock_class_key: "stock".to_string(),
        display_name: "Stock".to_string(),
        service_provider_key: "provider".to_string(),
        implementation_line_key: "line".to_string(),
        provider_surface_key: "surface".to_string(),
        credential_material_kind: "session_auth".to_string(),
        provider_account_id: None,
        provider_adapter: None,
        selector: serde_json::json!({}),
        provider_supply_metric_kind: "credential_count".to_string(),
        metric_kind: "credential_count".to_string(),
        token_window_key: None,
        token_window_seconds: None,
        min_credential_count: Some(1),
        target_credential_count: Some(2),
        max_credential_count: None,
        min_average_available_tokens: None,
        target_average_available_tokens: None,
        signal_enabled: true,
        signal_stream: "gw:credential-stock:signals".to_string(),
        signal_cooldown_secs: cooldown_seconds,
        enabled: true,
        last_signal_key: Some("stable".to_string()),
        last_signal_at: last_signal_at.map(str::to_string),
        created_at: "1970-01-01T00:00:00Z".to_string(),
        updated_at: "1970-01-01T00:00:00Z".to_string(),
    }
}

#[test]
fn maximum_cooldown_keeps_matching_signal_suppressed() {
    let policy = policy(Some("1970-01-01T00:00:00Z"), i64::MAX);
    assert!(signal_suppressed_by_cooldown(&policy, "stable"));
}

#[test]
fn finite_cooldown_beyond_calendar_keeps_matching_signal_suppressed() {
    // This fits i64 and exceeds even the optional large-date calendar.
    let policy = policy(Some("1970-01-01T00:00:00Z"), 40_000_000_000_000);
    assert!(signal_suppressed_by_cooldown(&policy, "stable"));
}

#[test]
fn representable_future_deadline_stays_suppressed() {
    let policy = policy(Some("9999-01-01T00:00:00Z"), 60);
    assert!(signal_suppressed_by_cooldown(&policy, "stable"));
}

#[test]
fn expired_deadline_allows_the_signal() {
    let policy = policy(Some("1970-01-01T00:00:00Z"), 60);
    assert!(!signal_suppressed_by_cooldown(&policy, "stable"));
}

#[test]
fn negative_cooldown_retains_zero_clamping() {
    let policy = policy(Some("1970-01-01T00:00:00Z"), i64::MIN);
    assert!(!signal_suppressed_by_cooldown(&policy, "stable"));
}

#[test]
fn missing_timestamp_does_not_suppress() {
    let policy = policy(None, i64::MAX);
    assert!(!signal_suppressed_by_cooldown(&policy, "stable"));
}

#[test]
fn malformed_timestamp_does_not_suppress() {
    let policy = policy(Some("invalid-timestamp"), i64::MAX);
    assert!(!signal_suppressed_by_cooldown(&policy, "stable"));
}

#[test]
fn different_signal_key_bypasses_an_overflowing_cooldown() {
    let policy = policy(Some("1970-01-01T00:00:00Z"), i64::MAX);
    assert!(!signal_suppressed_by_cooldown(&policy, "new-signal"));
}
