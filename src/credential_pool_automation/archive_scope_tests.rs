//! A configurable/shared archive directory never makes ordinary JSON deletable.
use super::archive::{
    archive_pruned_credentials_in_directory, archived_credential_count_in_directory,
    purge_credential_archive_directory,
};
use crate::routing::config::ProviderConfigYaml;
use std::{collections::HashSet, fs};
#[test]
fn shared_archive_purge_preserves_other_providers_and_material_json() {
    let root = std::env::temp_dir().join(format!("archive-scope-{}", uuid::Uuid::new_v4()));
    for id in ["provider-a", "provider-b"] {
        let provider: ProviderConfigYaml = serde_json::from_value(serde_json::json!({
            "id":id, "base_url":"https://example.invalid", "credentials":[{"id":"account", "api_key":"fixture"}]
        })).unwrap();
        archive_pruned_credentials_in_directory(
            &provider,
            &HashSet::from(["account".into()]),
            &root,
        )
        .unwrap();
    }
    fs::write(
        root.join("ordinary.json"),
        r#"{"id":"live","api_key":"preserve"}"#,
    )
    .unwrap();
    fs::write(
        root.join("malformed.json"),
        r#"{"schemaVersion":1,"providerId":"provider-a"}"#,
    )
    .unwrap();
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-a"),
        1
    );
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-b"),
        1
    );
    assert_eq!(
        purge_credential_archive_directory(&root, "provider-a").unwrap(),
        1
    );
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-a"),
        0
    );
    assert_eq!(
        archived_credential_count_in_directory(&root, "provider-b"),
        1
    );
    assert!(root.join("ordinary.json").exists());
    assert!(root.join("malformed.json").exists());
    // A storage directory accidentally selected as an archive retains live files.
    assert_eq!(
        purge_credential_archive_directory(&root, "not-an-owner").unwrap(),
        0
    );
    assert!(root.exists());
    fs::remove_dir_all(root).unwrap();
}
