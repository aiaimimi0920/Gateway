//! Focused routing-policy contracts, with no upstream calls or paid intelligence probes.
use neuro_gateway::routing::smart::{tier, Order, SmartRouting};

#[test]
fn four_tiers_have_non_overlapping_boundaries() {
    for (score, expected) in [
        (0.0, 0),
        (24.9, 0),
        (25.0, 1),
        (49.9, 1),
        (50.0, 2),
        (74.9, 2),
        (75.0, 3),
        (100.0, 3),
    ] {
        assert_eq!(tier(score), expected);
    }
    assert_eq!(tier(80.0), tier(84.0));
}

#[test]
fn policy_requires_a_complete_permutation() {
    assert!(serde_json::from_str::<Order>(r#"["price","availability","intelligence"]"#).is_ok());
    for value in [
        r#"["price","price","availability"]"#,
        r#"["price"]"#,
        r#"["speed","price","availability"]"#,
    ] {
        assert!(serde_json::from_str::<Order>(value).is_err());
    }
}

#[test]
fn sticky_is_last_attempt_not_last_success_and_does_not_leak_sessions() {
    let router = SmartRouting::default();
    router.begin(Some("session-a"), "model", "provider-a");
    router.outcome("provider-a", "model", true);
    router.begin(Some("session-a"), "model", "provider-b");
    router.outcome("provider-b", "model", false);
    assert_eq!(
        router.sticky(Some("session-a"), "model").as_deref(),
        Some("provider-b")
    );
    assert!(router.sticky(Some("session-b"), "model").is_none());
    router.begin(Some("session-a"), "different-model", "provider-c");
    assert!(router.sticky(Some("session-a"), "model").is_none());
}

#[test]
fn failures_reduce_only_the_affected_provider_model_and_success_recovers_gradually() {
    let router = SmartRouting::default();
    router.outcome("a", "model", false);
    router.outcome("a", "model", false);
    assert_eq!(tier(router.score("a", "model")), 2);
    assert_eq!(router.score("a", "other-model"), 100.0);
    assert_eq!(router.score("b", "model"), 100.0);
    router.outcome("a", "model", true);
    assert!(router.score("a", "model") < 81.0);
}

#[test]
fn exhausted_request_releases_sticky_but_intermediate_failure_does_not() {
    let router = SmartRouting::default();
    let request = uuid::Uuid::new_v4();
    router.begin_request(Some("session"), "model", "provider", request);
    router.outcome("provider", "model", false);
    assert_eq!(
        router.sticky(Some("session"), "model").as_deref(),
        Some("provider")
    );
    router.release_failed_request(Some("session"), "model", "provider", request);
    assert!(router.sticky(Some("session"), "model").is_none());
    assert!(router.score("provider", "model") < 86.0);
}

#[test]
fn late_failure_does_not_clear_newer_attempt_to_same_provider() {
    let router = SmartRouting::default();
    let old = uuid::Uuid::new_v4();
    let current = uuid::Uuid::new_v4();
    router.begin_request(Some("session"), "model", "provider", old);
    router.begin_request(Some("session"), "model", "provider", current);
    router.release_failed_request(Some("session"), "model", "provider", old);
    assert_eq!(
        router.sticky(Some("session"), "model").as_deref(),
        Some("provider")
    );
    router.release_failed_request(Some("other-session"), "model", "provider", current);
    router.release_failed_request(Some("session"), "other-model", "provider", current);
    assert!(router.sticky(Some("session"), "model").is_some());
    router.release_failed_request(Some("session"), "model", "provider", current);
    assert!(router.sticky(Some("session"), "model").is_none());
}
