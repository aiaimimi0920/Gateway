//! Public payload redaction and partial updates must survive owner extraction.

use super::model_pricing::merge_provider_model_pricing;
use super::redaction::mask_provider_payload_secrets;
use super::source_profile::{normalize_explicit_source_profile, ProviderSourceProfileBody};
use serde_json::json;

#[test]
fn recursive_redaction_masks_credentials_without_erasing_transport_metadata() {
    let masked = mask_provider_payload_secrets(json!({
        "apiKey": "demo-key-123456789",
        "transport": {
            "headers": {
                "Authorization": "Bearer synthetic-123456789",
                "COOKIE": "sid=synthetic-cookie",
                "x-api-key": "abcdefghij",
                "X-Goog-Api-Key": 42,
                "x-trace-id": "trace-a",
                "content-type": "application/json"
            }
        },
        "children": [{
            "API_SECRET": "abcdefghij",
            "auth_token": "   ",
            "token": "  abc  ",
            "cookie": null,
            "keep": true
        }],
        "defaultModel": "model-a"
    }));
    assert_eq!(masked["apiKey"], "demo***89");
    let headers = &masked["transport"]["headers"];
    assert_eq!(headers["Authorization"], "Bear***89");
    assert_eq!(headers["COOKIE"], "sid=***ie");
    assert_eq!(headers["x-api-key"], "abcd***ij");
    assert_eq!(headers["X-Goog-Api-Key"], "***");
    assert_eq!(headers["x-trace-id"], "trace-a");
    assert_eq!(headers["content-type"], "application/json");
    assert_eq!(masked["children"][0]["API_SECRET"], "abcd***ij");
    assert_eq!(masked["children"][0]["auth_token"], "");
    assert_eq!(masked["children"][0]["token"], "ab***");
    assert_eq!(masked["children"][0]["cookie"], "***");
    assert_eq!(masked["children"][0]["keep"], true);
    assert_eq!(masked["defaultModel"], "model-a");
}

#[test]
fn model_pricing_patch_preserves_other_models_and_supports_explicit_removal() {
    let entries = serde_json::from_value(json!([
        {"model": " remove-me "},
        {"model": " update-me ", "promptMicrosPer1kTokens": 0, "completionMicrosPer1kTokens": 12},
        {"model": "   ", "promptMicrosPer1kTokens": 99}
    ]))
    .unwrap();
    let updated = merge_provider_model_pricing(
        json!({
            "pricingByModel": {
                "keep-me": {"staticInputMicrosPer1kTokens": 7},
                "remove-me": {"staticInputMicrosPer1kTokens": 3}
            },
            "baseUrl": "https://provider.invalid/v1",
            "transport": {"timeoutSeconds": 17}
        }),
        entries,
    )
    .unwrap();
    assert_eq!(
        updated,
        json!({
            "modelPricing": {
                "keep-me": {"staticInputMicrosPer1kTokens": 7},
                "update-me": {"staticInputMicrosPer1kTokens": 0, "staticOutputMicrosPer1kTokens": 12}
            },
            "baseUrl": "https://provider.invalid/v1",
            "transport": {"timeoutSeconds": 17}
        })
    );
}

#[test]
fn negative_or_unsafe_model_prices_are_rejected_instead_of_becoming_free() {
    for value in [-1, crate::cash_billing::MAX_MICROS + 1] {
        let entries = serde_json::from_value(json!([
            {"model":"paid-model", "promptMicrosPer1kTokens":value, "completionMicrosPer1kTokens":12}
        ])).unwrap();
        assert!(merge_provider_model_pricing(json!({}), entries).is_err());
    }
}

#[test]
fn source_profile_rejects_modes_from_another_source_kind() {
    for source_kind in [
        "official_model_api",
        "official_vendor_api",
        "web_reverse_api",
    ] {
        assert!(
            normalize_explicit_source_profile(ProviderSourceProfileBody {
                source_kind: source_kind.to_string(),
                aggregator_api_mode: Some("hosted_compute".to_string()),
                web_reverse_access_mode: None,
                notes: None,
            })
            .is_err()
        );
    }
    for source_kind in [
        "official_model_api",
        "official_vendor_api",
        "aggregator_api",
    ] {
        assert!(
            normalize_explicit_source_profile(ProviderSourceProfileBody {
                source_kind: source_kind.to_string(),
                aggregator_api_mode: None,
                web_reverse_access_mode: Some("browser_challenge".to_string()),
                notes: None,
            })
            .is_err()
        );
    }
}
