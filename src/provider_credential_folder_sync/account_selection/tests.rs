//! Exercise the same selection/cache owner called before the import phase's hash check.
use super::*;
use crate::protocol::kiro_api_key as key;
use serde_json::json;
use std::cell::RefCell;

mod aliases;
mod failures;

fn account(id: &str, base: &str) -> db::GatewayProviderAccountView {
    let mut account = db::GatewayProviderAccountView {
        id: id.into(),
        label: id.into(),
        service_provider_key: "kiro_platform".into(),
        service_provider_label: "Kiro".into(),
        adapter: "kiro_compatible".into(),
        protocol_family: "kiro".into(),
        protocol_profile: "kiro".into(),
        status: "active".into(),
        source_kind: None,
        aggregator_api_mode: None,
        web_reverse_access_mode: None,
        source_notes: None,
        execution_mode: crate::routing::candidate::ProviderExecutionMode::DirectHttp,
        endpoint_execution_modes: None,
        payload: json!({"baseUrl":base}),
        storage_mode: "r2".into(),
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    if key::owns_base(base) {
        account.payload["authMode"] = json!(key::AUTH_MODE);
        account.payload["responsesPath"] = json!("/");
    }
    account
}

fn metadata(
    account: &db::GatewayProviderAccountView,
    base: Option<&str>,
) -> db::ProviderAccountFolderSyncMetadata {
    db::ProviderAccountFolderSyncMetadata {
        id: account.id.clone(),
        label: account.label.clone(),
        service_provider_key: account.service_provider_key.clone(),
        service_provider_label: account.service_provider_label.clone(),
        adapter: account.adapter.clone(),
        protocol_family: account.protocol_family.clone(),
        protocol_profile: account.protocol_profile.clone(),
        source_kind: account.source_kind.clone(),
        web_reverse_access_mode: account.web_reverse_access_mode.clone(),
        payload_base_url: base.map(str::to_string),
    }
}

fn descriptor(surface: &str) -> FolderImportPathDescriptor {
    super::super::layout::resolve_import_path_descriptor(
        &format!("kiro-platform/{surface}/api-key/fixture.json"),
        &json!({}),
    )
    .unwrap()
}

fn no_load(_: String) -> std::future::Ready<Result<db::GatewayProviderAccountView, GatewayError>> {
    panic!("selection must not perform an unrelated payload read")
}

#[tokio::test]
async fn kiro_object_discovery_selects_key_and_reuses_the_import_cache() {
    let key_account = account("key", key::BASE_URL);
    let oauth = account("oauth", "https://q.us-east-1.amazonaws.com");
    let mut selection =
        ImportAccountSelection::new(vec![metadata(&oauth, None), metadata(&key_account, None)]);
    let calls = RefCell::new(Vec::new());
    let selected = selection
        .select_with(&descriptor(key::PRESET_ID), |id| {
            calls.borrow_mut().push(id.clone());
            let value = if id == "key" {
                key_account.clone()
            } else {
                oauth.clone()
            };
            async move { Ok(value) }
        })
        .await
        .unwrap();
    assert_eq!(selected.as_deref(), Some("key"));
    assert_eq!(*calls.borrow(), ["oauth", "key"]);
    let cached = hydrate_with(&mut selection.cache, "key", &mut no_load)
        .await
        .unwrap();
    assert_eq!(cached.id, "key");
    assert_eq!(
        selection
            .select_with(&descriptor(key::PRESET_ID), no_load)
            .await
            .unwrap()
            .as_deref(),
        Some("key")
    );
    #[cfg(feature = "line-kiro-official-vendor-api")]
    {
        let normalized = super::super::normalization::normalize_import_payload(
            key::PRESET_ID,
            &cached,
            json!({"KIRO_API_KEY":"synthetic-key"}),
            Some("api_key"),
        )
        .unwrap();
        assert_eq!(normalized["auth_mode"], key::AUTH_MODE);
        assert_eq!(normalized["api_key"], "synthetic-key");
    }
}

#[tokio::test]
async fn kiro_object_discovery_reclassifies_before_legacy_oauth_selection() {
    let key_account = account("A-key-renamed", key::BASE_URL);
    let oauth = account("Z-oauth", "https://q.us-east-1.amazonaws.com");
    let mut selection =
        ImportAccountSelection::new(vec![metadata(&key_account, None), metadata(&oauth, None)]);
    assert_eq!(
        selection
            .select_with(&descriptor("kiro-compatible"), |id| {
                let value = if id == key_account.id {
                    key_account.clone()
                } else {
                    oauth.clone()
                };
                async move { Ok(value) }
            })
            .await
            .unwrap()
            .as_deref(),
        Some("Z-oauth")
    );
    let mut only_key = ImportAccountSelection::new(vec![metadata(&key_account, None)]);
    assert!(only_key
        .select_with(&descriptor("kiro-compatible"), |_| {
            let value = key_account.clone();
            async move { Ok(value) }
        })
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn kiro_object_discovery_is_lazy_and_ignores_label_only_candidates() {
    let mut unrelated = account("Kiro renamed label", "https://example.invalid");
    unrelated.adapter = "openai_compatible".into();
    unrelated.protocol_family = "openai".into();
    unrelated.protocol_profile = "openai".into();
    unrelated.service_provider_key = "openai_platform".into();
    let key_account = account("key", key::BASE_URL);
    let mut selection = ImportAccountSelection::new(vec![
        metadata(&unrelated, None),
        metadata(&key_account, None),
    ]);
    let ordinary = descriptor("openai");
    assert!(selection.select_with(&ordinary, no_load).await.is_ok());
    let calls = RefCell::new(Vec::new());
    assert_eq!(
        selection
            .select_with(&descriptor(key::PRESET_ID), |id| {
                calls.borrow_mut().push(id);
                let value = key_account.clone();
                async move { Ok(value) }
            })
            .await
            .unwrap()
            .as_deref(),
        Some("key")
    );
    assert_eq!(*calls.borrow(), ["key"]);
}

#[tokio::test]
async fn kiro_object_discovery_preserves_inline_maps_and_existing_ranking() {
    let first = account("A-key", key::BASE_URL);
    let last = account("Z-key", key::BASE_URL);
    let mut selection = ImportAccountSelection::new(vec![
        metadata(&last, Some(key::BASE_URL)),
        metadata(&first, Some(key::BASE_URL)),
    ]);
    assert_eq!(
        selection
            .select_with(&descriptor(key::PRESET_ID), no_load)
            .await
            .unwrap()
            .as_deref(),
        Some("A-key")
    );
}

#[tokio::test]
async fn kiro_object_discovery_matches_hydrated_ranking_with_snake_case_base() {
    let mut first = account("A-key", key::BASE_URL);
    first.payload.as_object_mut().unwrap().remove("baseUrl");
    first.payload["base_url"] = json!(key::BASE_URL);
    let last = account("Z-key", key::BASE_URL);
    let expected = select_provider_account_for_path(
        &build_provider_maps(&[last.clone(), first.clone()]),
        &descriptor(key::PRESET_ID),
    );
    let mut selection =
        ImportAccountSelection::new(vec![metadata(&last, None), metadata(&first, None)]);
    let selected = selection
        .select_with(&descriptor(key::PRESET_ID), |id| {
            let value = if id == first.id {
                first.clone()
            } else {
                last.clone()
            };
            async move { Ok(value) }
        })
        .await
        .unwrap();
    assert_eq!(selected, expected);
    assert_eq!(selected.as_deref(), Some("A-key"));
}

#[tokio::test]
async fn kiro_object_discovery_can_use_an_already_hydrated_account() {
    let key_account = account("key", key::BASE_URL);
    let mut selection = ImportAccountSelection::new(vec![metadata(&key_account, None)]);
    selection.cache.insert(key_account);
    assert_eq!(
        selection
            .select_with(&descriptor(key::PRESET_ID), no_load)
            .await
            .unwrap()
            .as_deref(),
        Some("key")
    );
}
