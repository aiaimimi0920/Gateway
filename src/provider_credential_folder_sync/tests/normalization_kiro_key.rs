//! The native CLI Key must never enter the legacy OAuth folder, even with a shared executor.
use super::super::{
    accounts::{build_provider_maps, select_provider_account_for_path},
    classification::{derive_provider_family_slug, derive_provider_surface_slug},
    layout::{default_folder_sync_relative_path, resolve_import_path_descriptor},
    normalization::normalize_import_payload,
    payload_metadata::{derive_credential_material_kind, materialize_export_payload},
};
use super::{build_test_credential, build_test_provider_account};
use crate::{db, protocol::kiro_api_key as key};
use serde_json::{json, Value};
mod safety;

fn provider() -> db::GatewayProviderAccountView {
    let mut account = build_test_provider_account(
        "Renamed",
        "kiro_compatible",
        "kiro",
        "kiro",
        key::BASE_URL,
        None,
        None,
    );
    account.service_provider_key = "kiro_platform".into();
    account.service_provider_label = "Kiro".into();
    account.payload =
        json!({"baseUrl":key::BASE_URL,"authMode":key::AUTH_MODE,"responsesPath":"/"});
    account
}
fn normalize(raw: Value) -> Result<Value, crate::error::GatewayError> {
    normalize_import_payload(key::PRESET_ID, &provider(), raw, Some("api_key"))
}

#[test]
fn kiro_key_folder_classifies_before_shared_executor_profile_and_mutable_labels() {
    let mut key_account = provider();
    key_account.id = "static-key-pool".into();
    key_account.label = "Grok renamed".into();
    assert_eq!(derive_provider_family_slug(&key_account), key::PRESET_ID);
    assert_eq!(derive_provider_surface_slug(&key_account), key::PRESET_ID);
    let mut oauth = provider();
    oauth.id = "oauth-pool".into();
    oauth.payload = json!({"baseUrl":"https://q.us-east-1.amazonaws.com"});
    assert_eq!(derive_provider_surface_slug(&oauth), "kiro-compatible");
    let maps = build_provider_maps(&[oauth.clone(), key_account.clone()]);
    for (path, expected) in [
        (
            "kiro-platform/kiro-api-key/api-key/key.json",
            "static-key-pool",
        ),
        ("kiro-api-key/key.json", "static-key-pool"),
        (
            "kiro-platform/kiro-compatible/bearer-token/key.json",
            "oauth-pool",
        ),
    ] {
        let descriptor = resolve_import_path_descriptor(path, &json!({})).unwrap();
        assert_eq!(
            select_provider_account_for_path(&maps, &descriptor).as_deref(),
            Some(expected)
        );
    }
    let only_key = build_provider_maps(&[key_account]);
    let old = resolve_import_path_descriptor("kiro/key.json", &json!({})).unwrap();
    assert!(select_provider_account_for_path(&only_key, &old).is_none());
}

#[cfg(feature = "line-kiro-official-vendor-api")]
#[test]
fn kiro_key_folder_imports_static_aliases_without_oauth_machine_or_secret_copies() {
    let result =
        normalize(json!({"apiKey":"synthetic-native-key","api_key":"synthetic-native-key"}))
            .unwrap();
    assert_eq!(result["api_key"], "synthetic-native-key");
    assert_eq!(result["auth_mode"], key::AUTH_MODE);
    assert_eq!(result["providerSurfaceKey"], key::PRESET_ID);
    assert_eq!(result["credentialMaterialKind"], "api_key");
    for field in [
        "apiKey",
        "kiro_oauth",
        "session_auth",
        "extra_body",
        "refresh_token",
        "rawSource",
    ] {
        assert!(result.get(field).is_none(), "retained {field}");
    }
}

#[test]
fn kiro_key_folder_rejects_oauth_session_wrong_kind_and_conflicting_aliases() {
    for raw in [
        json!({"apiKey":"synthetic-native-key","api_key":"different-private-key"}),
        json!({"apiKey":"synthetic-native-key","refreshToken":"private-refresh"}),
        json!({"apiKey":"synthetic-native-key","kiro_oauth":{}}),
        json!({"apiKey":"synthetic-native-key","sessionAuth":{"transport":"bearer"}}),
        json!({"apiKey":"synthetic-native-key","extraBody":{"kiroMachineId":"a".repeat(64)}}),
        json!({"apiKey":"synthetic-native-key","providerSurfaceKey":"kiro-compatible"}),
        json!({"apiKey":"synthetic-native-key","credentialMaterialKind":"bearer_token"}),
    ] {
        let error =
            normalize(raw).expect_err("static material must not accept OAuth or ambiguous copies");
        assert!(!error.message.contains("synthetic-native-key"));
        assert!(!error.message.contains("private-refresh"));
    }
}

#[test]
fn kiro_key_folder_cannot_convert_an_oauth_pool_or_bypass_the_selected_surface() {
    let mut oauth = provider();
    oauth.payload = json!({"baseUrl":"https://q.us-east-1.amazonaws.com"});
    assert!(normalize_import_payload(
        key::PRESET_ID,
        &oauth,
        json!({"apiKey":"synthetic-native-key"}),
        None
    )
    .is_err());
    assert!(normalize_import_payload(
        "openai",
        &provider(),
        json!({"apiKey":"synthetic-native-key"}),
        None
    )
    .is_err());
    assert!(normalize_import_payload(
        "kiro-compatible",
        &provider(),
        json!({"apiKey":"synthetic-native-key"}),
        None
    )
    .is_err());
}

#[test]
fn kiro_key_folder_default_export_uses_static_kind_not_the_shared_oauth_profile() {
    let account = provider();
    assert_eq!(derive_credential_material_kind(&account, None), "api_key");
    let credential = build_test_credential("stable-key", "folder_sync", None, "active", None);
    assert_eq!(
        default_folder_sync_relative_path(&account, &credential),
        "kiro-platform/kiro-api-key/api-key/stable-key.json"
    );
}
