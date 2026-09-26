use super::{
    choose_release_status, is_challenge_open, BrowserCapabilitySlotStatus,
    BrowserCapabilitySlotView,
};
use crate::routing::candidate::ProviderExecutionMode;

fn sample_slot(
    status: BrowserCapabilitySlotStatus,
    degradation_reasons: Option<Vec<&str>>,
) -> BrowserCapabilitySlotView {
    BrowserCapabilitySlotView {
        slot_id: "slot-1".to_string(),
        node_id: "node-1".to_string(),
        provider_account_id: "provider-1".to_string(),
        adapter: "lumalabs_compatible".to_string(),
        endpoint_kind: "images".to_string(),
        execution_mode: ProviderExecutionMode::BrowserBacked,
        status,
        runtime_state_object_key: None,
        account_name: None,
        last_warm_at: None,
        last_used_at: None,
        last_failure_at: None,
        degradation_reasons: degradation_reasons
            .map(|values| values.into_iter().map(|value| value.to_string()).collect()),
        updated_at: "2026-04-14T00:00:00Z".to_string(),
    }
}

#[test]
fn release_status_returns_cooling_for_failure_reasons() {
    assert_eq!(
        choose_release_status(Some("browser challenge timeout")),
        BrowserCapabilitySlotStatus::Cooling
    );
    assert_eq!(
        choose_release_status(Some("hard fail")),
        BrowserCapabilitySlotStatus::Cooling
    );
}

#[test]
fn release_status_returns_warm_for_clean_release() {
    assert_eq!(
        choose_release_status(Some("completed")),
        BrowserCapabilitySlotStatus::Warm
    );
}

#[test]
fn challenge_detection_looks_at_degradation_reasons() {
    assert!(is_challenge_open(&sample_slot(
        BrowserCapabilitySlotStatus::Cooling,
        Some(vec!["challenge_open"])
    )));
    assert!(!is_challenge_open(&sample_slot(
        BrowserCapabilitySlotStatus::Warm,
        Some(vec!["rate_limited"])
    )));
}
