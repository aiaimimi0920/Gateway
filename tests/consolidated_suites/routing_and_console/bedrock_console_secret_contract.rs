//! Synthetic Console material only: no AWS calls, live state or credentials.
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};

fn document(value: Value) -> RouteConfigYaml {
    serde_json::from_value(value).unwrap()
}

fn fixture(shared: bool) -> RouteConfigYaml {
    let material = json!({"awsAccessKeyId":"synthetic-access", "awsSecretAccessKey":"synthetic-secret", "awsRegion":"us-east-1"});
    document(
        json!({"providers":[{"id":"renamed", "preset":"bedrock-converse", "auth_mode":"aws_sigv4",
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com", "explicit_credentials_only":true,
        "extra_body": if shared { material.clone() } else { json!({}) },
        "credentials":[{"id":"a", "extra_body": if shared { json!({}) } else { material }}]}],
        "model_routes":[], "aliases":{}}),
    )
}

fn redacted(active: &RouteConfigYaml) -> (Value, Vec<SecretPatch>) {
    let redacted = redact_route_document(active).unwrap();
    let keeps = redacted
        .secrets
        .iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    (serde_json::to_value(redacted.document).unwrap(), keeps)
}

#[test]
fn bedrock_all_runtime_iam_aliases_use_the_secret_owner() {
    for key in [
        "awsAccessKeyId",
        "awsaccesskeyid",
        "AWSACCESSKEYID",
        "AWS_ACCESS_KEY_ID",
        "AWS-ACCESS-KEY-ID",
        "awsSecretAccessKey",
        "AWSSECRETACCESSKEY",
        "aws_secret_access_key",
        "awsSessionToken",
        "AWSSESSIONTOKEN",
    ] {
        let active = document(
            json!({"providers":[{"id":"p", "base_url":"https://example.test", "extra_body":{key:"synthetic-private"}}], "model_routes":[]}),
        );
        let (draft, _) = redacted(&active);
        assert!(
            !draft.to_string().contains("synthetic-private"),
            "alias escaped redaction: {key}"
        );
        let path = format!("/providers/0/extra_body/{key}");
        let replacement = resolve_secret_patches(
            &active,
            document(draft.clone()),
            &[SecretPatch::replace(&path, json!("synthetic-new"))],
        )
        .unwrap();
        assert_eq!(
            replacement.providers[0].extra_body[key],
            json!("synthetic-new")
        );
        let mut raw = draft;
        raw["providers"][0]["extra_body"][key] = json!("synthetic-inline");
        assert!(
            resolve_secret_patches(&active, document(raw), &[]).is_err(),
            "alias bypassed SecretPatch: {key}"
        );
    }
}

#[test]
fn bedrock_keep_rejects_authentication_destination_changes() {
    for change in [
        json!({"auth_mode":"bedrock_bearer"}),
        json!({"auth_mode":null}),
        json!({"auth_mode":"aws_sigv4_temporary"}),
        json!({"base_url":"https://bedrock-runtime.us-west-2.amazonaws.com"}),
        json!({"adapter":"openai_compatible"}),
        json!({"preset":"openai"}),
        json!({"protocol_profile":"anthropic"}),
        json!({"call_flow":{"call_adapter":"openai","special_adapter":"none","fingerprint_adapter":"auto"}}),
        json!({"messages_path":"/other"}),
        json!({"session_auth":{"transport":"bearer"}}),
    ] {
        let active = fixture(false);
        let (mut draft, keeps) = redacted(&active);
        for (key, value) in change.as_object().unwrap() {
            draft["providers"][0][key] = value.clone();
        }
        let error = resolve_secret_patches(&active, document(draft), &keeps)
            .expect_err("old material cannot be rebound through keep");
        assert_eq!(
            error.code(),
            "secret_keep_destination_changed",
            "change={change}"
        );
        assert!(!error.to_string().contains("synthetic-"));
    }
}

#[test]
fn bedrock_keep_binds_effective_region_and_credential_endpoint() {
    for shared in [false, true] {
        for change in [
            json!({"base_url":"https://bedrock-runtime.us-west-2.amazonaws.com"}),
            json!({"extra_body":{"awsRegion":"us-west-2"}}),
        ] {
            let active = fixture(shared);
            let (mut draft, keeps) = redacted(&active);
            for (key, value) in change.as_object().unwrap() {
                draft["providers"][0]["credentials"][0][key] = value.clone();
            }
            let error = resolve_secret_patches(&active, document(draft), &keeps)
                .expect_err("inherited materials are bound too");
            assert_eq!(error.code(), "secret_keep_destination_changed");
        }
    }
}

#[test]
fn bedrock_provider_secret_cannot_gain_a_new_destination_through_a_new_account() {
    let active = fixture(true);
    for endpoint in [
        "https://bedrock-runtime.us-east-1.amazonaws.com",
        "https://bedrock-runtime.us-west-2.amazonaws.com",
    ] {
        let (mut draft, keeps) = redacted(&active);
        draft["providers"][0]["credentials"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"new", "base_url":endpoint}));
        let result = resolve_secret_patches(&active, document(draft), &keeps);
        assert_eq!(
            result.unwrap_err().code(),
            "secret_keep_destination_changed"
        );
    }
}

