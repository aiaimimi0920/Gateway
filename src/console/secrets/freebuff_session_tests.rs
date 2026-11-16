//! CLI session receipt material belongs to one credential and the ordinary secret patch owner.
use super::*;
use serde_json::json;

fn document() -> RouteConfigYaml {
    serde_json::from_value(
        json!({"providers":[{"id":"freebuff-pool","preset":"freebuff",
        "base_url":"https://freebuff.com","explicit_credentials_only":true,
        "credentials":[{"id":"stable-account","api_key":"synthetic-auth-token",
            "freebuff_session":{"fingerprint_id":"synthetic-device",
                "fingerprint_hash":"synthetic-device-hash"}}]}]}),
    )
    .unwrap()
}
fn path(field: &str) -> String {
    format!("/providers/0/credentials/0/{field}")
}
fn keeps() -> Vec<SecretPatch> {
    [
        "api_key",
        "freebuff_session/fingerprint_id",
        "freebuff_session/fingerprint_hash",
    ]
    .map(|field| SecretPatch::keep(path(field)))
    .into()
}

#[test]
fn freebuff_session_credential_redaction_and_keep_preserve_private_receipt() {
    let active = document();
    let redacted = redact_route_document(&active).unwrap();
    for field in ["fingerprint_id", "fingerprint_hash"] {
        assert!(redacted.secrets.iter().any(|secret| {
            secret.path == path(&format!("freebuff_session/{field}")) && secret.configured
        }));
    }
    let raw = serde_json::to_string(&redacted.document).unwrap();
    assert!(!raw.contains("synthetic-"));
    let restored = resolve_secret_patches(&active, redacted.document, &keeps()).unwrap();
    let restored = serde_json::to_value(restored).unwrap();
    assert_eq!(
        restored["providers"][0]["credentials"][0]["freebuff_session"],
        json!({"fingerprint_id":"synthetic-device","fingerprint_hash":"synthetic-device-hash"})
    );
    assert_eq!(restored, serde_json::to_value(active).unwrap());
}

#[test]
fn freebuff_session_embedded_receipt_requires_secret_patch_and_grant() {
    for field in ["fingerprint_id", "fingerprint_hash"] {
        let active = document();
        let mut draft =
            serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
        draft["providers"][0]["credentials"][0]["freebuff_session"][field] =
            json!("embedded-device-secret");
        let draft = serde_json::from_value(draft).unwrap();
        assert!(route_config_request_requires_secret_grant(&draft, &[]));
        assert_eq!(
            resolve_secret_patches(&active, draft, &keeps())
                .unwrap_err()
                .code(),
            "secret_value_must_use_patch"
        );
    }
}

#[test]
fn freebuff_session_keep_binds_complete_resolved_receipt_and_business_target() {
    let active = document();
    for (pointer, replacement) in [
        ("/base_url", json!("https://www.codebuff.com")),
        ("/preset", json!("openai")),
        (
            "/chat_completions_path",
            json!("https://evil.invalid/steal"),
        ),
        ("/call_flow", json!({"special_adapter":"none"})),
        ("/credentials/0/freebuff_session", json!({})),
        (
            "/extra_body",
            json!({"freebuffStartRunPath":"https://evil.invalid/start"}),
        ),
        (
            "/credentials/0/extra_body",
            json!({"freebuff_session_path":"/other"}),
        ),
    ] {
        let mut value =
            serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
        let full = format!("/providers/0{pointer}");
        let (parent, field) = full.rsplit_once('/').unwrap();
        value.pointer_mut(parent).unwrap()[field] = replacement;
        let mut patches = keeps();
        if pointer.ends_with("freebuff_session") {
            patches[1] =
                SecretPatch::replace(path("freebuff_session/fingerprint_id"), json!("new-device"));
            patches[2] =
                SecretPatch::replace(path("freebuff_session/fingerprint_hash"), json!("new-hash"));
        }
        let draft = serde_json::from_value(value).unwrap();
        assert_eq!(
            resolve_secret_patches(&active, draft, &patches)
                .unwrap_err()
                .code(),
            "secret_keep_destination_changed",
            "{pointer}"
        );
    }
}

