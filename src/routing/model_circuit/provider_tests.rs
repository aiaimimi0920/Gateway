//! Provider IP restrictions must never turn model/account failures into whole-provider bans.
use super::*;

fn keys() -> Vec<CircuitKey> {
    vec![
        CircuitKey {
            provider: "site".into(),
            group: "provider:availability".into(),
            model: String::new(),
        },
        CircuitKey {
            provider: "site".into(),
            group: "group:shared".into(),
            model: "model-a".into(),
        },
        CircuitKey {
            provider: "site".into(),
            group: "balance:account-a".into(),
            model: String::new(),
        },
    ]
}

#[test]
fn provider_ip_block_does_not_broaden_model_or_balance_failures() {
    for outcome in [
        Outcome::Failure(0),
        Outcome::TransportFailure(0),
        Outcome::ChannelUnavailable(0),
        Outcome::BalanceUnavailable(0),
    ] {
        let runtime = ModelCircuits::default();
        let keys = keys();
        for now in 0..3 {
            // Independent failed requests share the provider ticket, not the model/account scope.
            let mut attempt_keys = keys.clone();
            attempt_keys[1].model = format!("model-{now}");
            attempt_keys[2].group = format!("balance:account-{now}");
            let tickets = runtime.acquire_at(&attempt_keys, now).unwrap();
            runtime.finish_at(tickets, outcome, now);
            assert!(
                runtime.available_at(&keys[0], now + 1),
                "ordinary failures must not ban the provider"
            );
        }
        assert_eq!(runtime.entries.lock()[&keys[0]].level, 0);
    }
}

#[test]
fn provider_ip_block_has_level_three_trial_escalation_and_stale_completion_protection() {
    let runtime = ModelCircuits::default();
    runtime.enable_network_test_guard();
    let keys = keys();
    let tickets = runtime.acquire_at(&keys, 0).unwrap();
    let late = runtime.acquire_at(&keys, 0).unwrap();
    runtime.finish_at(tickets, Outcome::ProviderUnavailable(0), 10);
    runtime.finish_at(late, Outcome::Success, 11);
    assert_eq!(runtime.entries.lock()[&keys[0]].level, 3);
    assert!(!runtime.available_at(&keys[0], 1_920_009));
    assert!(runtime.available_at(&keys[1], 12));
    assert!(runtime.available_at(&keys[2], 12));
    assert!(
        runtime.network_epoch().is_none(),
        "confirmed IP refusal must not stop fallback behind a global egress check"
    );
    let tickets = runtime.acquire_at(&keys, 1_920_010).unwrap();
    assert!(runtime.acquire_at(&keys, 1_920_010).is_err());
    runtime.finish_at(tickets, Outcome::ProviderUnavailable(0), 1_920_010);
    assert_eq!(runtime.entries.lock()[&keys[0]].level, 4);
    assert!(!runtime.available_at(&keys[0], 5_520_009));
    assert!(runtime.available_at(&keys[0], 5_520_010));
}

#[test]
fn provider_ip_block_is_not_cleared_by_unrelated_egress_recovery() {
    let runtime = ModelCircuits::default();
    runtime.enable_network_test_guard();
    let keys = keys();
    let pending = runtime.acquire_at(&keys, 0).unwrap();
    let transport = CircuitKey {
        provider: "other".into(),
        group: "group:other".into(),
        model: "m".into(),
    };
    let tickets = runtime.acquire_at(&[transport.clone()], 0).unwrap();
    runtime.finish_at(tickets, Outcome::TransportFailure(120_000), 1);
    let epoch = runtime.network_epoch().unwrap();
    runtime.complete_network_probe(epoch, false);
    runtime.finish_at(pending, Outcome::ProviderUnavailable(0), 2);
    runtime.complete_network_probe(epoch, true);
    assert!(!runtime.available_at(&keys[0], 3));
    assert_eq!(runtime.entries.lock()[&keys[0]].level, 3);
    assert!(runtime.available_at(&transport, 3));
}
