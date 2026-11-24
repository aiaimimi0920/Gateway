//! Hosted capability provenance is still available before the payload becomes typed.
use super::*;
use serde_json::json;

const VERSION: &str = "2024-12-01-preview";

fn account(settings: Value) -> Value {
    json!({"authMode":crate::protocol::sap_aicore::AUTH_MODE,
        "base_url":"https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com/v2/inference/deployments/first",
        "headers":{"AI-Resource-Group":"original"},"extra_body":settings})
}

#[test]
fn sap_aicore_hosted_merge_respects_deployment_and_not_the_credential_flag() {
    let mut parent = account(json!({"sap_api_version":VERSION}));
    let overlay = json!({"api_key":"opaque-test-key","extra_body":{"sap_o3_mini":true}});
    let merged = merge_provider_account_and_credential_payloads(&parent, &overlay);
    assert!(merged["extra_body"].get("sap_o3_mini").is_none());
    assert_eq!(merged["api_key"], "opaque-test-key");
    parent["extra_body"]["sap_o3_mini"] = json!(true);
    for overlay in [
        json!({}),
        json!({"extra_body":{"sap_o3_mini":false,"sap_image_input":true}}),
        json!({"extraBody":{"sap_o3_mini":null}}),
        json!({"extra_body":null}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&parent, &overlay);
        assert_eq!(merged["extra_body"]["sap_o3_mini"], true);
        assert!(merged["extra_body"].get("sap_image_input").is_none());
        assert!(merged.get("extraBody").is_none());
    }
    for overlay in [
        json!({"headers":{"AI-Resource-Group":"different"}}),
        json!({"extraBody":{"sap_api_version":"2024-10-21","sap_o3_mini":true}}),
        json!({"baseUrl":"https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com/v2/inference/deployments/second"}),
    ] {
        let merged = merge_provider_account_and_credential_payloads(&parent, &overlay);
        assert!(
            merged["extra_body"].get("sap_o3_mini").is_none(),
            "{overlay}"
        );
    }
}

#[test]
fn sap_aicore_hosted_merge_preserves_legacy_and_unrelated_overlays() {
    let parent = account(json!({"sap_api_version":"2024-10-21","sap_image_input":true}));
    let merged = merge_provider_account_and_credential_payloads(
        &parent,
        &json!({"extraBody":{"sap_image_input":false}}),
    );
    assert_eq!(merged["extra_body"]["sap_image_input"], true);
    let credential = json!({"extra_body":{"sap_o3_mini":true}});
    let merged = merge_provider_account_and_credential_payloads(&json!({}), &credential);
    assert_eq!(merged, credential);
    let credential = json!({"auth_mode":crate::protocol::sap_aicore::AUTH_MODE,
        "extra_body":{"sap_api_version":VERSION,"sap_o3_mini":true}});
    let merged = merge_provider_account_and_credential_payloads(&json!({}), &credential);
    assert_eq!(merged["extra_body"]["sap_api_version"], VERSION);
    assert!(merged["extra_body"].get("sap_o3_mini").is_none());
}

#[test]
fn sap_aicore_native_responses_requires_the_hosted_account_binding() {
    let mut parent = account(json!({"sap_api_version":VERSION}));
    let overlay =
        json!({"extra_body":{"sap_native_responses":true,"sap_native_responses_image_input":true}});
    use super::merge_provider_account_and_credential_payloads as merge;
    let flags = ["sap_native_responses", "sap_native_responses_image_input"];
    let merged = merge(&parent, &overlay);
    for flag in flags {
        assert!(merged["extra_body"].get(flag).is_none());
        parent["extra_body"][flag] = json!(true);
    }
    let merged = merge(
        &parent,
        &json!({"extraBody":{"sap_native_responses":false,"sap_native_responses_image_input":false}}),
    );
    for flag in flags {
        assert_eq!(merged["extra_body"][flag], true);
    }
    for overlay in [
        json!({"headers":{"AI-Resource-Group":"different"}}),
        json!({"extraBody":{"sap_api_version":"2024-10-21"}}),
        json!({"baseUrl":"https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com/v2/inference/deployments/second"}),
    ] {
        let merged = merge(&parent, &overlay);
        for flag in flags {
            assert!(
                merged["extra_body"].get(flag).is_none(),
                "{flag}: {overlay}"
            );
        }
    }
}

#[test]
fn sap_aicore_bedrock_is_a_hosted_pool_mode_not_a_credential_capability() {
    use super::merge_provider_account_and_credential_payloads as merge;
    let parent = account(json!({"sap_bedrock_converse":true}));
    for overlay in [
        json!({"extra_body":null}),
        json!({"extraBody":{
        "sap_bedrock_converse":false,"sap_orchestration_model":"other"}}),
    ] {
        assert_eq!(
            merge(&parent, &overlay)["extra_body"],
            json!({"sap_bedrock_converse":true})
        );
    }
    for overlay in [
        json!({"headers":{"AI-Resource-Group":"other"}}),
        json!({"extraBody":{"sap_api_version": VERSION}}),
        json!({"baseUrl":"https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com/v2/inference/deployments/other"}),
    ] {
        assert!(merge(&parent, &overlay).is_null(), "{overlay}");
    }
    let azure = account(json!({"sap_api_version":VERSION}));
    let merged = merge(&azure, &json!({"extra_body":{"sap_bedrock_converse":true}}));
    assert_eq!(merged["extra_body"], azure["extra_body"]);
}

#[test]
fn sap_aicore_bedrock_image_admission_is_owned_by_the_hosted_deployment() {
    use super::merge_provider_account_and_credential_payloads as merge;
    let mut parent = account(json!({"sap_bedrock_converse":true}));
    let overlay = json!({"extraBody":{"sap_bedrock_image_input":true}});
    assert_eq!(merge(&parent, &overlay)["extra_body"], parent["extra_body"]);
    parent["extra_body"]["sap_bedrock_image_input"] = json!(true);
    for overlay in [
        json!({"api_key":"opaque-test-key"}),
        json!({"extra_body":null}),
        json!({"extraBody":{"sap_bedrock_image_input":false}}),
    ] {
        assert_eq!(merge(&parent, &overlay)["extra_body"], parent["extra_body"]);
    }
    for overlay in [
        json!({"headers":{"AI-Resource-Group":"elsewhere"}}),
        json!({"extra_body":{"sap_api_version":VERSION}}),
    ] {
        assert!(merge(&parent, &overlay).is_null());
    }
    let azure = account(json!({"sap_api_version":VERSION,"sap_bedrock_image_input":true}));
    assert!(merge(&azure, &json!({}))["extra_body"]
        .get("sap_bedrock_image_input")
        .is_none());
}

#[test]
fn sap_aicore_orchestration_image_admission_is_owned_by_the_hosted_deployment() {
    use super::merge_provider_account_and_credential_payloads as merge;
    let mut parent = account(json!({"sap_orchestration_model":"gpt-4o"}));
    let overlay = json!({"extraBody":{"sap_orchestration_image_input":true}});
    assert_eq!(merge(&parent, &overlay)["extra_body"], parent["extra_body"]);
    parent["extra_body"]["sap_orchestration_image_input"] = json!(true);
    let overlay = json!({"extra_body":{"sap_orchestration_model":"other","sap_orchestration_image_input":false}});
    assert_eq!(merge(&parent, &overlay)["extra_body"], parent["extra_body"]);
    assert!(merge(&parent, &json!({"headers":{"AI-Resource-Group":"other"}})).is_null());
}
