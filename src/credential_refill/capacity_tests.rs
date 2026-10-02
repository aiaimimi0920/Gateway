//! Regression coverage for shared callback/folder/driver-pull admission.
use super::delivery::append_refill_credentials;
use crate::routing::config::{ProviderConfigYaml, ProviderCredentialYaml};

fn provider(maximum: usize) -> ProviderConfigYaml {
    serde_json::from_value(serde_json::json!({
        "id": "p", "base_url": "https://example.invalid", "auto_refill_enabled": false,
        "pool_target_size": maximum, "pool_min_size": 0,
        "credentials": [{"id": "existing", "api_key": "unchanged"}]
    }))
    .unwrap()
}
fn credentials(ids: &[&str]) -> Vec<ProviderCredentialYaml> {
    ids.iter()
        .map(|id| {
            serde_json::from_value(serde_json::json!({"id": id, "api_key": "fixture"})).unwrap()
        })
        .collect()
}

#[test]
fn manual_delivery_obeys_maximum_even_when_automation_is_disabled() {
    let mut provider = provider(2);
    assert_eq!(
        append_refill_credentials(&mut provider, credentials(&["new"])).unwrap(),
        1
    );
    let error = append_refill_credentials(&mut provider, credentials(&["overflow"])).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("credential_refill_capacity_exceeded")
    );
    assert_eq!(provider.credentials.len(), 2);
    assert!(!provider.pool_refill_in_progress);
}

#[test]
fn overflow_rejects_whole_delivery_without_a_partial_mutation() {
    let mut provider = provider(2);
    assert!(append_refill_credentials(&mut provider, credentials(&["new", "overflow"])).is_err());
    assert_eq!(provider.credentials.len(), 1);
    assert_eq!(
        provider.credentials[0].api_key.as_deref(),
        Some("unchanged")
    );
}

#[test]
fn duplicate_delivery_is_idempotent_at_the_maximum() {
    let mut provider = provider(1);
    assert_eq!(
        append_refill_credentials(&mut provider, credentials(&["existing"])).unwrap(),
        0
    );
    assert_eq!(
        provider.credentials[0].api_key.as_deref(),
        Some("unchanged")
    );
}

#[test]
fn disabled_credentials_cannot_create_a_nonconverging_refill_cycle() {
    let mut provider = provider(2);
    let mut incoming = credentials(&["disabled"]);
    incoming[0].enabled = Some(false);
    assert!(append_refill_credentials(&mut provider, incoming).is_err());
    assert_eq!(provider.credentials.len(), 1);
}

#[test]
fn serialized_concurrent_deliveries_recheck_the_last_slot() {
    let provider = std::sync::Mutex::new(provider(2));
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            append_refill_credentials(&mut provider.lock().unwrap(), credentials(&["first"]))
        });
        let second = scope.spawn(|| {
            append_refill_credentials(&mut provider.lock().unwrap(), credentials(&["second"]))
        });
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(provider.lock().unwrap().credentials.len(), 2);
}

#[test]
fn a_reduced_maximum_is_checked_again_at_delivery_time() {
    let mut provider = provider(100);
    // A previously created task does not reserve stale capacity after a route edit.
    provider.pool_target_size = Some(1);
    assert!(append_refill_credentials(&mut provider, credentials(&["late"])).is_err());
    assert_eq!(provider.credentials.len(), 1);
}

#[test]
fn observed_invalid_history_does_not_consume_new_available_capacity() {
    let mut provider = provider(2);
    // A health snapshot reports the old credential as invalid, leaving two slots.
    assert_eq!(
        super::delivery::append_refill_credentials_with_capacity(
            &mut provider,
            credentials(&["fresh-a", "fresh-b"]),
            2
        )
        .unwrap(),
        2
    );
    assert_eq!(provider.credentials.len(), 3);
    // A later snapshot reserves both new, still-unobserved credentials.
    assert!(super::delivery::append_refill_credentials_with_capacity(
        &mut provider,
        credentials(&["overflow"]),
        0
    )
    .is_err());
}
