//! Legacy family fallback aliases must resolve object identity before selecting OAuth.
use super::*;

async fn assert_legacy_selection(descriptor: FolderImportPathDescriptor) {
    assert_eq!(descriptor.provider_surface_slug, "kiro");
    let key_account = account("A-key", key::BASE_URL);
    let oauth = account("Z-oauth", "https://q.us-east-1.amazonaws.com");
    let mut selection =
        ImportAccountSelection::new(vec![metadata(&key_account, None), metadata(&oauth, None)]);
    let calls = RefCell::new(Vec::new());
    let selected = selection
        .select_with(&descriptor, |id| {
            calls.borrow_mut().push(id.clone());
            let value = if id == key_account.id {
                key_account.clone()
            } else {
                oauth.clone()
            };
            async move { Ok(value) }
        })
        .await
        .unwrap();
    assert_eq!(selected.as_deref(), Some("Z-oauth"));
    assert_eq!(*calls.borrow(), ["A-key", "Z-oauth"]);
    let mut only_key = ImportAccountSelection::new(vec![metadata(&key_account, None)]);
    assert!(only_key
        .select_with(&descriptor, |_| {
            let value = key_account.clone();
            async move { Ok(value) }
        })
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn kiro_object_discovery_covers_legacy_two_segment_family_alias() {
    let descriptor = super::super::super::layout::resolve_import_path_descriptor(
        "kiro-platform/key.json",
        &json!({}),
    )
    .unwrap();
    assert_legacy_selection(descriptor).await;
}

#[tokio::test]
async fn kiro_object_discovery_covers_payload_surface_alias_override() {
    let descriptor = super::super::super::layout::resolve_import_path_descriptor(
        "unrelated/key.json",
        &json!({"providerSurfaceKey":"kiro-platform"}),
    )
    .unwrap();
    assert_legacy_selection(descriptor).await;
}
