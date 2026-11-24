//! Short-lived congestion must not inherit the old minute-scale outage ladder.
use super::*;
use crate::error::classify_upstream_error;

fn key() -> CircuitKey {
    CircuitKey {
        provider: "zzz-fixture".into(),
        group: "group:shared".into(),
        model: "m".into(),
    }
}
fn finish(runtime: &ModelCircuits, at: u64, outcome: Outcome) {
    let tickets = runtime.acquire_at(&[key()], at).unwrap();
    runtime.finish_at(tickets, outcome, at);
}
fn trip(runtime: &ModelCircuits) {
    for at in [0, 10, 20] {
        finish(runtime, at, Outcome::Failure(0));
    }
}
fn rate_limit(wait: Option<u64>) -> Outcome {
    let mut error = classify_upstream_error(
        429,
        r#"{"error":{"code":"gateway_queue_full","message":"Too many pending requests, please retry later"}}"#,
        None,
    );
    error.upstream_retry_after_ms = wait;
    failure_outcome(&error)
}

#[test]
fn transient_failure_ladder_is_ten_twenty_forty_sixty_seconds() {
    let runtime = ModelCircuits::default();
    trip(&runtime);
    let mut at = 20;
    for delay in [10_000, 20_000, 40_000, 60_000, 60_000] {
        assert!(!runtime.available_at(&key(), at + delay - 1));
        at += delay;
        assert!(runtime.available_at(&key(), at));
        finish(&runtime, at, Outcome::Failure(0));
    }
}

#[test]
fn one_complete_trial_success_restores_traffic_and_resets_failure_level() {
    let runtime = ModelCircuits::default();
    let stale = runtime.acquire_at(&[key()], 0).unwrap();
    trip(&runtime);
    finish(&runtime, 10_020, Outcome::Success);
    assert!(runtime.available_at(&key(), 10_021));
    assert_eq!(runtime.entries.lock()[&key()].level, 0);
    // A pre-trip completion must not contaminate the recovered generation.
    runtime.finish_at(stale, Outcome::Failure(0), 10_022);
    assert!(runtime.entries.lock()[&key()].failures.is_empty());
    for at in [10_030, 10_040, 10_050] {
        finish(&runtime, at, Outcome::Failure(0));
    }
    assert!(runtime.available_at(&key(), 20_050));
}

#[test]
fn queue_full_uses_thirty_seconds_even_after_previous_outages() {
    let runtime = ModelCircuits::default();
    finish(&runtime, 0, Outcome::Success);
    runtime.entries.lock().get_mut(&key()).unwrap().level = 4;
    finish(&runtime, 1, rate_limit(None));
    assert!(!runtime.available_at(&key(), 30_000));
    assert!(runtime.available_at(&key(), 30_001));
    finish(&runtime, 30_001, rate_limit(None));
    assert!(runtime.available_at(&key(), 60_001));
}

#[test]
fn rate_limit_respects_explicit_zero_short_long_and_overflow_waits() {
    for delay in [0, 1, 2_500, 600_000, 7_200_000, u64::MAX] {
        let runtime = ModelCircuits::default();
        finish(&runtime, 1, rate_limit(Some(delay)));
        let until = 1u64.saturating_add(delay);
        if delay > 0 {
            assert!(!runtime.available_at(&key(), until - 1));
        }
        let trial = runtime.acquire_at(&[key()], until).unwrap();
        assert!(runtime.acquire_at(&[key()], until).is_err());
        runtime.finish_at(trial, Outcome::Success, until);
        assert!(runtime.available_at(&key(), until));
    }
}

#[test]
fn in_flight_failure_burst_does_not_extend_cooldown_or_increment_levels() {
    let runtime = ModelCircuits::default();
    let tickets: Vec<_> = (0..20)
        .map(|_| runtime.acquire_at(&[key()], 0).unwrap())
        .collect();
    for (index, ticket) in tickets.into_iter().enumerate() {
        runtime.finish_at(ticket, Outcome::Failure(0), index as u64);
    }
    assert_eq!(runtime.entries.lock()[&key()].level, 1);
    assert_eq!(runtime.entries.lock()[&key()].in_flight, 0);
    assert!(!runtime.available_at(&key(), 10_001));
    assert!(runtime.available_at(&key(), 10_002));
}

#[test]
fn transient_trial_cancellation_releases_lease_after_five_seconds() {
    let runtime = ModelCircuits::default();
    trip(&runtime);
    finish(&runtime, 10_020, Outcome::Neutral);
    assert!(!runtime.available_at(&key(), 15_019));
    assert!(runtime.available_at(&key(), 15_020));
    assert_eq!(runtime.entries.lock()[&key()].level, 1);
}

#[test]
fn persisted_rate_limit_default_matches_short_hard_circuit_wait() {
    use crate::provider_failure::classify_provider_failure;
    use crate::provider_health_state::transition_for_failure_with_retry_after;
    let classification = classify_provider_failure(Some(429), Some("gateway_queue_full"), None);
    for (hint, expected) in [
        (None, 30),
        (Some(0), 0),
        (Some(2_500), 3),
        (Some(600_000), 600),
    ] {
        assert_eq!(
            transition_for_failure_with_retry_after(&classification, hint).cooldown_seconds,
            Some(expected)
        );
    }
}

#[test]
fn rate_limit_does_not_poison_balance_or_provider_scope() {
    let runtime = ModelCircuits::default();
    let balance = CircuitKey {
        group: "balance:account".into(),
        model: String::new(),
        ..key()
    };
    let provider = CircuitKey {
        group: "provider:availability".into(),
        model: String::new(),
        ..key()
    };
    let tickets = runtime
        .acquire_at(&[key(), balance.clone(), provider.clone()], 0)
        .unwrap();
    runtime.finish_at(tickets, rate_limit(None), 1);
    assert!(!runtime.available_at(&key(), 2));
    for scope in [balance, provider] {
        assert!(runtime.available_at(&scope, 2));
        assert_eq!(runtime.entries.lock()[&scope].level, 0);
    }
}

#[test]
fn explicit_restriction_keeps_conservative_cancellation_and_stable_recovery() {
    let runtime = ModelCircuits::default();
    finish(&runtime, 0, Outcome::ChannelUnavailable(0));
    finish(&runtime, 1_920_000, Outcome::Neutral);
    assert!(!runtime.available_at(&key(), 1_949_999));
    finish(&runtime, 1_950_000, Outcome::Failure(0));
    assert!(!runtime.available_at(&key(), 5_549_999));
    finish(&runtime, 5_550_000, Outcome::Success);
    assert!(!runtime.available_at(&key(), 5_579_999));
    finish(&runtime, 5_580_000, Outcome::Success);
    assert_eq!(runtime.entries.lock()[&key()].level, 4);
    finish(&runtime, 7_380_000, Outcome::Success);
    assert_eq!(runtime.entries.lock()[&key()].level, 0);
    assert!(!runtime.entries.lock()[&key()].slow_recovery);
}