#[test]
fn freebuff_session_metadata_edit_and_stable_id_reorder_do_not_move_secrets() {
    let mut active = document();
    let sibling =
        serde_json::from_value(json!({"id":"sibling","api_key":"sibling-secret"})).unwrap();
    active.providers[0].credentials.push(sibling);
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].label = Some("renamed".into());
    draft.providers[0].credentials[0].supported_models = vec!["different-model".into()];
    draft.providers[0].credentials.swap(0, 1);
    let patches = [
        SecretPatch::keep("/providers/0/credentials/0/api_key"),
        SecretPatch::keep("/providers/0/credentials/1/api_key"),
        SecretPatch::keep("/providers/0/credentials/1/freebuff_session/fingerprint_id"),
        SecretPatch::keep("/providers/0/credentials/1/freebuff_session/fingerprint_hash"),
    ];
    let restored = resolve_secret_patches(&active, draft, &patches).unwrap();
    assert_eq!(
        restored.providers[0].credentials[0].api_key.as_deref(),
        Some("sibling-secret")
    );
    assert_eq!(
        restored.providers[0].credentials[1].freebuff_session,
        active.providers[0].credentials[0].freebuff_session
    );
}

#[test]
fn freebuff_session_replace_clear_and_container_deletion_use_existing_secret_semantics() {
    let active = document();
    let mut patches = keeps();
    patches[0] = SecretPatch::replace(path("api_key"), json!("new-token"));
    patches[1] = SecretPatch::replace(path("freebuff_session/fingerprint_id"), json!("new-device"));
    patches[2] = SecretPatch::replace(path("freebuff_session/fingerprint_hash"), json!("new-hash"));
    let replaced = resolve_secret_patches(
        &active,
        redact_route_document(&active).unwrap().document,
        &patches,
    )
    .unwrap();
    assert_eq!(
        replaced.providers[0].credentials[0]
            .freebuff_session
            .as_ref()
            .unwrap()
            .fingerprint_id(),
        Some("new-device")
    );
    let clear: Vec<_> = patches
        .iter()
        .map(|p| SecretPatch::clear(&p.path))
        .collect();
    let cleared = resolve_secret_patches(
        &active,
        redact_route_document(&active).unwrap().document,
        &clear,
    )
    .unwrap();
    assert!(cleared.providers[0].credentials[0]
        .freebuff_session
        .as_ref()
        .unwrap()
        .fingerprint_id()
        .is_none());
    let mut deleted = redact_route_document(&active).unwrap().document;
    deleted.providers[0].credentials[0].freebuff_session = None;
    assert_eq!(
        resolve_secret_patches(
            &active,
            deleted.clone(),
            &[SecretPatch::keep(path("api_key"))]
        )
        .unwrap_err()
        .code(),
        "secret_keep_destination_changed"
    );
    assert!(
        resolve_secret_patches(&active, deleted, &[SecretPatch::clear(path("api_key"))]).is_ok()
    );
}

#[test]
fn freebuff_session_identity_replacement_and_unknown_schema_do_not_gain_kept_material() {
    let active = document();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].credentials[0].id = Some("replacement-account".into());
    assert!(resolve_secret_patches(&active, draft, &keeps()).is_err());
    let mut value = serde_json::to_value(&active).unwrap();
    value["providers"][0]["credentials"][0]["freebuff_session"]["token_endpoint"] =
        json!("https://evil.invalid");
    assert!(serde_json::from_value::<RouteConfigYaml>(value).is_err());
    let debug = format!("{:?}", active.providers[0].credentials[0].freebuff_session);
    assert!(!debug.contains("synthetic-"));
}

#[test]
fn freebuff_business_consent_tracks_stable_account_and_not_models_or_array_positions() {
    let mut active = document();
    active.providers[0].credentials[0].extra_body.insert(
        "freebuffBusiness".into(),
        json!({"origin":"https://codebuff.com","client":"cli"}),
    );
    active.providers[0]
        .credentials
        .push(serde_json::from_value(json!({"id":"sibling"})).unwrap());
    active.providers.push(
        serde_json::from_value(json!({"id":"unrelated","base_url":"https://example.invalid"}))
            .unwrap(),
    );
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].credentials[0].supported_models = vec!["new-manual".into()];
    draft.providers[0].credentials.swap(0, 1);
    draft.providers.swap(0, 1);
    assert!(!freebuff_business_target_changed(&active, &draft));
    let key = "freebuffBusiness";
    let original = draft.providers[1].credentials[1]
        .extra_body
        .remove(key)
        .unwrap();
    assert!(freebuff_business_target_changed(&active, &draft));
    draft.providers[1].credentials[0]
        .extra_body
        .insert(key.into(), original.clone());
    assert!(freebuff_business_target_changed(&active, &draft));
    draft.providers[1].credentials[1]
        .extra_body
        .insert(key.into(), original);
    assert!(
        freebuff_business_target_changed(&active, &draft),
        "new sibling audience requires consent too"
    );
    draft.providers[1].credentials[0].extra_body.remove(key);
    draft.providers[1].credentials.remove(1);
    assert!(
        !freebuff_business_target_changed(&active, &draft),
        "account removal transfers no authority"
    );
}
