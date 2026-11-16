//! Kiro registered secrets use normal secret patches, with field-independent authorization binding.
use super::*;
use neuro_gateway::console::secrets::{resolve_secret_patches, SecretPatch};

fn keeps(document: &RouteConfigYaml) -> Vec<SecretPatch> {
    redact_route_document(document)
        .unwrap()
        .secrets
        .iter()
        .map(|secret| SecretPatch::keep(&secret.path))
        .collect()
}

#[test]
fn kiro_secret_keep_roundtrip_and_stable_account_reorder_preserve_exact_material() {
    let mut active = document(context());
    let mut other = active.providers[0].credentials[0].clone();
    other.id = Some("second-account".into());
    other.api_key = Some("second-access".into());
    active.providers[0].credentials.push(other);
    let redacted = redact_route_document(&active).unwrap();
    let patches = keeps(&active);
    let mut draft = redacted.document;
    draft.providers[0].credentials.reverse();
    let resolved = resolve_secret_patches(&active, draft, &patches).unwrap();
    assert_eq!(
        resolved.providers[0].credentials[0].id.as_deref(),
        Some("second-account")
    );
    assert_eq!(
        resolved.providers[0].credentials[0].api_key.as_deref(),
        Some("second-access")
    );
    assert_eq!(
        resolved.providers[0].credentials[1].api_key.as_deref(),
        Some("synthetic-access")
    );
    assert_eq!(
        resolved.providers[0].credentials[1].kiro_oauth,
        active.providers[0].credentials[0].kiro_oauth
    );
}

#[test]
fn kiro_keep_rejects_every_registration_destination_or_secret_change() {
    for change in [
        json!({"client_id":"other-client"}),
        json!({"directory_region":"eu-west-1"}),
        json!({"auth_method":"identity_center","start_url":"https://other.awsapps.com/start"}),
        json!({"scopes":["codewhisperer:analysis"]}),
        json!({"client_secret_expires_at":1893456001i64}),
        json!({"machine_id":"b".repeat(64)}),
        Value::Null,
    ] {
        let active = document(context());
        let mut draft =
            serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
        let target = &mut draft["providers"][0]["credentials"][0]["kiro_oauth"];
        if change.is_null() {
            *target = Value::Null;
        } else {
            for (key, value) in change.as_object().unwrap() {
                target[key] = value.clone();
            }
        }
        assert!(
            resolve_secret_patches(
                &active,
                serde_json::from_value(draft).unwrap(),
                &keeps(&active)
            )
            .is_err(),
            "accepted {change}"
        );
    }
    let active = document(context());
    let draft = redact_route_document(&active).unwrap().document;
    let mut patches = keeps(&active);
    let path = "/providers/0/credentials/0/kiro_oauth/client_secret";
    *patches.iter_mut().find(|patch| patch.path == path).unwrap() =
        SecretPatch::replace(path, json!("different-secret"));
    assert!(
        resolve_secret_patches(&active, draft, &patches).is_err(),
        "kept access cannot silently gain another registration"
    );
}

#[test]
fn kiro_explicit_replace_and_clear_work_without_inline_secret_or_generic_fallback() {
    let active = document(context());
    let redacted = redact_route_document(&active).unwrap();
    assert!(resolve_secret_patches(&active, active.clone(), &[]).is_err());
    for clear in [false, true] {
        let patches: Vec<_> = redacted
            .secrets
            .iter()
            .map(|secret| {
                if clear || !secret.configured {
                    SecretPatch::clear(&secret.path)
                } else {
                    SecretPatch::replace(&secret.path, json!("synthetic-replacement"))
                }
            })
            .collect();
        let resolved =
            resolve_secret_patches(&active, redacted.document.clone(), &patches).unwrap();
        RouteConfigStore::from_document(resolved.clone()).unwrap();
        let credential = &resolved.providers[0].credentials[0];
        assert_eq!(
            credential
                .kiro_oauth
                .as_ref()
                .unwrap()
                .client_secret()
                .is_none(),
            clear
        );
    }
}

#[tokio::test]
async fn kiro_normal_management_commit_preserves_redaction_and_revision_material() {
    use neuro_gateway::console::{ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime};
    let root = std::env::temp_dir().join(format!("gateway-kiro-secret-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let routes = root.join("routes.yaml");
    let active = document(context());
    std::fs::write(&routes, serde_yaml::to_string(&active).unwrap()).unwrap();
    let config = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(routes),
        ..Default::default()
    })
    .unwrap();
    let runtime = RouteConfigRuntime::new_local(&config).unwrap();
    runtime.recover_startup().await.unwrap();
    let snapshot = runtime.route_config().snapshot();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].label = Some("policy metadata only".into());
    let resolved = resolve_secret_patches(&active, draft, &keeps(&active)).unwrap();
    runtime
        .commit_document(snapshot.revision().id(), resolved, None)
        .await
        .unwrap();
    let restarted = RouteConfigRuntime::new_local(&config).unwrap();
    restarted.recover_startup().await.unwrap();
    let revision = restarted
        .route_config()
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let stored = restarted
        .coordinator()
        .unwrap()
        .load_revision(&revision)
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.document().providers[0].credentials[0].kiro_oauth,
        active.providers[0].credentials[0].kiro_oauth
    );
}

#[test]
fn kiro_entire_context_can_be_deleted_in_one_explicit_material_clear() {
    let active = document(context());
    let redacted = redact_route_document(&active).unwrap();
    let mut draft = redacted.document;
    draft.providers[0].credentials[0].kiro_oauth = None;
    let clears: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| !s.path.contains("/kiro_oauth/"))
        .map(|s| SecretPatch::clear(&s.path))
        .collect();
    let resolved = resolve_secret_patches(&active, draft, &clears).unwrap();
    assert!(resolved.providers[0].credentials[0].kiro_oauth.is_none());
    assert!(resolved.providers[0].credentials[0].api_key.is_none());
    assert!(resolved.providers[0].credentials[0].refresh_token.is_none());
    RouteConfigStore::from_document(resolved).unwrap();
}

#[test]
fn kiro_context_deletion_without_nested_patch_cannot_keep_old_access_or_refresh() {
    let active = document(context());
    let redacted = redact_route_document(&active).unwrap();
    let mut draft = redacted.document;
    draft.providers[0].credentials[0].kiro_oauth = None;
    let keeps: Vec<_> = redacted
        .secrets
        .iter()
        .filter(|s| !s.path.contains("/kiro_oauth/"))
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let error = resolve_secret_patches(&active, draft, &keeps).unwrap_err();
    assert_eq!(error.code(), "secret_keep_destination_changed");
}
