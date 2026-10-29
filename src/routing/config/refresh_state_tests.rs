//! Absolute expiry and retry control must not reset when the Gateway restarts.
use super::*;

fn state(expiry: Option<&str>) -> TokenRefreshState {
    let credential: ProviderCredentialYaml = serde_json::from_value(serde_json::json!({
        "id":"fixture", "api_key":"fixture-access", "refresh_token":"fixture-refresh",
        "refresh_endpoint":"https://example.com/token", "token_expires_in_secs":21600,
        "expires_at":expiry
    }))
    .unwrap();
    TokenRefreshState::from_credential(&credential).unwrap()
}

#[test]
fn real_expiry_not_full_lifetime_controls_restart_deadline() {
    let expiry = future_rfc3339_after_secs(120);
    let original = state(Some(&expiry));
    let restarted = state(Some(&expiry));
    for config in [&original, &restarted] {
        assert!(config
            .refresh_snapshot(std::time::Duration::from_secs(300))
            .is_some());
        assert!(config
            .refresh_snapshot(std::time::Duration::from_secs(30))
            .is_none());
        let remaining = config
            .expires_at
            .lock()
            .saturating_duration_since(std::time::Instant::now());
        assert!(remaining.as_secs() <= 120);
        assert!(remaining.as_secs() > 100);
    }
}

#[test]
fn missing_invalid_and_past_expiry_are_due_not_extended() {
    for expiry in [None, Some("invalid"), Some("2020-01-01T08:00:00+08:00")] {
        assert!(state(expiry)
            .refresh_snapshot(std::time::Duration::ZERO)
            .is_some());
    }
}

#[test]
fn future_expiry_and_timezone_are_respected() {
    let config = state(Some("2099-01-01T08:00:00+08:00"));
    assert!(config
        .refresh_snapshot(std::time::Duration::from_secs(300))
        .is_none());
}

#[test]
fn retry_is_exponential_bounded_and_terminal_failure_blocks() {
    let config = state(None);
    for failures in 1..=9 {
        config.record_refresh_failure(0, false);
        assert!(config.refresh_snapshot(std::time::Duration::MAX).is_none());
        let mut lifecycle = config.lifecycle.lock();
        assert_eq!(lifecycle.failures, failures);
        let remaining = lifecycle
            .retry_at
            .unwrap()
            .saturating_duration_since(std::time::Instant::now());
        let expected = (60u64 << (failures - 1).min(6)).min(3600);
        assert!(remaining.as_secs() <= expected && remaining.as_secs() >= expected - 2);
        lifecycle.retry_at = None;
    }
    config.record_refresh_failure(0, true);
    config.lifecycle.lock().retry_at = None;
    assert!(config.refresh_snapshot(std::time::Duration::MAX).is_none());
    assert!(state(None)
        .refresh_snapshot(std::time::Duration::MAX)
        .is_some());
}

#[test]
fn attempt_lock_is_released_on_cancellation_and_success_clears_backoff() {
    let config = state(None);
    let guard = config.try_refresh_lock().unwrap();
    assert!(config.try_refresh_lock().is_none());
    drop(guard);
    assert!(config.try_refresh_lock().is_some());
    config.record_refresh_failure(0, false);
    assert!(config
        .apply_refresh_if_active(
            0,
            "new-access".into(),
            None,
            3600,
            future_rfc3339_after_secs(3600)
        )
        .is_some());
    let lifecycle = config.lifecycle.lock();
    assert_eq!(lifecycle.failures, 0);
    assert!(lifecycle.retry_at.is_none());
}
