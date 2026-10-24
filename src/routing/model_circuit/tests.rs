use super::*;
fn key(group: &str) -> CircuitKey {
    CircuitKey {
        provider: "site".into(),
        group: group.into(),
        model: "model".into(),
    }
}
fn finish(runtime: &ModelCircuits, key: &CircuitKey, at: u64, outcome: Outcome) {
    let ticket = runtime.acquire_at(&[key.clone()], at).unwrap();
    runtime.finish_at(ticket, outcome, at);
}
fn trip(runtime: &ModelCircuits, key: &CircuitKey) {
    for at in [0, 10, 20] {
        finish(runtime, key, at, Outcome::Failure(0));
    }
}
#[test]
fn consecutive_short_window_failures_open_and_only_one_trial_can_enter() {
    let runtime = ModelCircuits::default();
    let plus = key("plus");
    trip(&runtime, &plus);
    assert!(!runtime.available_at(&plus, 120_019));
    assert!(runtime.available_at(&key("pro"), 30));
    let trial = runtime.acquire_at(&[plus.clone()], 120_020).unwrap();
    assert!(runtime.acquire_at(&[plus.clone()], 120_020).is_err());
    runtime.finish_at(trial, Outcome::Failure(0), 120_020);
    assert!(!runtime.available_at(&plus, 600_019));
    assert!(runtime.available_at(&plus, 600_020));
}
#[test]
fn repeated_trial_failures_back_off_two_eight_thirty_two_and_sixty_minutes() {
    let runtime = ModelCircuits::default();
    let k = key("plus");
    trip(&runtime, &k);
    let mut at = 120_020;
    for minutes in [8, 32, 60, 60] {
        finish(&runtime, &k, at, Outcome::Failure(0));
        at += minutes * 60_000;
        assert!(!runtime.available_at(&k, at - 1));
        assert!(runtime.available_at(&k, at));
    }
}
#[test]
fn success_breaks_streak_and_old_failures_do_not_accumulate_forever() {
    let runtime = ModelCircuits::default();
    let k = key("plus");
    finish(&runtime, &k, 0, Outcome::Failure(0));
    finish(&runtime, &k, 10, Outcome::Success);
    finish(&runtime, &k, 20, Outcome::Failure(0));
    finish(&runtime, &k, 60_021, Outcome::Failure(0));
    assert!(runtime.available_at(&k, 60_021));
}
#[test]
fn late_completions_cannot_clear_new_generation_and_two_spaced_trials_restore() {
    let runtime = ModelCircuits::default();
    let k = key("plus");
    let late = runtime.acquire_at(&[k.clone()], 0).unwrap();
    trip(&runtime, &k);
    runtime.finish_at(late, Outcome::Success, 21);
    assert!(!runtime.available_at(&k, 21));
    finish(&runtime, &k, 120_020, Outcome::Success);
    assert!(!runtime.available_at(&k, 150_019));
    finish(&runtime, &k, 150_020, Outcome::Success);
    assert!(runtime.available_at(&k, 150_020));
    for at in [150_030, 150_040, 150_050] {
        finish(&runtime, &k, at, Outcome::Failure(0));
    }
    assert!(!runtime.available_at(&k, 630_049));
    assert!(runtime.available_at(&k, 630_050));
}
#[test]
fn neutral_trial_does_not_count_as_failure_or_leak_lease() {
    let runtime = ModelCircuits::default();
    let k = key("plus");
    trip(&runtime, &k);
    finish(&runtime, &k, 120_020, Outcome::Neutral);
    assert!(runtime.acquire_at(&[k.clone()], 120_021).is_err());
    assert!(runtime.acquire_at(&[k], 150_020).is_ok());
}
#[test]
fn upstream_wait_is_a_minimum_not_capped_at_one_hour() {
    let runtime = ModelCircuits::default();
    let k = key("plus");
    finish(&runtime, &k, 0, Outcome::Failure(7_200_000));
    assert!(!runtime.available_at(&k, 7_199_999));
    assert!(runtime.available_at(&k, 7_200_000));
}
#[test]
fn failure_taxonomy_excludes_request_errors_and_includes_diagnostic_403() {
    use crate::error::{classify_upstream_error, GatewayError};
    for error in [
        GatewayError::bad_request("invalid"),
        classify_upstream_error(400, "maximum context length", None),
        classify_upstream_error(403, "permission denied", None),
        GatewayError::content_filtered("no"),
    ] {
        assert_eq!(failure_delay(&error), None);
    }
    let diagnostic = classify_upstream_error(
        403,
        "<html><title>系统终端 - 异常状态诊断</title></html>",
        None,
    );
    assert_eq!(failure_delay(&diagnostic), Some(0));
    assert_eq!(
        failure_delay(&classify_upstream_error(503, "unavailable", None)),
        Some(0)
    );
}

