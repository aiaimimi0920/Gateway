//! Pure logical-clock incident tests; no real network or account material.
use super::*;
fn key(provider: &str, model: &str) -> CircuitKey {
    CircuitKey {
        provider: provider.into(),
        group: "plus".into(),
        model: model.into(),
    }
}
fn failed(runtime: &ModelCircuits, key: &CircuitKey, at: u64) {
    let ticket = runtime.acquire_at(&[key.clone()], at).unwrap();
    runtime.finish_at(ticket, Outcome::TransportFailure(0), at);
}
fn trigger(runtime: &ModelCircuits, key: &CircuitKey) {
    for at in [0, 10, 20] {
        failed(runtime, key, at);
    }
}
#[test]
fn failed_egress_round_blocks_every_model_then_rewinds_only_incident_changes() {
    let runtime = ModelCircuits::default();
    let old = key("old", "m");
    trigger(&runtime, &old);
    failed(&runtime, &old, 120_020);
    let old_trial = runtime.acquire_at(&[old.clone()], 600_020).unwrap();
    runtime.network.lock().enabled = true;
    let first = key("first", "m");
    let sibling = key("sibling", "other");
    let in_flight = runtime.acquire_at(&[sibling.clone()], 0).unwrap();
    trigger(&runtime, &first);
    let epoch = runtime.network_epoch().unwrap();
    assert!(!runtime.available_at(&sibling, 30));
    runtime.complete_network_probe(epoch, false);
    assert_eq!(runtime.network_report()["label"], "临时网络波动");
    runtime.finish_at(in_flight, Outcome::TransportFailure(120_000), 40);
    runtime.finish_at(old_trial, Outcome::TransportFailure(0), 600_030);
    assert_eq!(runtime.entries.lock()[&old].level, 3);
    assert_eq!(runtime.entries.lock()[&sibling].level, 1);
    runtime.complete_network_probe(epoch, true);
    assert_eq!(runtime.entries.lock()[&old].level, 2);
    assert!(runtime.available_at(&old, 600_040));
    assert!(runtime.available_at(&first, 50));
    assert!(runtime.available_at(&sibling, 50));
    assert_eq!(runtime.entries.lock()[&first].level, 0);
    assert_eq!(runtime.network_report()["last_rollback_circuits"], 3);
}
#[test]
fn reachable_egress_does_not_undo_real_upstream_failure() {
    let runtime = ModelCircuits::default();
    runtime.network.lock().enabled = true;
    let k = key("site", "m");
    trigger(&runtime, &k);
    let epoch = runtime.network_epoch().unwrap();
    runtime.complete_network_probe(epoch, true);
    assert!(!runtime.available_at(&k, 100));
    assert_eq!(runtime.entries.lock()[&k].level, 1);
    assert!(runtime.available_at(&key("other", "m"), 100));
}
#[test]
fn manual_reset_fences_late_failure_and_outage_recovery_cannot_resurrect_it() {
    let runtime = ModelCircuits::default();
    let k = key("site", "m");
    let late = runtime.acquire_at(&[k.clone()], 0).unwrap();
    trigger(&runtime, &k);
    runtime.reset_provider("site");
    runtime.finish_at(late, Outcome::TransportFailure(120_000), 30);
    assert!(runtime.available_at(&k, 31));
    assert_eq!(runtime.entries.lock()[&k].level, 0);
    runtime.network.lock().enabled = true;
    trigger(&runtime, &k);
    let epoch = runtime.network_epoch().unwrap();
    runtime.complete_network_probe(epoch, false);
    runtime.reset_provider("site");
    runtime.complete_network_probe(epoch, true);
    assert!(runtime.available_at(&k, 40));
    assert_eq!(runtime.entries.lock()[&k].level, 0);
}
#[test]
fn network_gate_is_neutral_and_does_not_lease_or_fallback() {
    let runtime = std::sync::Arc::new(ModelCircuits::default());
    runtime.network.lock().enabled = true;
    trigger(&runtime, &key("site", "m"));
    let gate = CircuitAttempt::new(runtime.clone(), vec![key("other", "m")]);
    let error = gate.begin().unwrap_err();
    assert_eq!(error.code.as_deref(), Some("gateway_network_unstable"));
    assert!(matches!(failure_outcome(&error), Outcome::Neutral));
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!runtime.entries.lock().contains_key(&key("other", "m")));
}
#[tokio::test]
async fn probe_accepts_one_reachable_target_and_never_sends_credentials_or_follows_redirects() {
    use axum::{
        http::{HeaderMap, StatusCode},
        routing::get,
        Router,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/ok",
                    get(|headers: HeaderMap| async move {
                        for name in ["authorization", "cookie", "x-api-key", "chatgpt-account-id"] {
                            assert!(!headers.contains_key(name));
                        }
                        StatusCode::NO_CONTENT
                    }),
                )
                .route("/bad", get(|| async { StatusCode::BAD_GATEWAY }))
                .route(
                    "/redirect",
                    get(|| async {
                        (
                            StatusCode::FOUND,
                            [("location", "http://127.0.0.1:1/private")],
                        )
                    }),
                ),
        )
        .await
        .unwrap();
    });
    let client = crate::http_client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::none())
        .timeout(std::time::Duration::from_millis(200))
        .build()
        .unwrap();
    let bad = format!("http://{address}/bad");
    let ok = format!("http://{address}/ok");
    assert!(!network_probe::probe(&client, &[&bad, "http://127.0.0.1:1/missing"]).await);
    assert!(network_probe::probe(&client, &[&bad, &ok]).await);
    assert!(!network_probe::probe(&client, &[&format!("http://{address}/redirect")]).await);
    server.abort();
    let _ = server.await;
}