#[test]
fn bedrock_keep_allows_metadata_and_stable_id_reordering_and_yaml_reload() {
    let mut active = fixture(false);
    active.providers.push(
        document(json!({"providers":[{"id":"other", "base_url":"https://example.test"}]}))
            .providers
            .remove(0),
    );
    let (mut draft, _) = redacted(&active);
    draft["providers"][0]["label"] = json!("new label");
    draft["providers"][0]["credentials"][0]["account_name"] = json!("new name");
    draft["providers"][0]["credentials"][0]["supported_models"] = json!(["fixture-model"]);
    draft["providers"].as_array_mut().unwrap().swap(0, 1);
    let keeps = ["awsAccessKeyId", "awsSecretAccessKey"]
        .map(|key| SecretPatch::keep(format!("/providers/1/credentials/0/extra_body/{key}")));
    let resolved = resolve_secret_patches(&active, document(draft), &keeps).unwrap();
    let saved = serde_yaml::to_string(&resolved).unwrap();
    let reload: RouteConfigYaml = serde_yaml::from_str(&saved).unwrap();
    assert_eq!(
        reload.providers[1].credentials[0].extra_body["awsSecretAccessKey"],
        json!("synthetic-secret")
    );
    assert!(
        !serde_json::to_string(&redact_route_document(&reload).unwrap().document)
            .unwrap()
            .contains("synthetic-")
    );
}

#[test]
fn bedrock_legacy_keep_is_unchanged_but_entering_explicit_mode_requires_replacement() {
    let mut active = fixture(false);
    active.providers[0].auth_mode = None;
    let (mut draft, keeps) = redacted(&active);
    draft["providers"][0]["base_url"] = json!("https://operator.example.test");
    assert!(resolve_secret_patches(&active, document(draft), &keeps).is_ok());
    let (mut draft, keeps) = redacted(&active);
    draft["providers"][0]["auth_mode"] = json!("aws_sigv4");
    assert_eq!(
        resolve_secret_patches(&active, document(draft), &keeps)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}

#[test]
fn bedrock_explicit_replace_or_clear_can_change_the_material_binding() {
    let active = fixture(false);
    for clear in [false, true] {
        let (mut draft, _) = redacted(&active);
        draft["providers"][0]["base_url"] =
            json!("https://bedrock-runtime.us-west-2.amazonaws.com");
        draft["providers"][0]["credentials"][0]["extra_body"]["awsRegion"] = json!("us-west-2");
        let patches: Vec<_> = ["awsAccessKeyId", "awsSecretAccessKey"]
            .into_iter()
            .map(|key| {
                let path = format!("/providers/0/credentials/0/extra_body/{key}");
                if clear {
                    SecretPatch::clear(path)
                } else {
                    SecretPatch::replace(path, json!("synthetic-new"))
                }
            })
            .collect();
        let resolved = resolve_secret_patches(&active, document(draft), &patches).unwrap();
        assert_eq!(
            resolved.providers[0].credentials[0]
                .extra_body
                .contains_key("awsSecretAccessKey"),
            !clear
        );
    }
}

#[test]
fn bedrock_shared_material_keep_excludes_credentials_with_complete_exact_overrides() {
    let active = fixture(true);
    let (mut draft, mut patches) = redacted(&active);
    draft["providers"][0]["credentials"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({"id":"new", "base_url":"https://bedrock-runtime.us-west-2.amazonaws.com",
        "extra_body":{"awsRegion":"us-west-2"}}),
        );
    for key in ["awsAccessKeyId", "awsSecretAccessKey"] {
        // The public draft cannot contain secrets: inheritance is evaluated after patches are resolved.
        patches.push(SecretPatch::replace(
            format!("/providers/0/credentials/1/extra_body/{key}"),
            json!("synthetic-new"),
        ));
    }
    assert!(resolve_secret_patches(&active, document(draft), &patches).is_ok());
}

#[test]
fn bedrock_existing_credential_cannot_drop_its_override_and_borrow_a_kept_provider_key() {
    let mut active = fixture(true);
    active.providers[0].credentials[0].extra_body.insert(
        "awsSecretAccessKey".into(),
        json!("synthetic-private-account"),
    );
    let (mut draft, mut patches) = redacted(&active);
    draft["providers"][0]["credentials"][0]["extra_body"]
        .as_object_mut()
        .unwrap()
        .remove("awsSecretAccessKey");
    patches.retain(|p| {
        !p.path
            .ends_with("credentials/0/extra_body/awsSecretAccessKey")
    });
    patches.push(SecretPatch::clear(
        "/providers/0/credentials/0/extra_body/awsSecretAccessKey",
    ));
    assert_eq!(
        resolve_secret_patches(&active, document(draft), &patches)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}

#[test]
fn bedrock_bearer_keep_binds_mode_destination_and_refresh_or_keepalive_target() {
    let active = document(
        json!({"providers":[{"id":"bearer", "preset":"bedrock-converse", "auth_mode":"bedrock_bearer",
        "base_url":"https://bedrock-runtime.us-east-1.amazonaws.com", "credentials":[{"id":"a", "api_key":"synthetic-bearer",
        "extra_body":{"awsRegion":"us-east-1"}}]}]}),
    );
    for change in [
        json!({"base_url":"https://attacker.invalid"}),
        json!({"refresh_endpoint":"https://attacker.invalid/token"}),
        json!({"keepalive":{"serviceUrl":"https://attacker.invalid"}}),
    ] {
        let (mut draft, keeps) = redacted(&active);
        for (key, value) in change.as_object().unwrap() {
            draft["providers"][0]["credentials"][0][key] = value.clone();
        }
        assert_eq!(
            resolve_secret_patches(&active, document(draft), &keeps)
                .unwrap_err()
                .code(),
            "secret_keep_destination_changed"
        );
    }
}