#[test]
fn capacity_reclaims_success_churn_but_never_active_tickets_or_restrictions() {
    let runtime = ModelCircuits::default();
    let active_key = key("active");
    let active = runtime.acquire_at(&[active_key.clone()], 0).unwrap();
    let blocked = key("blocked");
    trip(&runtime, &blocked);
    for i in 0..8190 {
        finish(
            &runtime,
            &key(&format!("success-{i}")),
            30,
            Outcome::Success,
        );
    }
    finish(&runtime, &key("new"), 40, Outcome::Success);
    assert!(runtime.entries.lock().contains_key(&active_key));
    assert!(!runtime.available_at(&blocked, 40));
    runtime.finish_at(active, Outcome::Failure(0), 50);
    assert_eq!(runtime.entries.lock()[&active_key].failures.len(), 1);
}

#[test]
fn multi_group_acquisition_is_atomic_and_stable_success_resets_escalation() {
    let runtime = ModelCircuits::default();
    let plus = key("plus");
    let pro = key("pro");
    trip(&runtime, &plus);
    assert!(runtime
        .acquire_at(&[pro.clone(), plus.clone()], 30)
        .is_err());
    assert!(!runtime.entries.lock().contains_key(&pro));
    finish(&runtime, &plus, 120_020, Outcome::Success);
    finish(&runtime, &plus, 150_020, Outcome::Success);
    finish(&runtime, &plus, 1_950_020, Outcome::Success);
    assert_eq!(runtime.entries.lock()[&plus].level, 0);
}

#[test]
fn dropping_last_attempt_owner_releases_trial_without_resetting_escalation() {
    let runtime = std::sync::Arc::new(ModelCircuits::default());
    let k = key("plus");
    trip(&runtime, &k);
    runtime.entries.lock().get_mut(&k).unwrap().phase = Phase::Open(0);
    let gate = CircuitAttempt::new(runtime.clone(), vec![k.clone(), k.clone()]);
    gate.begin().unwrap();
    assert!(!runtime.available(&k));
    drop(gate);
    let entries = runtime.entries.lock();
    assert_eq!(entries[&k].in_flight, 0);
    assert_eq!(entries[&k].level, 1);
    assert!(matches!(
        entries[&k].phase,
        Phase::Trial { lease: None, .. }
    ));
}

#[test]
fn transport_unknown_is_counted_but_arbitrary_unknown_and_oversized_keys_are_not_retained() {
    let mut error =
        crate::error::GatewayError::service_unavailable("Network error: fixture body reset");
    error.kind = crate::error::ErrorKind::Unknown;
    error.http_status = None;
    assert_eq!(failure_delay(&error), Some(0));
    error.message = "unknown application error".into();
    assert_eq!(failure_delay(&error), None);
    let runtime = ModelCircuits::default();
    let mut k = key("plus");
    k.model = "m".repeat(1025);
    assert!(!runtime.available(&k));
    assert!(runtime.acquire_at(&[k], 0).is_err());
    assert!(runtime.entries.lock().is_empty());
}

#[test]
fn unavailable_channel_jumps_to_third_level_and_failed_trial_reaches_fourth() {
    let runtime = ModelCircuits::default();
    let k = key("auto");
    let late = runtime.acquire_at(&[k.clone()], 0).unwrap();
    let body = serde_json::json!({"error": {"code": "model_not_found", "message":
        "No available channel for model model under group auto (distributor)"}});
    let error = crate::error::classify_upstream_error(503, &body.to_string(), None);
    finish(&runtime, &k, 10, failure_outcome(&error));
    assert_eq!(runtime.entries.lock()[&k].level, 3);
    assert!(runtime.entries.lock()[&k].failures.is_empty());
    assert!(!runtime.available_at(&k, 1_920_009));
    assert!(runtime.available_at(&k, 1_920_010));
    assert!(runtime.available_at(&key("other-group"), 11));
    let mut other_model = k.clone();
    other_model.model = "other-model".into();
    assert!(runtime.available_at(&other_model, 11));
    let mut other_provider = k.clone();
    other_provider.provider = "other-provider".into();
    assert!(runtime.available_at(&other_provider, 11));
    runtime.finish_at(late, Outcome::Success, 11);
    assert_eq!(runtime.entries.lock()[&k].level, 3);
    finish(&runtime, &k, 1_920_010, failure_outcome(&error));
    assert_eq!(runtime.entries.lock()[&k].level, 4);
    assert!(!runtime.available_at(&k, 5_520_009));
    assert!(runtime.available_at(&k, 5_520_010));
    finish(&runtime, &k, 5_520_010, Outcome::Success);
    assert!(!runtime.available_at(&k, 5_550_009));
    finish(&runtime, &k, 5_550_010, Outcome::Success);
    assert!(runtime.available_at(&k, 5_550_010));
    assert_eq!(runtime.entries.lock()[&k].level, 4);
}
