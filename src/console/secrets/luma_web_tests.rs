//! Luma Web Keep must retain the original destination and board, including inherited material.
use super::*;
use serde_json::json;

fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"luma","preset":"lumalabs",
        "base_url":"https://app.lumalabs.ai","credentials":[{"id":"saved",
        "api_key":"synthetic-cookie","extra_body":{"realmId":"board-a"},
        "headers":{"User-Agent":"original-agent"}}]}]}))
    .unwrap()
}
fn keeps(active: &RouteConfigYaml) -> Vec<SecretPatch> {
    redact_route_document(active)
        .unwrap()
        .secrets
        .into_iter()
        .filter(|secret| secret.configured)
        .map(|secret| SecretPatch::keep(secret.path))
        .collect()
}

#[test]
fn luma_web_keep_rejects_destination_and_board_changes() {
    let active = document();
    let redacted = serde_json::to_value(redact_route_document(&active).unwrap().document).unwrap();
    for (path, replacement) in [
        ("/providers/0/base_url", json!("https://other.invalid")),
        ("/providers/0/preset", json!("openai")),
        (
            "/providers/0/credentials/0/extra_body/realmId",
            json!("board-b"),
        ),
        (
            "/providers/0/credentials/0/headers/User-Agent",
            json!("other-agent"),
        ),
    ] {
        let mut draft = redacted.clone();
        *draft.pointer_mut(path).unwrap() = replacement;
        let error = resolve_secret_patches(
            &active,
            serde_json::from_value(draft).unwrap(),
            &keeps(&active),
        )
        .unwrap_err();
        assert_eq!(error.code(), "secret_keep_destination_changed", "{path}");
    }
}

#[test]
fn luma_web_keep_preserves_secrets_for_model_edits_and_rejects_new_consumers() {
    let active = document();
    let mut draft = redact_route_document(&active).unwrap().document;
    draft.providers[0].label = Some("renamed".into());
    draft.providers[0].credentials[0].supported_models = vec!["uni-1".into()];
    let saved = resolve_secret_patches(&active, draft, &keeps(&active)).unwrap();
    assert_eq!(
        saved.providers[0].credentials[0].api_key.as_deref(),
        Some("synthetic-cookie")
    );

    let mut inherited = document();
    inherited.providers[0].api_key = "provider-cookie".into();
    inherited.providers[0].credentials[0].api_key = None;
    inherited.providers[0]
        .extra_body
        .insert("realmId".into(), json!("board-a"));
    let mut draft = redact_route_document(&inherited).unwrap().document;
    let mut another = draft.providers[0].credentials[0].clone();
    another.id = Some("another".into());
    draft.providers[0].credentials.push(another);
    assert_eq!(
        resolve_secret_patches(&inherited, draft, &keeps(&inherited))
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}

#[test]
fn luma_web_keep_detects_adapter_or_origin_without_preset() {
    for adapter in [None, Some("lumalabs_compatible".to_string())] {
        let mut active = document();
        active.providers[0].preset = None;
        active.providers[0].adapter = adapter;
        let mut draft = redact_route_document(&active).unwrap().document;
        draft.providers[0].credentials[0].base_url = Some("https://other.invalid".into());
        assert_eq!(
            resolve_secret_patches(&active, draft, &keeps(&active))
                .unwrap_err()
                .code(),
            "secret_keep_destination_changed"
        );
    }
}
