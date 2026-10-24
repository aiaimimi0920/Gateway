//! Balance restrictions isolate the credential, not its healthy entitlement-group siblings.
use super::*;
use crate::error::{classify_upstream_error, FallbackHint, GatewayError};

#[test]
fn balance_failure_is_level_three_account_wide_and_does_not_poison_group() {
    let runtime = ModelCircuits::default();
    let group = CircuitKey {
        provider: "site".into(),
        group: "group:shared".into(),
        model: "m".into(),
    };
    let balance = CircuitKey {
        provider: "site".into(),
        group: "balance:empty".into(),
        model: String::new(),
    };
    let healthy = CircuitKey {
        group: "balance:funded".into(),
        ..balance.clone()
    };
    let error = classify_upstream_error(403,
        "预扣费额度失败, 用户剩余额度: BTC10.895248, 需要预扣费额度: BTC32.560000 (request id: key-fixture)", Some("site"));
    assert!(matches!(
        error.fallback_hint,
        FallbackHint::FallbackProvider { .. }
    ));
    let tickets = runtime
        .acquire_at(&[group.clone(), balance.clone()], 0)
        .unwrap();
    runtime.finish_at(tickets, failure_outcome(&error), 10);
    assert_eq!(runtime.entries.lock()[&balance].level, 3);
    assert!(!runtime.available_at(&balance, 1_920_009));
    assert!(runtime.available_at(&balance, 1_920_010));
    assert!(runtime.available_at(&group, 11));
    assert!(runtime.available_at(&healthy, 11));
    let tickets = runtime.acquire_at(&[balance.clone()], 1_920_010).unwrap();
    runtime.finish_at(tickets, failure_outcome(&error), 1_920_010);
    assert_eq!(runtime.entries.lock()[&balance].level, 4);
    assert!(!runtime.available_at(&balance, 5_520_009));
    assert!(runtime.available_at(&balance, 5_520_010));
    let classification = crate::provider_failure::classify_provider_failure(
        error.http_status,
        None,
        Some(&error.message),
    );
    assert!(!classification.permanent);
    assert_eq!(
        classification.class,
        crate::provider_failure::ProviderFailureClass::QuotaExhausted
    );
}

#[test]
fn local_denial_and_arbitrary_403_do_not_trip_balance_or_authorize_replay() {
    for error in [
        GatewayError::unauthorized("Insufficient account balance"),
        crate::access_key_groups::denied(),
        classify_upstream_error(403, "预扣费额度失败: upstream policy denied", None),
        classify_upstream_error(403, "Forbidden by policy", None),
    ] {
        assert!(matches!(failure_outcome(&error), Outcome::Neutral));
        assert!(!crate::error::is_explicit_admission_rejection(&error));
    }
}
