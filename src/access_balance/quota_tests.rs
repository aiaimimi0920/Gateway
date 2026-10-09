use super::*;

fn quota(mode: &str, limit: Option<i64>) -> KeyQuotaInput {
    KeyQuotaInput {
        mode: mode.into(),
        limit,
        currency: None,
    }
}

#[test]
fn quota_rejects_invalid_modes_limits_and_overflow() {
    for input in [
        quota("unknown", None),
        quota("unlimited", Some(1)),
        quota("token_prepaid", None),
        quota("token_prepaid", Some(-1)),
        quota("message_prepaid", Some(9_007_199_254_740_992)),
    ] {
        assert!(input.validate().is_err());
    }
    assert!(quota("message_prepaid", Some(0)).validate().is_ok());
    assert!(quota("token_prepaid", Some(9_007_199_254_740_991))
        .validate()
        .is_ok());
    let mut balance = crate::access_balance::policy::unlimited("id");
    balance.total_tokens = Some(i64::MAX);
    balance.remaining_tokens = Some(-1);
    assert!(quota("token_prepaid", Some(10))
        .apply("id", Some(balance))
        .is_err());
}

#[test]
fn quota_mode_switch_does_not_reset_prior_unit_consumption_or_periods() {
    let mut balance = crate::access_balance::policy::unlimited("id");
    balance.total_messages = Some(10);
    balance.remaining_messages = Some(6);
    balance.status = "suspended".into();
    balance.period_ends_at = Some("2030-01-01T00:00:00Z".into());
    let balance = quota("unlimited", None).apply("id", Some(balance)).unwrap();
    let balance = quota("message_prepaid", Some(20))
        .apply("id", Some(balance))
        .unwrap();
    assert_eq!(balance.remaining_messages, Some(16));
    assert_eq!(balance.status, "suspended");
    assert_eq!(
        balance.period_ends_at.as_deref(),
        Some("2030-01-01T00:00:00Z")
    );
}

#[test]
fn server_disallows_untracked_paid_mode_switches_but_allows_safe_initialization() {
    let mut balance = crate::access_balance::policy::unlimited("id");
    assert!(quota("token_prepaid", Some(100))
        .validate_server_mode(Some(&balance))
        .is_ok());
    balance.balance_mode = "token_prepaid".into();
    assert!(quota("message_prepaid", Some(100))
        .validate_server_mode(Some(&balance))
        .is_err());
    assert!(quota("unlimited", None)
        .validate_server_mode(Some(&balance))
        .is_err());
    assert!(quota("token_prepaid", Some(100))
        .validate_server_mode(Some(&balance))
        .is_ok());
    balance.balance_mode = "request_prepaid".into();
    assert!(quota("message_prepaid", Some(100))
        .validate_server_mode(Some(&balance))
        .is_ok());
}
