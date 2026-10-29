use super::*;
use crate::error::classify_upstream_error;

fn key(account: &str, model: &str) -> CircuitKey {
    CircuitKey {
        provider: "chatgpt".into(),
        group: account.into(),
        model: model.into(),
    }
}
fn refusal() -> GatewayError {
    classify_upstream_error(
        429,
        r#"{"error":{"code":"turn_rate_limit_exceeded","message":"Codex is at capacity, please try again later"}}"#,
        Some("chatgpt"),
    )
}
fn delay(error: &GatewayError) -> u64 {
    match error.fallback_hint {
        FallbackHint::Retry { delay_ms, .. } => delay_ms,
        _ => panic!("missing retry hint"),
    }
}

#[test]
fn backoff_expires_without_probe_and_success_resets_only_exact_subject() {
    let mut backoff = CapacityBackoff::default();
    let k = key("a", "m");
    let mut now = 0;
    for expected in [2_000, 4_000, 8_000, 15_000, 15_000] {
        let ticket = backoff.acquire(&k, now).unwrap();
        let error = backoff.failure(ticket, &refusal(), now);
        assert_eq!(delay(&error), expected);
        let denied = backoff.acquire(&k, now + expected - 1).unwrap_err();
        assert_eq!(delay(&denied), 1);
        assert!(!denied.retryable);
        assert_eq!(denied.http_status, Some(429));
        assert_eq!(denied.code, error.code);
        assert_eq!(denied.message, error.message);
        // Repeated local refusals do not increase the level or move the deadline.
        assert!(backoff.acquire(&k, now + expected - 1).is_err());
        now += expected;
    }
    for other in [key("b", "m"), key("a", "other")] {
        let ticket = backoff.acquire(&other, now - 1).unwrap();
        backoff.finish(ticket, true);
    }
    let ticket = backoff.acquire(&k, now).unwrap();
    backoff.finish(ticket, true);
    let ticket = backoff.acquire(&k, now).unwrap();
    assert_eq!(delay(&backoff.failure(ticket, &refusal(), now)), 2_000);
}

#[test]
fn old_in_flight_success_cannot_erase_newer_refusal_and_neutral_cleans_up() {
    let mut backoff = CapacityBackoff::default();
    let k = key("a", "m");
    let old = backoff.acquire(&k, 0).unwrap();
    let failed = backoff.acquire(&k, 0).unwrap();
    backoff.failure(failed, &refusal(), 10);
    backoff.finish(old, true);
    assert_eq!(delay(&backoff.acquire(&k, 11).unwrap_err()), 1_999);
    let other = key("b", "m");
    let ticket = backoff.acquire(&other, 0).unwrap();
    backoff.finish(ticket, false);
    assert!(!backoff.entries.contains_key(&other));
}

#[test]
fn upstream_minimum_is_not_clipped_even_at_u64_max() {
    let mut backoff = CapacityBackoff::default();
    let k = key("a", "m");
    let mut error = refusal();
    error.upstream_retry_after_ms = Some(u64::MAX);
    let ticket = backoff.acquire(&k, 5).unwrap();
    assert_eq!(delay(&backoff.failure(ticket, &error, 5)), u64::MAX);
    assert_eq!(delay(&backoff.acquire(&k, 6).unwrap_err()), u64::MAX - 1);
}

#[test]
fn explicit_minimum_is_respected_before_normal_traffic_resumes() {
    let mut backoff = CapacityBackoff::default();
    let k = key("a", "m");
    let mut error = refusal();
    error.http_status = Some(503);
    error.upstream_retry_after_ms = Some(25_001);
    let ticket = backoff.acquire(&k, 10).unwrap();
    assert_eq!(delay(&backoff.failure(ticket, &error, 10)), 25_001);
    assert_eq!(
        backoff.acquire(&k, 25_010).unwrap_err().http_status,
        Some(503)
    );
    let ticket = backoff.acquire(&k, 25_011).unwrap();
    backoff.finish(ticket, true);
}

#[test]
fn circuit_attempt_counts_capacity_once_and_cleans_cancelled_lease() {
    use crate::routing::model_circuit::{CircuitAttempt, ModelCircuits};
    let runtime = std::sync::Arc::new(ModelCircuits::default());
    let k = key("a", "m");
    let gate = CircuitAttempt::scoped(runtime.clone(), vec![k.clone()], Some(k.clone()));
    gate.begin().unwrap();
    let error = gate.finish_error(&refusal());
    gate.finish_error(&error); // Outer finalization cannot count this send again.
    assert!(
        runtime.available(&k),
        "capacity never opens the hard circuit"
    );
    assert_eq!(runtime.capacity.lock().entries[&k].level, 1);
    assert!(gate.begin().is_err());
    assert_eq!(runtime.capacity.lock().entries[&k].level, 1);
    let other = key("other", "m");
    let cancelled =
        CircuitAttempt::scoped(runtime.clone(), vec![other.clone()], Some(other.clone()));
    cancelled.begin().unwrap();
    drop(cancelled);
    assert!(!runtime.capacity.lock().entries.contains_key(&other));
}

#[test]
fn bounded_registry_does_not_evict_active_restrictions_or_leases() {
    let mut backoff = CapacityBackoff::default();
    for i in 0..LIMIT {
        let ticket = backoff.acquire(&key(&i.to_string(), "m"), 0).unwrap();
        backoff.failure(ticket, &refusal(), 0);
    }
    assert_eq!(
        backoff
            .acquire(&key("new", "m"), 1)
            .unwrap_err()
            .code
            .as_deref(),
        Some("capacity_backoff_registry_full")
    );
    let ticket = backoff.acquire(&key("new", "m"), 2_000).unwrap();
    backoff.finish(ticket, false);
    assert!(backoff.entries.is_empty());
}
