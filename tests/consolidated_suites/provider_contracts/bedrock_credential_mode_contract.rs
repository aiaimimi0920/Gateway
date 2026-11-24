//! Explicit account modes, compile admission and keep bindings, using synthetic data only.
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::db::merge_provider_account_and_credential_payloads;
use neuro_gateway::routing::candidate::ProviderAccountPayload;
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};

fn document(provider: Value) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[provider],"model_routes":[],"aliases":{}})).unwrap()
}

fn provider() -> Value {
    json!({"id":"renamed-pool", "preset":"bedrock-converse", "explicit_credentials_only":true,
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com", "credentials":[]})
}

fn credential(mode: &str) -> Value {
    let extra = match mode {
        "bedrock_bearer" => json!({"awsRegion":"us-east-1"}),
        "aws_sigv4" => {
            json!({"awsAccessKeyId":"synthetic-access", "awsSecretAccessKey":"synthetic-secret", "awsRegion":"us-east-1"})
        }
        _ => {
            json!({"awsAccessKeyId":"synthetic-access", "awsSecretAccessKey":"synthetic-secret", "awsSessionToken":"synthetic-sts", "awsRegion":"us-east-1"})
        }
    };
    json!({"id":mode, "authMode":mode, "api_key":if mode == "bedrock_bearer" {"synthetic-bearer"} else {""}, "extra_body":extra})
}

#[test]
fn account_modes_roundtrip_and_compile_in_one_pool_with_no_shared_material() {
    for preset in [true, false] {
        let mut p = provider();
        if !preset {
            p.as_object_mut().unwrap().remove("preset");
            p["adapter"] = json!("bedrock_converse_compatible");
        }
        p["credentials"] = json!([
            credential("bedrock_bearer"),
            credential("aws_sigv4"),
            credential("aws_sigv4_temporary")
        ]);
        let typed = document(p);
        let encoded = serde_json::to_value(&typed).unwrap();
        assert_eq!(
            encoded["providers"][0]["credentials"][1]["auth_mode"],
            "aws_sigv4"
        );
        let reloaded: RouteConfigYaml =
            serde_yaml::from_str(&serde_yaml::to_string(&typed).unwrap()).unwrap();
        let store = RouteConfigStore::from_document(reloaded).unwrap();
        let providers = store.get_providers();
        for (index, mode) in ["bedrock_bearer", "aws_sigv4", "aws_sigv4_temporary"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                providers[0].credential_pool[index]
                    .payload
                    .auth_mode
                    .as_deref(),
                Some(*mode)
            );
        }
        assert!(providers[0].payload.auth_mode.is_none());
    }
}

#[test]
fn absent_account_mode_inherits_without_adding_a_serialized_field() {
    let mut p = provider();
    p["auth_mode"] = json!("bedrock_bearer");
    let mut c = credential("bedrock_bearer");
    c.as_object_mut().unwrap().remove("authMode");
    p["credentials"] = json!([c]);
    let typed = document(p);
    assert!(
        serde_json::to_value(&typed).unwrap()["providers"][0]["credentials"][0]
            .get("auth_mode")
            .is_none()
    );
    assert_eq!(
        RouteConfigStore::from_document(typed)
            .unwrap()
            .get_providers()[0]
            .credential_pool[0]
            .payload
            .auth_mode
            .as_deref(),
        Some("bedrock_bearer")
    );
}

#[test]
fn hosted_mode_aliases_override_between_layers_but_duplicates_stay_invalid() {
    for (base, overlay) in [("auth_mode", "authMode"), ("authMode", "auth_mode")] {
        let mut account = json!({"adapter":"bedrock_converse_compatible", "base_url":"https://example.test", "api_key":""});
        account[base] = json!("bedrock_bearer");
        let mut cred = json!({});
        cred[overlay] = json!("aws_sigv4");
        let merged = merge_provider_account_and_credential_payloads(&account, &cred);
        let payload: ProviderAccountPayload = serde_json::from_value(merged).unwrap();
        assert_eq!(payload.auth_mode.as_deref(), Some("aws_sigv4"));
        let duplicate = merge_provider_account_and_credential_payloads(
            &account,
            &json!({"auth_mode":"aws_sigv4", "authMode":"bedrock_bearer"}),
        );
        assert!(serde_json::from_value::<ProviderAccountPayload>(duplicate).is_err());
        // Preserve the existing Hosted generic merge contract: blank inherits, null removes.
        cred[overlay] = json!("");
        let merged = merge_provider_account_and_credential_payloads(&account, &cred);
        assert_eq!(
            serde_json::from_value::<ProviderAccountPayload>(merged)
                .unwrap()
                .auth_mode
                .as_deref(),
            Some("bedrock_bearer")
        );
        cred[overlay] = Value::Null;
        let merged = merge_provider_account_and_credential_payloads(&account, &cred);
        assert!(serde_json::from_value::<ProviderAccountPayload>(merged)
            .unwrap()
            .auth_mode
            .is_none());
    }
}

#[test]
fn explicit_empty_pool_is_valid_but_incomplete_real_accounts_are_not() {
    let mut p = provider();
    p["auth_mode"] = json!("aws_sigv4_temporary");
    assert!(RouteConfigStore::from_document(document(p.clone())).is_ok());
    for bad in [
        json!({"id":"a", "extra_body":{"awsRegion":"us-east-1"}}),
        json!({"id":"a", "extra_body":{"awsAccessKeyId":"synthetic-access", "awsSecretAccessKey":"synthetic-secret", "awsRegion":"us-east-1"}}),
        json!({"id":"a", "enabled":false}),
    ] {
        p["credentials"] = json!([bad]);
        assert!(
            RouteConfigStore::from_document(document(p.clone())).is_err(),
            "incomplete explicit account compiled"
        );
    }
    p["credentials"] = json!([]);
    p["explicit_credentials_only"] = json!(false);
    assert!(
        RouteConfigStore::from_document(document(p)).is_err(),
        "provider default is a real account"
    );
}

#[test]
fn final_material_validation_rejects_conflict_target_and_adapter_rebinding() {
    for bad in [
        json!({"extra_body":{"awsRegion":"us-west-2"}}),
        json!({"extra_body":{"awsRegion":"us-east-1", "awsAccessKeyId":"synthetic-other"}}),
        json!({"base_url":"https://example.test"}),
        json!({"auth_token":"synthetic-conflict"}),
    ] {
        let mut p = provider();
        let mut c = credential("bedrock_bearer");
        for (k, v) in bad.as_object().unwrap() {
            c[k] = v.clone();
        }
        p["credentials"] = json!([c]);
        assert!(
            RouteConfigStore::from_document(document(p)).is_err(),
            "invalid material accepted: {bad}"
        );
    }
    let mut p = provider();
    p["call_flow"] =
        json!({"call_adapter":"openai", "fingerprint_adapter":"auto", "special_adapter":"none"});
    p["credentials"] = json!([credential("bedrock_bearer")]);
    assert!(RouteConfigStore::from_document(document(p)).is_err());
}

#[test]
fn credential_explicit_modes_protect_kept_material_even_without_provider_mode() {
    for shared in [false, true] {
        let mut p = provider();
        let mut c = credential("aws_sigv4");
        if shared {
            p["extra_body"] = c["extra_body"].take();
            c["extra_body"] = json!({});
        }
        p["credentials"] = json!([c]);
        let active = document(p);
        let redacted = redact_route_document(&active).unwrap();
        let keeps: Vec<_> = redacted
            .secrets
            .iter()
            .filter(|s| s.configured)
            .map(|s| SecretPatch::keep(&s.path))
            .collect();
        for mode in [
            Value::Null,
            json!("bedrock_bearer"),
            json!("aws_sigv4_temporary"),
        ] {
            let mut draft = serde_json::to_value(&redacted.document).unwrap();
            draft["providers"][0]["credentials"][0]["auth_mode"] = mode;
            let error =
                resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &keeps)
                    .expect_err("keep cannot change effective mode");
            assert_eq!(error.code(), "secret_keep_destination_changed");
        }
        if shared {
            let mut draft = serde_json::to_value(&redacted.document).unwrap();
            draft["providers"][0]["credentials"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":"new", "auth_mode":"aws_sigv4"}));
            assert_eq!(
                resolve_secret_patches(&active, serde_json::from_value(draft).unwrap(), &keeps)
                    .unwrap_err()
                    .code(),
                "secret_keep_destination_changed"
            );
        }
    }
}