#[test]
fn recovery_fences_unobserved_inflight_transport_but_preserves_confirmed_balance_refusal() {
    let runtime = ModelCircuits::default();
    runtime.network.lock().enabled = true;
    let late_key = key("late", "m");
    let late = runtime.acquire_at(&[late_key.clone()], 0).unwrap();
    let quota_key = CircuitKey {
        provider: "quota".into(),
        group: "balance:account".into(),
        model: String::new(),
    };
    let quota = runtime.acquire_at(&[quota_key.clone()], 0).unwrap();
    trigger(&runtime, &key("first", "m"));
    let epoch = runtime.network_epoch().unwrap();
    runtime.complete_network_probe(epoch, false);
    runtime.finish_at(quota, Outcome::BalanceUnavailable(0), 30);
    runtime.complete_network_probe(epoch, true);
    runtime.finish_at(late, Outcome::TransportFailure(120_000), 40);
    assert!(runtime.available_at(&late_key, 41));
    assert_eq!(runtime.entries.lock()[&quota_key].level, 3);
    assert!(!runtime.available_at(&quota_key, 41));
}

#[test]
fn provider_ip_block_disables_all_models_for_that_provider_at_level_three() {
    let runtime = ModelCircuits::default();
    let provider_key = CircuitKey {
        provider: "site".into(),
        group: "provider:availability".into(),
        model: String::new(),
    };
    let model_key = key("site", "gpt-6.1-sol");
    let tickets = runtime
        .acquire_at(&[provider_key.clone(), model_key.clone()], 0)
        .unwrap();
    runtime.finish_at(tickets, Outcome::ProviderUnavailable(0), 1);

    assert!(!runtime.available_at(&provider_key, 2));
    assert!(runtime.available_at(&key("site", "another-model"), 2));
    assert!(runtime.available_at(&key("other-provider", "gpt-6.1-sol"), 2));
    assert_eq!(runtime.entries.lock()[&provider_key].level, 3);
    assert!(matches!(
        runtime.entries.lock()[&provider_key].phase,
        Phase::Open(until) if until >= 1 + 1_920_000
    ));
    assert_eq!(runtime.entries.lock()[&model_key].level, 0);
}
