//! Received Luma material is durable evidence, never permission to publish or resend.
use super::*;
use serde_json::json;

#[test]
fn luma_received_receipt_has_a_distinct_non_publishable_storage_variant() {
    let value = json!({"kind":"lumaOauth","tokens":{"body":"",
        "startedAt":"2026-01-01T00:00:00Z","binding":"0".repeat(64)}});
    let receipt = serde_json::from_value::<GrantReceipt>(value);
    assert!(
        receipt.is_ok(),
        "Luma received evidence needs a dedicated variant: {receipt:?}"
    );
}
